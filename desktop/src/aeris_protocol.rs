use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};
use tokio::{io::AsyncReadExt, net::{TcpListener, TcpStream}, sync::{Mutex, Notify}, time::Instant};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use crate::mpv_player::MpvPlayer;

const PROTOCOL_VERSION: u8 = 1;
const PORT: u16 = 45872;
const UPLOAD_PORT: u16 = 45873;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CHUNK_BYTES: usize = 1024 * 1024;
const PAIRING_LIFETIME: Duration = Duration::from_secs(180);

#[derive(Debug, Deserialize)]
struct Envelope {
    protocol: String,
    version: u8,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    payload: Value,
}

struct TemporaryFileGuard {
    path: PathBuf,
    keep: bool,
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct IncomingMedia {
    path: PathBuf,
    cleanup: TemporaryFileGuard,
    file: File,
    expected_size: u64,
    received_size: u64,
    expected_hash: String,
    file_name: String,
    mime_type: String,
    track_title: String,
    track_artist: String,
}

pub async fn run_server(shutdown: Arc<Notify>, status: mpsc::Sender<String>) -> Result<()> {
    let ip = local_ip_address::local_ip().context("Не удалось определить IP-адрес локальной сети")?;
    if !is_private_ip(ip) {
        bail!("Определён непубличный LAN-адрес: {ip}. Сервер не будет запускаться.");
    }
    let listener = TcpListener::bind(SocketAddr::new(ip, PORT))
        .await
        .with_context(|| format!("Не удалось открыть порт {PORT}"))?;
    let upload_listener = TcpListener::bind(SocketAddr::new(ip, UPLOAD_PORT))
        .await
        .with_context(|| format!("Не удалось открыть порт {UPLOAD_PORT} для браузерной загрузки"))?;

    let pairing_code = Uuid::new_v4().simple().to_string()[..6].to_ascii_uppercase();
    let expires_at = Instant::now() + PAIRING_LIFETIME;
    let paired_tokens = Arc::new(Mutex::new(load_paired_tokens()));
    let player = Arc::new(Mutex::new(MpvPlayer::new()));

    let mut playback_tick = tokio::time::interval(Duration::from_millis(500));

    let _ = status.send(format!(
        "AERIS Protocol v1 запущен\nАдрес: ws://{ip}:{PORT}/ws\nЗагрузка из браузера: http://{ip}:{UPLOAD_PORT}\nКод подключения: {pairing_code}\nКод действует 3 минуты"
    ));

    loop {
        tokio::select! {
            _ = playback_tick.tick() => {
                if let Ok(playback) = player.lock().await.playback_status().await {
                    if playback.get("idle").and_then(Value::as_bool) != Some(true) {
                        let _ = status.send(format!("PLAYBACK:{}", playback));
                    }
                }
            }
            _ = shutdown.notified() => {
                let _ = status.send("Сервер AERIS остановлен".into());
                break;
            }
            accepted = listener.accept() => {
                let (stream, peer) = accepted.context("Ошибка входящего TCP-подключения")?;
                let code = pairing_code.clone();
                let status = status.clone();
                let tokens = paired_tokens.clone();
                let player = player.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_client(stream, peer, code, expires_at, tokens, player, status).await {
                        tracing::warn!("AERIS client {peer} disconnected: {error:#}");
                    }
                });
            }
            upload_accepted = upload_listener.accept() => {
                let (stream, peer) = upload_accepted.context("Ошибка браузерного подключения")?;
                let code = pairing_code.clone();
                let status = status.clone();
                let player = player.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_browser_upload(stream, peer, code, expires_at, player, status).await {
                        tracing::warn!("AERIS browser upload from {peer} failed: {error:#}");
                    }
                });
            }
        }
    }
    Ok(())
}

