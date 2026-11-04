//! mDNS advertisement of a fake Chromecast (`_googlecast._tcp.local`).
//!
//! This is what makes Cast senders on the LAN discover us. The TXT records
//! (`id`, `md`, `fn`, `ca`, ...) are what the Android Cast SDK / Google Home
//! reads to decide we are a plausible Chromecast.

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::info;

/// Register `_googlecast._tcp.local` and return the daemon (keep it alive!).
pub fn advertise(
    friendly_name: &str,
    model: &str,
    device_id: &str,
    port: u16,
    capabilities: u32,
) -> Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new().context("failed to start mDNS daemon")?;

    let host_name = format!("{}.local.", sanitize_host(friendly_name));
    let ip = local_ip().context("failed to determine local IP")?;

    let ca = capabilities.to_string();
    let properties: Vec<(&str, &str)> = vec![
        ("id", device_id),     // device id (16 hex chars)
        ("ve", "05"),          // version
        ("md", model),         // model, e.g. "Chromecast Ultra"
        ("fn", friendly_name), // friendly name
        ("ca", &ca),           // capabilities bitmask (decimal)
        ("st", "0"),           // setup state
        ("ic", "/setup/icon.png"),
        ("nf", "1"),
        ("rs", ""),
    ];

    let service = ServiceInfo::new(
        "_googlecast._tcp.local.",
        friendly_name,
        &host_name,
        &ip,
        port,
        &properties[..],
    )
    .context("failed to build mDNS service info")?;

    daemon
        .register(service)
        .context("failed to register mDNS service")?;

    info!("advertising _googlecast._tcp.local '{friendly_name}' ({model}) at {ip}:{port}");
    Ok(daemon)
}

fn sanitize_host(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "openchromecast".to_string()
    } else {
        cleaned
    }
}

/// Determine the LAN IP to advertise. Ask Windows which local address it
/// would use for an outbound route first: enumerating adapters and taking the
/// first RFC 1918 address can accidentally select a virtual/host-only adapter
/// (for example 192.168.137.1) instead of the active Wi-Fi address.
fn local_ip() -> Result<String> {
    use std::net::{IpAddr, Ipv4Addr};

    // UDP connect performs route selection without sending a packet. On a
    // normal home/office network this returns the active LAN interface, e.g.
    // 192.168.1.x, rather than the first private address in adapter order.
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if let IpAddr::V4(v4) = addr.ip() {
                    if !v4.is_unspecified() && !v4.is_loopback() {
                        return Ok(v4.to_string());
                    }
                }
            }
        }
    }

    // Fallback for networks without a usable default route.
    let mut fallback: Option<Ipv4Addr> = None;
    for (_name, ip) in local_ip_address::list_afinet_netifas()? {
        if let IpAddr::V4(v4) = ip {
            if v4.is_private() && !v4.is_loopback() {
                return Ok(v4.to_string());
            }
            if fallback.is_none() && !v4.is_loopback() && !v4.is_unspecified() {
                fallback = Some(v4);
            }
        }
    }
    fallback
        .map(|ip| ip.to_string())
        .context("no usable IPv4 address found")
}
