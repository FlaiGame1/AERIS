//! OpenChromecast receiver engine integrated into NEXUS CAST.

pub mod config;
pub mod crypto;
pub mod mdns;
pub mod media;
pub mod player;
pub mod proto;
pub mod receiver;
pub mod server;
pub mod state;
pub mod cover;
pub mod youtube;

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Notify};
use tracing::{info, warn};

/// The core receiver: identity, mDNS, player backend, TLS listener. Runs until
/// the listener ends, Ctrl-C, or the tray requests shutdown.
pub async fn run_receiver(cli: config::Cli, shutdown: Arc<Notify>, ready: std::sync::mpsc::Sender<String>) -> Result<()> {
    let device_id = match &cli.device_id {
        Some(d) => d.clone(),
        // Full 128-bit UUID (32 hex chars): pychromecast explicitly ignores
        // devices whose `id` TXT is not a parseable UUID.
        None => uuid::Uuid::new_v4().simple().to_string(),
    };

    // What a Cast picker will show. The computer's own name unless the run said otherwise.
    let friendly_name = cli.friendly_name.clone().unwrap_or_else(computer_name);

    // --- Device identity (signing key + TLS certificate) ---
    let identity = crypto::Identity::load_or_generate(cli.cert.as_deref(), cli.key.as_deref())
        .context("failed to prepare device identity")?;
    info!(
        "device identity ready (auth cert {} bytes)",
        identity.auth_cert_der().len()
    );

    // --- mDNS advertisement ---
    // Non-fatal: if local-network privacy (macOS 15+) blocks multicast mDNS,
    // still run the receiver (TCP listener) and tell the user how to fix
    // discovery instead of silently dying.
    //
    // (No startup probe is needed to "trigger" the macOS Local Network
    // prompt — on stable macOS the first multicast mDNS send itself surfaces
    // it. A macOS 27 beta presentation bug suppressed the prompt entirely;
    // that is not fixable from the app.)
    let _mdns = match mdns::advertise(
        &friendly_name,
        &cli.model,
        &device_id,
        cli.port,
        cli.capabilities,
    ) {
        Ok(d) => {
            info!("advertising '{}' ({})", friendly_name, cli.model);
            Some(d)
        }
        Err(e) => {
            warn!(
                "mDNS advertisement failed: {e:#}. The device will NOT be \
                 discoverable. On macOS, open System Settings -> Privacy & \
                 Security -> Local Network and allow OpenChromecast, then \
                 restart the app."
            );
            None
        }
    };

    // --- Player backend ---
    let player = match cli.player.to_ascii_lowercase().as_str() {
        "none" => {
            info!("using null player (no media playback)");
            player::PlayerHandle::spawn_null()
        }
        "vlc" => {
            let path = resolve_vlc_path(&cli.vlc);
            info!("spawning VLC player ({path}), rc port {}", cli.vlc_port);
            match player::vlc::spawn(&path, cli.vlc_port).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("VLC unavailable ({e:#}); falling back to null player");
                    player::PlayerHandle::spawn_null()
                }
            }
        }
        _ => {
            let mpv_path = resolve_mpv_path(&cli.mpv);
            let ipc = cli.ipc.clone().unwrap_or_else(default_ipc_path);
            info!("spawning mpv player ({mpv_path}), ipc {ipc}");
            match player::mpv::spawn(&mpv_path, &ipc).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("mpv unavailable ({e:#}); falling back to null player");
                    player::PlayerHandle::spawn_null()
                }
            }
        }
    };

    // --- Shared state ---
    let shared = server::Shared {
        state: Arc::new(Mutex::new(state::AppState::default())),
        identity: Arc::new(identity),
        player,
    };

    // --- Cast V2 TLS listener ---
    let listener = TcpListener::bind(("0.0.0.0", cli.port))
        .await
        .with_context(|| format!("failed to bind port {}", cli.port))?;
    info!("listening for Cast V2 connections on port {}", cli.port);
    let _ = ready.send(format!("Приёмник запущен на порту {}. Ищи NEXUS CAST в телефоне", cli.port));

    tokio::select! {
        r = server::run(listener, shared) => r?,
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl-C, shutting down");
        }
        _ = shutdown.notified() => {
            info!("exit requested, shutting down");
        }
    }

    Ok(())
}