fn extract_embedded_cover(media_path: &std::path::Path) -> Option<PathBuf> {
    use lofty::{file::TaggedFileExt, picture::PictureType, tag::TagExt};
    let tagged = lofty::read_from_path(media_path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().iter().find(|picture| {
        picture.pic_type() == PictureType::CoverFront || picture.pic_type() == PictureType::CoverBack
    }).or_else(|| tag.pictures().first())?;
    let data = picture.data();
    if data.is_empty() || data.len() > 12 * 1024 * 1024 { return None; }
    let directory = std::env::temp_dir().join("AERIS").join("Received");
    fs::create_dir_all(&directory).ok()?;
    let path = directory.join(format!("cover-{}.img", Uuid::new_v4()));
    fs::write(&path, data).ok()?;
    Some(path)
}

async fn handle_client(
    stream: TcpStream,
    peer: SocketAddr,
    pairing_code: String,
    expires_at: Instant,
    paired_tokens: Arc<Mutex<HashSet<String>>>,
    player: Arc<Mutex<MpvPlayer>>,
    status: mpsc::Sender<String>,
) -> Result<()> {
    // AERIS is a LAN-only receiver. Do not even complete a WebSocket handshake
    // for public/non-local peers, even if the port was exposed accidentally.
    if !is_private_ip(peer.ip()) {
        bail!("Отклонено подключение не из локальной сети: {}", peer.ip());
    }

    let ws = accept_async(stream).await.context("WebSocket handshake failed")?;
    let (mut write, mut read) = ws.split();
    let mut paired = false;
    let mut device_label = peer.to_string();
    let mut incoming: Option<IncomingMedia> = None;

    while let Some(message) = read.next().await {
        match message.context("WebSocket read failed")? {
            Message::Text(text) => {
                if text.len() > MAX_MESSAGE_BYTES {
                    send_error(&mut write, None, "message_too_large", "JSON больше 64 КиБ").await?;
                    bail!("message too large");
                }
                let env: Envelope = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(error) => {
                        send_error(&mut write, None, "invalid_json", &error.to_string()).await?;
                        continue;
                    }
                };
                if env.protocol != "aeris" || env.version != PROTOCOL_VERSION {
                    send_error(&mut write, env.id, "unsupported_protocol", "Ожидается AERIS v1").await?;
                    continue;
                }

                match env.kind.as_str() {
                    "hello" => {
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":1,"type":"hello_ack","id":env.id,
                            "payload":{
                                "server_name":"AERIS Desktop",
                                "protocol_version":1,
                                "pairing_required":true,
                                "pairing_expires_in":expires_at.saturating_duration_since(Instant::now()).as_secs()
                            }
                        })).await?;
                    }
                    "pair" => {
                        if let Some(token) = env.payload.get("token").and_then(Value::as_str) {
                            let valid = paired_tokens.lock().await.contains(token);
                            if valid {
                                paired = true;
                                let device_name = env.payload.get("device_name").and_then(Value::as_str).unwrap_or("Android").chars().take(48).collect::<String>();
                                device_label = format!("{device_name} ({peer})");
                                let _ = status.send(format!("Повторно подключено устройство: {device_label}"));
                                send_json(&mut write, json!({
                                    "protocol":"aeris","version":1,"type":"pair_result","id":env.id,
                                    "payload":{"paired":true,"session":"remembered","token":token}
                                })).await?;
                            } else {
                                send_json(&mut write, json!({
                                    "protocol":"aeris","version":1,"type":"pair_result","id":env.id,
                                    "payload":{"paired":false,"reason":"invalid_device_token"}
                                })).await?;
                            }
                        } else {
                            let code = env.payload.get("code").and_then(Value::as_str).unwrap_or("");
                            if Instant::now() > expires_at || code != pairing_code {
                                send_json(&mut write, json!({
                                    "protocol":"aeris","version":1,"type":"pair_result","id":env.id,
                                    "payload":{"paired":false,"reason":"invalid_or_expired_code"}
                                })).await?;
                            } else {
                                let token = Uuid::new_v4().simple().to_string();
                                {
                                    let mut tokens = paired_tokens.lock().await;
                                    tokens.insert(token.clone());
                                    if let Err(error) = save_paired_tokens(&tokens) {
                                        tokens.remove(&token);
                                        send_error(&mut write, env.id, "pairing_storage_failed", &format!("Не удалось сохранить сопряжение: {error}")).await?;
                                        continue;
                                    }
                                }
                                paired = true;
                                let device_name = env.payload.get("device_name").and_then(Value::as_str).unwrap_or("Android").chars().take(48).collect::<String>();
                                device_label = format!("{device_name} ({peer})");
                                let _ = status.send(format!("Подключено новое устройство: {device_label}"));
                                send_json(&mut write, json!({
                                    "protocol":"aeris","version":1,"type":"pair_result","id":env.id,
                                    "payload":{"paired":true,"session":"remembered","token":token}
                                })).await?;
                            }
                        }
                    }
                    "ping" => {
                        send_json(&mut write, json!({"protocol":"aeris","version":1,"type":"pong","id":env.id,"payload":{}})).await?;
                    }

                    "get_status" => {
                        if !paired {
                            send_error(&mut write, env.id, "pairing_required", "Сначала выполните сопряжение").await?;
                            continue;
                        }
                        match player.lock().await.playback_status().await {
                            Ok(playback) => send_json(&mut write, json!({
                                "protocol":"aeris","version":1,"type":"playback_status","id":env.id,
                                "payload":playback
                            })).await?,
                            Err(error) => send_error(&mut write, env.id, "player_error", &format!("{error:#}")).await?,
                        }
                    }
                    "begin_media" => {
                        if !paired {
                            send_error(&mut write, env.id, "pairing_required", "Сначала выполните сопряжение").await?;
                            continue;
                        }
                        if incoming.is_some() {
                            send_error(&mut write, env.id, "transfer_in_progress", "Другая передача уже выполняется").await?;
                            continue;
                        }
                        let size = env.payload.get("size").and_then(Value::as_u64).unwrap_or(0);
                        let hash = env.payload.get("sha256").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
                        let name = env.payload.get("file_name").and_then(Value::as_str).unwrap_or("media.bin");
                        let mime = env.payload.get("mime_type").and_then(Value::as_str).unwrap_or("application/octet-stream");
                        if size == 0 || size > MAX_MEDIA_BYTES || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                            send_error(&mut write, env.id, "invalid_media_metadata", "Файл пустой, больше 512 МиБ или SHA-256 некорректен").await?;
                            continue;
                        }
                        let safe = sanitize_name(name);
                        let dir = std::env::temp_dir().join("AERIS").join("Received");
                        fs::create_dir_all(&dir)?;
                        let path = dir.join(format!("{}-{}", Uuid::new_v4(), safe));
                        let file = File::create(&path)?;
                        if let Some(encoded_cover) = env.payload.get("cover_base64").and_then(Value::as_str) {
                            if encoded_cover.len() <= 48 * 1024 {
                                if let Ok(cover_bytes) = base64::engine::general_purpose::STANDARD.decode(encoded_cover) {
                                    if !cover_bytes.is_empty() && cover_bytes.len() <= 36 * 1024 {
                                        let cover_dir = std::env::temp_dir().join("AERIS").join("Covers");
                                        if fs::create_dir_all(&cover_dir).is_ok() {
                                            let cover_path = cover_dir.join(format!("{}.img", Uuid::new_v4()));
                                            if fs::write(&cover_path, &cover_bytes).is_ok() {
                                                let _ = status.send(format!("COVER_PATH:{}", cover_path.display()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        incoming = Some(IncomingMedia {
                            path: path.clone(),
                            cleanup: TemporaryFileGuard { path, keep: false },
                            file,
                            expected_size: size,
                            received_size: 0,
                            expected_hash: hash,
                            file_name: safe,
                            mime_type: mime.to_string(),
                            track_title: env.payload.get("track_title").and_then(Value::as_str).unwrap_or("").to_string(),
                            track_artist: env.payload.get("track_artist").and_then(Value::as_str).unwrap_or("").to_string(),
                        });
                        let _ = status.send(format!("Получение файла: {name} ({:.1} МБ)", size as f64 / 1048576.0));
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":1,"type":"media_ready","id":env.id,
                            "payload":{"chunk_size":MAX_CHUNK_BYTES}
                        })).await?;
                    }
                    "finish_media" => {
                        if !paired {
                            send_error(&mut write, env.id, "pairing_required", "Сначала выполните сопряжение").await?;
                            continue;
                        }
                        let Some(mut transfer) = incoming.take() else {
                            send_error(&mut write, env.id, "no_transfer", "Нет активной передачи").await?;
                            continue;
                        };
                        transfer.file.flush()?;
                        drop(transfer.file);
                        if transfer.received_size != transfer.expected_size {
                            let _ = fs::remove_file(&transfer.path);
                            send_error(&mut write, env.id, "size_mismatch", "Передан не весь файл").await?;
                            continue;
                        }
                        let mut file = File::open(&transfer.path)?;
                        let mut hasher = Sha256::new();
                        let mut buffer = [0u8; 65536];
                        loop {
                            let count = file.read(&mut buffer)?;
                            if count == 0 { break; }
                            hasher.update(&buffer[..count]);
                        }
                        if format!("{:x}", hasher.finalize()) != transfer.expected_hash {
                            let _ = fs::remove_file(&transfer.path);
                            send_error(&mut write, env.id, "hash_mismatch", "Проверка целостности не пройдена").await?;
                            continue;
                        }

                        // Once the complete file passes size and SHA-256 checks, keep it
                        // even if the external player cannot open it.
                        transfer.cleanup.keep = true;
                        let playback = match player.lock().await.play_file(&transfer.path).await {
                            Ok(()) => {
                                let title = if transfer.track_title.trim().is_empty() { "Получено медиа".to_owned() } else { transfer.track_title.trim().to_owned() };
                                let artist = if transfer.track_artist.trim().is_empty() { "Неизвестный исполнитель".to_owned() } else { transfer.track_artist.trim().to_owned() };
                                let _ = status.send(format!("Воспроизведение: {} · {}", title, artist));
                                if let Some(cover_path) = extract_embedded_cover(&transfer.path) {
                                    let _ = status.send(format!("COVER:{}", cover_path.display()));
                                }
                                "playing"
                            }
                            Err(error) => {
                                let _ = status.send(format!("Файл принят, но mpv не запустился: {error:#}"));
                                "playback_failed"
                            }
                        };
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":1,"type":"media_complete","id":env.id,
                            "payload":{"received":true,"playback":playback,"file_name":transfer.file_name}
                        })).await?;
                    }
                    "cancel_media" => {
                        if let Some(transfer) = incoming.take() {
                            drop(transfer.file);
                            let _ = fs::remove_file(transfer.path);
                        }
                        send_json(&mut write, json!({"protocol":"aeris","version":1,"type":"media_cancelled","id":env.id,"payload":{}})).await?;
                    }
                    "play" | "pause" | "toggle_pause" | "stop" | "seek" | "seek_relative" | "set_volume" | "volume_delta" => {
                        if !paired {
                            send_error(&mut write, env.id, "pairing_required", "Сначала выполните сопряжение").await?;
                            continue;
                        }
                        let result = match env.kind.as_str() {
                            "play" => player.lock().await.play().await,
                            "pause" => player.lock().await.pause().await,
                            "toggle_pause" => player.lock().await.toggle_pause().await,
                            "stop" => player.lock().await.stop().await,
                            "seek" => {
                                let position = env.payload.get("position").and_then(Value::as_f64);
                                match position.filter(|v| v.is_finite() && (0.0..=86400.0).contains(v)) {
                                    Some(value) => player.lock().await.seek(value).await,
                                    None => {
                                        send_error(&mut write, env.id, "invalid_seek", "Позиция должна быть от 0 до 86400 секунд").await?;
                                        continue;
                                    }
                                }
                            }
                            "seek_relative" => {
                                let offset = env.payload.get("offset").and_then(Value::as_f64);
                                match offset.filter(|v| v.is_finite() && (-3600.0..=3600.0).contains(v)) {
                                    Some(value) => player.lock().await.seek_relative(value).await,
                                    None => {
                                        send_error(&mut write, env.id, "invalid_seek", "Смещение должно быть от -3600 до 3600 секунд").await?;
                                        continue;
                                    }
                                }
                            }
                            "set_volume" => {
                                let volume = env.payload.get("volume").and_then(Value::as_f64);
                                match volume.filter(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
                                    Some(value) => player.lock().await.set_volume(value * 100.0).await,
                                    None => {
                                        send_error(&mut write, env.id, "invalid_volume", "Громкость должна быть от 0.0 до 1.0").await?;
                                        continue;
                                    }
                                }
                            }
                            "volume_delta" => {
                                let delta = env.payload.get("delta").and_then(Value::as_f64);
                                match delta.filter(|v| v.is_finite() && (-100.0..=100.0).contains(v)) {
                                    Some(value) => player.lock().await.change_volume(value).await,
                                    None => {
                                        send_error(&mut write, env.id, "invalid_volume", "Изменение громкости должно быть от -100 до 100").await?;
                                        continue;
                                    }
                                }
                            }
                            _ => unreachable!(),
                        };
                        match result {
                            Ok(()) => send_json(&mut write, json!({
                                "protocol":"aeris","version":1,"type":"command_result","id":env.id,
                                "payload":{"success":true,"command":env.kind}
                            })).await?,
                            Err(error) => send_error(&mut write, env.id, "player_error", &format!("{error:#}")).await?,
                        }
                    }
                    other => send_error(&mut write, env.id, "unknown_message_type", &format!("Неизвестная команда: {other}")).await?,
                }
            }
            Message::Binary(bytes) => {
                if !paired {
                    send_error(&mut write, None, "pairing_required", "Сначала выполните сопряжение").await?;
                    continue;
                }
                let Some(transfer) = incoming.as_mut() else {
                    send_error(&mut write, None, "no_transfer", "Сначала отправьте begin_media").await?;
                    continue;
                };
                if bytes.len() > MAX_CHUNK_BYTES || transfer.received_size + bytes.len() as u64 > transfer.expected_size {
                    if let Some(transfer) = incoming.take() {
                        drop(transfer.file);
                        let _ = fs::remove_file(transfer.path);
                    }
                    send_error(&mut write, None, "invalid_chunk", "Недопустимый размер блока").await?;
                    continue;
                }
                transfer.file.write_all(&bytes)?;
                transfer.received_size += bytes.len() as u64;
                if transfer.received_size % (8 * 1024 * 1024) < bytes.len() as u64 {
                    let _ = status.send(format!("Получено {:.1} / {:.1} МБ", transfer.received_size as f64 / 1048576.0, transfer.expected_size as f64 / 1048576.0));
                }
            }
            Message::Ping(data) => write.send(Message::Pong(data)).await?,
            Message::Close(_) => { write.flush().await?; break; }
            _ => {}
        }
    }

    if let Some(transfer) = incoming.take() {
        drop(transfer.file);
        let _ = fs::remove_file(transfer.path);
    }
    if paired { let _ = status.send(format!("Устройство отключено: {device_label}")); }
    Ok(())
}

