use anyhow::{Context, Result, bail};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc::{self, Sender, SyncSender},
    thread,
    time::Duration,
};
use serde_json::Value;

enum Command {
    Play(PathBuf, String, SyncSender<std::result::Result<Value, String>>),
    Resume(SyncSender<std::result::Result<Value, String>>),
    Pause(SyncSender<std::result::Result<Value, String>>),
    Stop(SyncSender<std::result::Result<Value, String>>),
    Seek(f64, SyncSender<std::result::Result<Value, String>>),
    Volume(f32, SyncSender<std::result::Result<Value, String>>),
    Status(SyncSender<std::result::Result<Value, String>>),
}

#[derive(Clone)]
pub struct AudioPlayerHandle {
    sender: Sender<Command>,
}

impl AudioPlayerHandle {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("aeris-audio-engine".to_owned())
            .spawn(move || {
                let mut player = AudioPlayer::default();
                while let Ok(command) = receiver.recv() {
                    let (reply, result) = match command {
                        Command::Play(path, title, reply) => (reply, player.play_file(&path, &title).map(|_| player.status())),
                        Command::Resume(reply) => (reply, player.resume().map(|_| player.status())),
                        Command::Pause(reply) => (reply, player.pause().map(|_| player.status())),
                        Command::Stop(reply) => {
                            player.stop();
                            (reply, Ok(player.status()))
                        }
                        Command::Seek(seconds, reply) => (reply, player.seek(seconds).map(|_| player.status())),
                        Command::Volume(volume, reply) => (reply, player.set_volume(volume).map(|_| player.status())),
                        Command::Status(reply) => (reply, Ok(player.status())),
                    };
                    let _ = reply.send(result.map_err(|e| format!("{e:#}")));
                }
            });
        Self { sender }
    }

    fn request(&self, build: impl FnOnce(SyncSender<std::result::Result<Value, String>>) -> Command) -> Result<Value> {
        let (reply, response) = mpsc::sync_channel(1);
        self.sender.send(build(reply)).context("Аудиодвижок AERIS недоступен")?;
        response.recv().context("Аудиодвижок AERIS не ответил")?
            .map_err(|message| anyhow::anyhow!(message))
    }

    pub fn play_file(&self, path: &Path, title: &str) -> Result<Value> {
        let path = path.to_path_buf();
        let title = title.to_owned();
        self.request(|reply| Command::Play(path, title, reply))
    }
    pub fn resume(&self) -> Result<Value> { self.request(Command::Resume) }
    pub fn pause(&self) -> Result<Value> { self.request(Command::Pause) }
    pub fn stop(&self) -> Result<Value> { self.request(Command::Stop) }
    pub fn seek(&self, seconds: f64) -> Result<Value> { self.request(|reply| Command::Seek(seconds, reply)) }
    pub fn set_volume(&self, volume: f32) -> Result<Value> { self.request(|reply| Command::Volume(volume, reply)) }
    pub fn status(&self) -> Result<Value> { self.request(Command::Status) }
}

struct AudioPlayer {
    _stream: Option<OutputStream>,
    sink: Option<Sink>,
    title: String,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self { _stream: None, sink: None, title: String::new() }
    }
}

impl AudioPlayer {
    fn play_file(&mut self, path: &Path, title: &str) -> Result<()> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("Не удалось открыть аудиоустройство Windows")?;
        let file = File::open(path).context("Не удалось открыть аудиофайл")?;
        let decoder = Decoder::try_from(file)
            .context("Формат аудио не поддерживается встроенным плеером AERIS")?;
        let sink = Sink::connect_new(stream.mixer());
        sink.append(decoder);
        sink.play();
        self._stream = Some(stream);
        self.sink = Some(sink);
        self.title = title.to_owned();
        Ok(())
    }

    fn resume(&self) -> Result<()> {
        self.sink.as_ref().context("Сейчас ничего не загружено")?.play();
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        self.sink.as_ref().context("Сейчас ничего не загружено")?.pause();
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() { sink.stop(); }
        self._stream = None;
        self.title.clear();
    }

    fn seek(&self, seconds: f64) -> Result<()> {
        if !seconds.is_finite() || seconds < 0.0 { bail!("Позиция перемотки должна быть неотрицательным числом"); }
        self.sink.as_ref().context("Сейчас ничего не загружено")?
            .try_seek(Duration::from_secs_f64(seconds))
            .context("Не удалось перемотать аудио")?;
        Ok(())
    }

    fn set_volume(&self, volume: f32) -> Result<()> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) { bail!("Громкость должна быть от 0 до 1"); }
        self.sink.as_ref().context("Сейчас ничего не загружено")?.set_volume(volume);
        Ok(())
    }

    fn status(&self) -> Value {
        match self.sink.as_ref() {
            Some(sink) => serde_json::json!({
                "title": self.title,
                "state": if sink.empty() { "ended" } else if sink.is_paused() { "paused" } else { "playing" },
                "position_seconds": sink.get_pos().as_secs_f64(),
                "volume": sink.volume()
            }),
            None => serde_json::json!({"title": "", "state": "stopped", "position_seconds": 0.0, "volume": 1.0}),
        }
    }
}
