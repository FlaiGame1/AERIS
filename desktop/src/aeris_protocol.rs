use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::{Arc, mpsc},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Notify,
    time::Instant,
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

const PROTOCOL_VERSION: u8 = 1;
const PORT: u16 = 45872;
const MAX_MESSAGE_BYTES: usize = 64 * 1024;
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

pub async fn run_server(
    shutdown: Arc<Notify>,
    status: mpsc::Sender<String>,
) -> Result<()> {
    let ip = local_ip_address::local_ip().context("Не удалось определить IP-адрес локальной сети")?;
    if !is_private_ip(ip) {
        bail!("Определён непубличный LAN-адрес: {ip}. Сервер не будет запускаться.");
    }

    let address = SocketAddr::new(ip, PORT);
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("Не удалось открыть порт {PORT}"))?;

    // A short-lived one-time code is shown in the desktop UI. This first
    // development version pairs one WebSocket session at a time.
    let pairing_code = Uuid::new_v4().simple().to_string()[..6].to_ascii_uppercase();
    let expires_at = Instant::now() + PAIRING_LIFETIME;

    let _ = status.send(format!(
        "AERIS Protocol v1 запущен\nАдрес: ws://{ip}:{PORT}/ws\nКод подключения: {pairing_code}\nКод действует 3 минуты"
    ));

    loop {
        tokio::select! {
            _ = shutdown.notified() => {
                let _ = status.send("Сервер AERIS остановлен".to_owned());
                break;
            }
            accepted = listener.accept() => {
                let (stream, peer) = accepted.context("Ошибка входящего TCP-подключения")?;
                let code = pairing_code.clone();
                let status = status.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_client(stream, peer, code, expires_at, status).await {
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
    status: mpsc::Sender<String>,
) -> Result<()> {
    let ws = accept_async(stream).await.context("WebSocket handshake failed")?;
    let (mut write, mut read) = ws.split();
    let mut paired = false;

    while let Some(message) = read.next().await {
        let message = message.context("WebSocket read failed")?;
        match message {
            Message::Text(text) => {
                if text.len() > MAX_MESSAGE_BYTES {
                    send_json(&mut write, json!({
                        "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                        "payload":{"code":"message_too_large","message":"Maximum message size is 64 KiB"}
                    })).await?;
                    bail!("message exceeds 64 KiB");
                }

                let envelope: Envelope = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(error) => {
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                            "payload":{"code":"invalid_json","message":error.to_string()}
                        })).await?;
                        continue;
                    }
                };

                if envelope.protocol != "aeris" || envelope.version != PROTOCOL_VERSION {
                    send_json(&mut write, json!({
                        "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                        "id":envelope.id,"payload":{"code":"unsupported_protocol","message":"Expected AERIS protocol version 1"}
                    })).await?;
                    continue;
                }

                match envelope.kind.as_str() {
                    "hello" => {
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":PROTOCOL_VERSION,"type":"hello_ack",
                            "id":envelope.id,
                            "payload":{"server_name":"AERIS Desktop","protocol_version":1,
                                "pairing_required":true,"pairing_expires_in":expires_at.saturating_duration_since(Instant::now()).as_secs()}
                        })).await?;
                    }
                    "pair" => {
                        let supplied = envelope.payload.get("code").and_then(Value::as_str).unwrap_or("");
                        if Instant::now() > expires_at || supplied != pairing_code {
                            send_json(&mut write, json!({
                                "protocol":"aeris","version":PROTOCOL_VERSION,"type":"pair_result",
                                "id":envelope.id,"payload":{"paired":false,"reason":"invalid_or_expired_code"}
                            })).await?;
                        } else {
                            paired = true;
                            let _ = status.send(format!("Подключено устройство: {peer}"));
                            send_json(&mut write, json!({
                                "protocol":"aeris","version":PROTOCOL_VERSION,"type":"pair_result",
                                "id":envelope.id,"payload":{"paired":true,"session":"temporary"}
                            })).await?;
                        }
                    }
                    "ping" => {
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":PROTOCOL_VERSION,"type":"pong","id":envelope.id,"payload":{}
                        })).await?;
                    }
                    "play_url" | "play" | "pause" | "stop" | "seek" | "set_volume" => {
                        if !paired {
                            send_json(&mut write, json!({
                                "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                                "id":envelope.id,"payload":{"code":"pairing_required","message":"Pair this device before sending commands"}
                            })).await?;
                            continue;
                        }

                        if envelope.kind == "play_url" {
                            let raw_url = envelope.payload.get("url").and_then(Value::as_str).unwrap_or("");
                            let parsed = url::Url::parse(raw_url);
                            match parsed {
                                Ok(url) if url.scheme() == "http" || url.scheme() == "https" => {}
                                _ => {
                                    send_json(&mut write, json!({
                                        "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                                        "id":envelope.id,"payload":{"code":"invalid_media_url","message":"Only HTTP and HTTPS media URLs are accepted"}
                                    })).await?;
                                    continue;
                                }
                            }
                        }

                        // The protocol transport is live. Player integration is the next
                        // development step; acknowledge receipt without claiming playback.
                        let _ = status.send(format!("Команда от {peer}: {}", envelope.kind));
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":PROTOCOL_VERSION,"type":"command_ack",
                            "id":envelope.id,"payload":{"received":true,"playback":"not_implemented_yet"}
                        })).await?;
                    }
                    other => {
                        send_json(&mut write, json!({
                            "protocol":"aeris","version":PROTOCOL_VERSION,"type":"error",
                            "id":envelope.id,"payload":{"code":"unknown_message_type","message":format!("Unknown message type: {other}")}
                        })).await?;
                    }
                }
            }
            Message::Ping(data) => write.send(Message::Pong(data)).await?,
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

async fn send_json<S>(sink: &mut S, value: Value) -> Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    sink.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

fn is_private_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || (ip.octets()[0] == 169 && ip.octets()[1] == 254),
        std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}