const BROWSER_UPLOAD_PAGE: &str = r#"<!doctype html>
<html lang="ru">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="dark">
<title>AERIS — отправка файла</title>
<style>
*{box-sizing:border-box}body{margin:0;min-height:100vh;background:#121923;color:#f3f7fb;font:16px system-ui,sans-serif;display:grid;place-items:center;padding:20px}
main{width:min(100%,440px);background:#1b2532;border:1px solid #2b3b4d;border-radius:22px;padding:26px;box-shadow:0 18px 55px #0005}
.logo{color:#2a9bdc;font-size:13px;font-weight:800;letter-spacing:.18em}h1{font-size:27px;margin:8px 0}p{color:#a9b8c8;line-height:1.5}
label{display:block;font-size:13px;color:#a9b8c8;margin:18px 0 7px}
input{width:100%;padding:13px;border:1px solid #40536a;border-radius:11px;background:#111a25;color:#fff;font:inherit}
button{width:100%;margin-top:20px;padding:14px;border:0;border-radius:12px;background:#2a9bdc;color:white;font-weight:700;font-size:16px}
button:disabled{opacity:.55}#result{margin-top:16px;min-height:24px;font-size:14px;overflow-wrap:anywhere}
small{display:block;margin-top:18px;color:#8092a5;line-height:1.5}
</style></head>
<body><main>
<div class="logo">AERIS · LOCAL TRANSFER</div>
<h1>Отправить на компьютер</h1>
<p>Выбери аудио или видео в памяти телефона. Файл уйдёт напрямую на компьютер по Wi-Fi — Telegram для этого не нужен.</p>
<label for="code">Код подключения с AERIS Desktop</label>
<input id="code" inputmode="text" maxlength="6" autocomplete="off" placeholder="Например, A1B2C3">
<label for="file">Музыка или видео</label>
<input id="file" type="file" accept="audio/*,video/*,.mp3,.m4a,.flac,.wav,.mp4,.mkv,.webm,.mov">
<button id="send" type="button">Отправить и воспроизвести</button>
<div id="result" role="status"></div>
<small>Оба устройства должны быть в одной локальной сети. Код действует 3 минуты. Используй только доверенную домашнюю сеть.</small>
</main>
<script>
const code=document.getElementById('code');
const file=document.getElementById('file');
const button=document.getElementById('send');
const result=document.getElementById('result');
button.addEventListener('click',async()=>{
 const selected=file.files&&file.files[0];
 if(!selected){result.textContent='Сначала выбери файл.';return;}
 if(!/^[A-Za-z0-9]{6}$/.test(code.value.trim())){result.textContent='Введи шестизначный код с компьютера.';return;}
 if(selected.size===0||selected.size>512*1024*1024){result.textContent='Файл пустой или больше 512 МиБ.';return;}
 button.disabled=true;result.textContent='Передаём файл… Не закрывай страницу.';
 try{
  const url='/upload?code='+encodeURIComponent(code.value.trim().toUpperCase())+'&name='+encodeURIComponent(selected.name);
  const response=await fetch(url,{method:'POST',headers:{'Content-Type':'application/octet-stream'},body:selected});
  const data=await response.json();
  if(!response.ok||!data.success)throw new Error(data.error||'Ошибка загрузки');
  result.textContent='Готово! Воспроизведение: '+data.file_name;
 }catch(error){result.textContent='Не удалось отправить: '+error.message;}
 finally{button.disabled=false;}
});
</script></body></html>"#;

async fn handle_browser_upload(
    mut stream: TcpStream,
    peer: SocketAddr,
    pairing_code: String,
    expires_at: Instant,
    player: Arc<Mutex<MpvPlayer>>,
    status: mpsc::Sender<String>,
) -> Result<()> {
    if !is_private_ip(peer.ip()) {
        send_http_json(&mut stream, "403 Forbidden", json!({"success":false,"error":"AERIS принимает только подключения из локальной сети"})).await?;
        bail!("Отклонено браузерное подключение не из локальной сети: {}", peer.ip());
    }

    let mut request = Vec::new();
    let mut buffer = [0u8; 16 * 1024];
    let header_end = loop {
        let count = tokio::time::timeout(Duration::from_secs(60), stream.read(&mut buffer))
            .await
            .context("Таймаут ожидания HTTP-заголовков браузера")?
            .context("Не удалось прочитать HTTP-запрос")?;
        if count == 0 {
            bail!("Браузер закрыл соединение до отправки заголовков");
        }
        request.extend_from_slice(&buffer[..count]);
        if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            break position;
        }
        if request.len() > 16 * 1024 {
            bail!("HTTP-заголовки слишком большие");
        }
    };

    let initial_body = request[header_end + 4..].to_vec();
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let target = request_parts.next().unwrap_or("/");
    let mut content_length = None;
    let mut content_length_seen = false;
    let mut ambiguous_http_framing = false;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                if content_length_seen {
                    ambiguous_http_framing = true;
                }
                content_length_seen = true;
                content_length = value.trim().parse::<u64>().ok();
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                // Chunked requests are not supported by this small single-request
                // receiver. Reject them rather than guessing the request body length.
                ambiguous_http_framing = true;
            }
        }
    }
    if ambiguous_http_framing {
        send_http_json(&mut stream, "400 Bad Request", json!({
            "success":false,
            "error":"Некорректные или неподдерживаемые HTTP-заголовки загрузки."
        })).await?;
        return Ok(());
    }

    if method == "GET" && (target == "/" || target.starts_with("/?")) {
        send_http_response(&mut stream, "200 OK", "text/html; charset=utf-8", BROWSER_UPLOAD_PAGE.as_bytes()).await?;
        return Ok(());
    }

    let Some((path, query)) = target.split_once('?') else {
        send_http_json(&mut stream, "404 Not Found", json!({"success":false,"error":"Страница не найдена"})).await?;
        return Ok(());
    };
    if method != "POST" || path != "/upload" {
        send_http_json(&mut stream, "404 Not Found", json!({"success":false,"error":"Страница не найдена"})).await?;
        return Ok(());
    }

    let code = query_parameter(query, "code").unwrap_or_default().to_ascii_uppercase();
    if Instant::now() > expires_at || code != pairing_code {
        send_http_json(&mut stream, "403 Forbidden", json!({"success":false,"error":"Код неверный или истёк. Посмотри новый код в AERIS Desktop."})).await?;
        return Ok(());
    }

    let Some(size) = content_length else {
        send_http_json(&mut stream, "411 Length Required", json!({"success":false,"error":"Браузер не передал размер файла."})).await?;
        return Ok(());
    };
    if size == 0 || size > MAX_MEDIA_BYTES {
        send_http_json(&mut stream, "413 Content Too Large", json!({"success":false,"error":"Файл пустой или больше лимита 512 МиБ."})).await?;
        return Ok(());
    }

    let file_name = sanitize_name(&query_parameter(query, "name").unwrap_or_else(|| "media.bin".into()));
    let directory = std::env::temp_dir().join("AERIS").join("Received");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{}-{}", Uuid::new_v4(), file_name));
    let mut cleanup = TemporaryFileGuard { path: path.clone(), keep: false };
    let mut file = File::create(&path)?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;

    let first_count = initial_body.len().min(size as usize);
    if first_count > 0 {
        file.write_all(&initial_body[..first_count])?;
        hasher.update(&initial_body[..first_count]);
        received += first_count as u64;
    }
    while received < size {
        let count = tokio::time::timeout(Duration::from_secs(60), stream.read(&mut buffer))
            .await
            .context("Таймаут ожидания данных файла из браузера")?
            .context("Ошибка при получении файла из браузера")?;
        if count == 0 {
            drop(file);
            let _ = fs::remove_file(&path);
            bail!("Браузер оборвал загрузку: получено {received} из {size} байт");
        }
        let take = (size - received).min(count as u64) as usize;
        file.write_all(&buffer[..take])?;
        hasher.update(&buffer[..take]);
        received += take as u64;
    }
    file.flush()?;
    drop(file);
    let digest = format!("{:x}", hasher.finalize());
    // The upload is complete; preserve it even if playback fails.
    cleanup.keep = true;

    match player.lock().await.play_file(&path).await {
        Ok(()) => {
            let _ = status.send(format!("Получено из браузера: {} · {:.1} МБ", file_name, size as f64 / 1048576.0));
            send_http_json(&mut stream, "200 OK", json!({
                "success":true,
                "file_name":file_name.clone(),
                "size":size,
                "sha256":digest.clone()
            })).await?;
            tracing::info!("AERIS browser upload from {peer}: {file_name} ({size} bytes, sha256 {digest})");
        }
        Err(error) => {
            let _ = status.send(format!("Файл получен, но mpv не запустил его: {error:#}"));
            send_http_json(&mut stream, "500 Internal Server Error", json!({
                "success":false,
                "error":format!("Файл принят, но mpv не смог его открыть: {error:#}")
            })).await?;
        }
    }
    Ok(())
}

async fn send_http_json(stream: &mut TcpStream, status: &str, value: Value) -> Result<()> {
    let body = serde_json::to_vec(&value)?;
    send_http_response(stream, status, "application/json; charset=utf-8", &body).await
}

async fn send_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    use tokio::io::AsyncWriteExt;
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

fn query_parameter(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (url_decode(name) == key).then(|| url_decode(value))
    })
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => { decoded.push(b' '); index += 1; }
            b'%' if index + 2 < bytes.len() => {
                let hex = |byte: u8| -> Option<u8> {
                    match byte {
                        b'0'..=b'9' => Some(byte - b'0'),
                        b'a'..=b'f' => Some(byte - b'a' + 10),
                        b'A'..=b'F' => Some(byte - b'A' + 10),
                        _ => None,
                    }
                };
                if let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    decoded.push((high << 4) | low);
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
            byte => { decoded.push(byte); index += 1; }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}


fn token_store_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("AERIS").join("paired_devices.json")
}

fn load_paired_tokens() -> HashSet<String> {
    fs::read(token_store_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn save_paired_tokens(tokens: &HashSet<String>) -> Result<()> {
    let path = token_store_path();
    let parent = path.parent().context("Не удалось определить папку сопряжённых устройств")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let mut values: Vec<&String> = tokens.iter().collect();
    values.sort();
    fs::write(&temporary, serde_json::to_vec_pretty(&values)?)?;
    fs::rename(&temporary, &path).or_else(|_| {
        fs::copy(&temporary, &path)?;
        fs::remove_file(&temporary)
    })?;
    Ok(())
}

async fn send_error<S>(sink: &mut S, id: Option<String>, code: &str, message: &str) -> Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    send_json(sink, json!({"protocol":"aeris","version":1,"type":"error","id":id,"payload":{"code":code,"message":message}})).await
}

async fn send_json<S>(sink: &mut S, value: Value) -> Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    sink.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

fn sanitize_name(input: &str) -> String {
    let name = input.rsplit(|c| c == '/' || c == '\\').next().unwrap_or("media.bin");
    // Keep Unicode letters and numbers (including Cyrillic filenames) while
    // removing path separators, control characters and Windows-invalid symbols.
    let safe: String = name.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')'))
        .take(100)
        .collect();
    let safe = safe.trim().trim_matches('.');
    if safe.is_empty() { "media.bin".into() } else { safe.into() }
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || (ip.octets()[0] == 169 && ip.octets()[1] == 254),
        std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}


