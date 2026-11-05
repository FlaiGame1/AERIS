use anyhow::{Context, Result, bail};
use alhazen_core::{Player, PlayerConfig, PlayerEvent, PlayerState, Source, VideoFrame};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, time::Duration};

/// AERIS-owned playback/session layer. alhazen-core supplies codec backends;
/// AERIS owns the lifecycle, remote-control contract, status and UI frame handoff.
pub struct AerisPlayer {
    player: Option<Player>,
    current_path: Option<PathBuf>,
    last_error: Option<String>,
}

impl Default for AerisPlayer {
    fn default() -> Self { Self::new() }
}

impl AerisPlayer {
    pub fn new() -> Self {
        Self { player: None, current_path: None, last_error: None }
    }

    pub fn play_file(&mut self, path: &Path) -> Result<()> {
        if !path.is_file() { bail!("Медиафайл не найден: {}", path.display()); }
        let source = Source::parse(&path.to_string_lossy())
            .context("Не удалось определить источник медиафайла")?;
        let config = PlayerConfig {
            autoplay: false,
            prefer_hardware: true,
            decoder_threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(4),
            max_output_size: Some((1920, 1080)),
            ..PlayerConfig::default()
        };
        // Open/probe first. Keep the previous item alive if the new file cannot be opened.
        let next = Player::open(source, config).context("AERIS не смог открыть медиапоток")?;
        next.play();
        let previous_path = self.current_path.replace(path.to_path_buf());
        self.player = Some(next);
        if let Some(previous_path) = previous_path {
            if previous_path != path { let _ = std::fs::remove_file(previous_path); }
        }
        self.last_error = None;
        tracing::info!("AERIS native playback started: {}", path.display());
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        let p = self.player.as_ref().context("Сейчас ничего не загружено")?;
        p.play();
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        let p = self.player.as_ref().context("Сейчас ничего не загружено")?;
        p.pause();
        Ok(())
    }

    pub fn toggle_pause(&mut self) -> Result<()> {
        let p = self.player.as_ref().context("Сейчас ничего не загружено")?;
        match p.state() {
            PlayerState::Playing | PlayerState::Buffering => p.pause(),
            PlayerState::Paused | PlayerState::Ended => p.play(),
            PlayerState::Loading => bail!("Медиапоток ещё загружается"),
            PlayerState::Error(error) => bail!("Ошибка медиадвижка: {error}"),
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.player.take().is_none() {
            bail!("Сейчас ничего не воспроизводится");
        }
        if let Some(path) = self.current_path.take() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        if !seconds.is_finite() || !(0.0..=86400.0).contains(&seconds) {
            bail!("Некорректная позиция перемотки");
        }
        let p = self.player.as_ref().context("Сейчас ничего не загружено")?;
        if !p.is_seekable() { bail!("Этот источник нельзя перематывать"); }
        p.seek(Duration::from_secs_f64(seconds));
        Ok(())
    }

    pub fn seek_relative(&mut self, offset: f64) -> Result<()> {
        if !offset.is_finite() || !(-3600.0..=3600.0).contains(&offset) {
            bail!("Некорректное смещение перемотки");
        }
        let position = self.player.as_ref().context("Сейчас ничего не загружено")?.position().as_secs_f64();
        self.seek((position + offset).max(0.0))
    }

    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            bail!("Громкость должна быть от 0 до 1");
        }
        self.player.as_ref().context("Сейчас ничего не загружено")?.set_volume(volume as f32);
        Ok(())
    }

    pub fn change_volume(&mut self, delta: f64) -> Result<()> {
        if !delta.is_finite() || !(-100.0..=100.0).contains(&delta) {
            bail!("Некорректное изменение громкости");
        }
        let current = self.player.as_ref().context("Сейчас ничего не загружено")?.volume() as f64;
        self.set_volume((current + delta / 100.0).clamp(0.0, 1.0))
    }

    pub fn playback_status(&mut self) -> Result<Value> {
        let Some(player) = self.player.as_ref() else {
            return Ok(json!({"idle":true,"position":0.0,"duration":0.0,"paused":true,"volume":0.8}));
        };
        let position = player.position().as_secs_f64();
        let duration = player.duration().map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let state = player.state();
        let metadata = player.metadata();
        let (title, artist, album) = metadata.map(|m| (
            m.title.unwrap_or_default(), m.artist.unwrap_or_default(), m.album.unwrap_or_default()
        )).unwrap_or_default();
        let paused = !matches!(state, PlayerState::Playing | PlayerState::Buffering);
        let error = match state { PlayerState::Error(e) => Some(e.to_string()), _ => None };
        Ok(json!({
            "idle":false,"position":position,"duration":duration,"paused":paused,
            "volume":player.volume(),"has_video":player.has_video(),"has_audio":player.has_audio(),
            "seekable":player.is_seekable(),"metadata_title":title,"metadata_artist":artist,
            "metadata_album":album,"path":self.current_path.as_ref().map(|p|p.to_string_lossy().to_string()),
            "error":error
        }))
    }

    pub fn current_frame(&self) -> Option<VideoFrame> {
        self.player.as_ref()?.current_frame()
    }

    pub fn poll_events(&mut self) -> Vec<String> {
        let events: Vec<PlayerEvent> = self.player.as_ref()
            .map(|player| player.events().try_iter().collect())
            .unwrap_or_default();
        let mut messages = Vec::new();
        let mut ended = false;
        for event in events {
            match event {
                PlayerEvent::Warning(message) => messages.push(message),
                PlayerEvent::Error(error) => {
                    let message = error.to_string();
                    self.last_error = Some(message.clone());
                    messages.push(message);
                }
                PlayerEvent::Ended => {
                    ended = true;
                    messages.push("Воспроизведение завершено".to_owned());
                },
                _ => {}
            }
        }
        if ended {
            // Drop decoder/audio/video buffers before deleting the source file.
            self.player.take();
            if let Some(path) = self.current_path.take() {
                let _ = std::fs::remove_file(path);
            }
        }
        messages
    }
}

impl Drop for AerisPlayer {
    fn drop(&mut self) {
        self.player.take();
        if let Some(path) = self.current_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
