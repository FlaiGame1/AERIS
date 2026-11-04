use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
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
use tokio::{net::{TcpListener, TcpStream}, sync::{Mutex, Notify}, time::Instant};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use crate::mpv_player::MpvPlayer;

const PROTOCOL_VERSION: u8 = 1;
const PORT: u16 = 45872;
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

struct IncomingMedia {
    path: PathBuf,
    file: File,
    expected_size: u64,
    received_size: u64,
    expected_hash: String,
    file_name: String,
    mime_type: String,
}

pub async fn run_server(shutdown: Arc<Notify>, status: mpsc::Sender<String>) -> Result<()> {
    let ip = local_ip_address::local_ip().context("Не удалось определить IP-адрес локальной сети")?;
    if !is_private_ip(ip) {
        bail!("Определён непубличный LAN-адрес: {ip}. Сервер не будет запускаться.");
    }
    let listener = TcpListener::bind(SocketAddr::new(ip, PORT))
        .await
        .with_context(|| format!("Не удалось открыть порт {PORT}"))?;

    let pairing_code = Uuid::new_v4().simple().to_string()[..6].to_ascii_uppercase();
    let expires_at = Instant::now() + PAIRING_LIFETIME;
    let paired_tokens = Arc::new(Mutex::new(load_paired_tokens()));
    let player = Arc::new(Mutex::new(MpvPlayer::new()));

    let _ = status.send(format!(
        "AERIS Protocol v1 запущен\nАдрес: ws://{ip}:{PORT}/ws\nКод подключения: {pairing_code}\nКод действует 3 минуты"
    ));

    loop {
        tokio::select! {
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
        }
    }
    Ok(())
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
    let ws = accept_async(stream).await.context("WebSocket handshake failed")?;
    let (mut write, mut read) = ws.split();
    let mut paired = false;
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
                                let _ = status.send(format!("Повторно подключено устройство: {peer}"));
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
                                let _ = status.send(format!("Подключено новое устройство: {peer}"));
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
                        incoming = Some(IncomingMedia {
                            path,
                            file,
                            expected_size: size,
                            received_size: 0,
                            expected_hash: hash,
                            file_name: safe,
                            mime_type: mime.to_string(),
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

                        let playback = match player.lock().await.play_file(&transfer.path).await {
                            Ok(()) => {
                                let _ = status.send(format!("Воспроизведение: {} · {}", transfer.file_name, transfer.mime_type));
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
    Ok(())
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
    let safe: String = name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')'))
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