#[cfg(test)]
mod tests {
    use super::is_private_ip;
    use std::net::IpAddr;

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("valid test IP")
    }

    #[test]
    fn accepts_private_and_local_addresses() {
        for address in [
            "10.0.0.8",
            "172.16.0.8",
            "192.168.1.20",
            "127.0.0.1",
            "169.254.1.2",
            "fc00::1",
            "fe80::1",
            "::1",
        ] {
            assert!(is_private_ip(ip(address)), "{address} should be local");
        }
    }

    #[test]
    fn rejects_public_addresses() {
        for address in ["8.8.8.8", "1.1.1.1", "203.0.113.10", "2001:4860:4860::8888"] {
            assert!(!is_private_ip(ip(address)), "{address} should not be local");
        }
    }

    #[test]
    fn rejects_non_local_special_ranges() {
        for address in ["224.0.0.1", "240.0.0.1", "100.64.0.1"] {
            assert!(!is_private_ip(ip(address)), "{address} should not be accepted as a private LAN peer");
        }
    }

    #[test]
    fn sanitizes_uploaded_file_names() {
        assert_eq!(super::sanitize_name("../../movie.mp4"), "movie.mp4");
        assert_eq!(super::sanitize_name(r"C:\\Users\\user\\track.mp3"), "track.mp3");
        assert_eq!(super::sanitize_name("..."), "media.bin");
        assert_eq!(super::sanitize_name("bad<>name?.mp4"), "badname.mp4");
        assert_eq!(super::sanitize_name("Моя музыка.mp3"), "Моя музыка.mp3");
        assert_eq!(super::sanitize_name("видео 2026.mp4"), "видео 2026.mp4");
    }

    #[test]
    fn decodes_browser_upload_query_parameters() {
        assert_eq!(super::query_parameter("code=ABC123&name=My%20video%20%281%29.mp4", "name").as_deref(), Some("My video (1).mp4"));
        assert_eq!(super::query_parameter("code=ABC123&name=a+b.mp3", "name").as_deref(), Some("a b.mp3"));
        assert_eq!(super::query_parameter("code=ABC123", "name"), None);
    }

    #[test]
    fn temporary_upload_guard_removes_incomplete_file() {
        let path = std::env::temp_dir().join(format!("aeris-test-{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"partial").expect("write test file");
        drop(super::TemporaryFileGuard { path: path.clone(), keep: false });
        assert!(!path.exists());
    }

    #[test]
    fn temporary_upload_guard_preserves_completed_file() {
        let path = std::env::temp_dir().join(format!("aeris-test-{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"complete").expect("write test file");
        let mut guard = super::TemporaryFileGuard { path: path.clone(), keep: false };
        guard.keep = true;
        drop(guard);
        assert!(path.exists());
        std::fs::remove_file(path).expect("remove test file");
    }
}