/// Platform-appropriate default mpv IPC endpoint.
fn default_ipc_path() -> String {
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\openchromecast-{}", std::process::id())
    }
    #[cfg(unix)]
    {
        // temp_dir() (not a hard-coded /tmp) so it lands in the app's writable
        // temp — under the Mac App Store sandbox /tmp is not writable and the
        // real path is the container's temp dir.
        std::env::temp_dir()
            .join(format!("openchromecast-{}.sock", std::process::id()))
            .display()
            .to_string()
    }
}

/// Resolve the mpv executable path: honor an explicit path (anything other
/// than the bare `mpv` default), then auto-detect well-known install
/// locations (e.g. the winget package dir), then fall back to PATH.
fn resolve_mpv_path(explicit: &str) -> String {
    if !explicit.is_empty() && explicit != "mpv" {
        return explicit.to_string();
    }
    // 1) mpv bundled inside the app (self-contained: users install nothing).
    if let Some(p) = bundled_mpv_path() {
        return p;
    }
    #[cfg(windows)]
    {
        if let Ok(root) = std::env::var("LOCALAPPDATA") {
            let packages = std::path::Path::new(&root).join(r"Microsoft\WinGet\Packages");
            if let Ok(entries) = std::fs::read_dir(&packages) {
                for entry in entries.flatten() {
                    let candidate = entry.path().join("mpv.exe");
                    if candidate.exists() {
                        return candidate.to_string_lossy().to_string();
                    }
                }
            }
        }
    }
    "mpv".to_string()
}

/// What this computer is called, as the platform's own interface calls it.
///
/// The *computer name*, not the DNS host name: `Yukari's MacBook Pro` is what the person in front of it
/// recognises, and `yukaris-macbook-pro.local` is what nobody does. Each platform keeps its own answer and
/// none of them is `gethostname`.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    #[cfg(windows)]
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        if !name.is_empty() {
            return name;
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Ok(contents) = std::fs::read_to_string("/etc/hostname") {
        let name = contents.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    // A machine that will not say still has to be called something.
    "OpenChromecast".to_string()
}

/// Find a mpv shipped inside the app package (next to the executable, or in
/// the .app bundle's Resources on macOS). Returns `None` when not bundled.
fn bundled_mpv_path() -> Option<String> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    #[cfg(windows)]
    let candidates = [exe_dir.join("mpv").join("mpv.exe"), exe_dir.join("mpv.exe")];
    // Inside OpenChromecast.app: <exe_dir> = Contents/MacOS, so the bundled
    // player lives in Contents/Resources/mpv/.
    #[cfg(target_os = "macos")]
    let candidates = [
        exe_dir
            .join("..")
            .join("Resources")
            .join("mpv")
            .join("bin")
            .join("mpv"),
        exe_dir.join("..").join("Resources").join("mpv").join("mpv"),
        exe_dir.join("mpv"),
    ];
    #[cfg(not(any(windows, target_os = "macos")))]
    let candidates = [exe_dir.join("mpv"), exe_dir.join("mpv").join("mpv")];
    candidates
        .iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

/// Resolve the VLC executable path (explicit flag, else well-known locations).
fn resolve_vlc_path(explicit: &str) -> String {
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    #[cfg(windows)]
    {
        const CANDIDATES: [&str; 2] = [
            r"C:\Program Files\VideoLAN\VLC\vlc.exe",
            r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe",
        ];
        for p in CANDIDATES {
            if std::path::Path::new(p).exists() {
                return p.to_string();
            }
        }
    }
    "vlc".to_string()
}
