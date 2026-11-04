use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

enum Command {
    Play(PathBuf, Sender<Result<(), String>>),
    Resume(Sender<Result<(), String>>),
    Pause(Sender<Result<(), String>>),
    Stop(Sender<Result<(), String>>),
    Seek(f64, Sender<Result<(), String>>),
    Volume(f32, Sender<Result<(), String>>),
}

#[derive(Clone)]
pub struct PlayerHandle {
    tx: Sender<Command>,
}

impl Default for PlayerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let _ = thread::Builder::new()
            .name("aeris-audio-player".into())
            .spawn(move || player_worker(rx));
        Self { tx }
    }

    fn request(&self, make: impl FnOnce(Sender<Result<(), String>>) -> Command) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx.send(make(reply_tx)).map_err(|_| "Аудиоплеер AERIS завершил работу".to_string())?;
        reply_rx.recv_timeout(Duration::from_secs(8))
            .map_err(|_| "Аудиоплеер не ответил вовремя".to_string())?
    }

    pub fn play_file(&self, path: PathBuf) -> Result<(), String> {
        self.request(|reply| Command::Play(path, reply))
    }
    pub fn resume(&self) -> Result<(), String> {
        self.request(Command::Resume)
    }
    pub fn pause(&self) -> Result<(), String> {
        self.request(Command::Pause)
    }
    pub fn stop(&self) -> Result<(), String> {
        self.request(Command::Stop)
    }
    pub fn seek(&self, seconds: f64) -> Result<(), String> {
        self.request(|reply| Command::Seek(seconds, reply))
    }
    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        self.request(|reply| Command::Volume(volume, reply))
    }
}

fn player_worker(rx: mpsc::Receiver<Command>) {
    let mut output: Option<(OutputStream, OutputStreamHandle)> = None;
    let mut sink: Option<Sink> = None;
    let mut volume = 0.8_f32;

    while let Ok(command) = rx.recv() {
        let (reply, result) = match command {
            Command::Play(path, reply) => {
                let result = (|| {
                    let file = File::open(&path).map_err(|e| format!("Не удалось открыть аудио: {e}"))?;
                    let decoder = Decoder::new(BufReader::new(file))
                        .map_err(|e| format!("Формат аудио не поддерживается: {e}"))?;
                    if output.is_none() {
                        output = Some(OutputStream::try_default()
                            .map_err(|e| format!("Не удалось открыть аудиоустройство Windows: {e}"))?);
                    }
                    let handle = &output.as_ref().expect("output initialized").1;
                    let new_sink = Sink::try_new(handle)
                        .map_err(|e| format!("Не удалось создать аудиопоток: {e}"))?;
                    if let Some(old) = sink.take() { old.stop(); }
                    new_sink.set_volume(volume);
                    new_sink.append(decoder);
                    sink = Some(new_sink);
                    tracing::info!("AERIS audio playback started: {}", path.display());
                    Ok(())
                })();
                (reply, result)
            }
            Command::Resume(reply) => {
                let result = match sink.as_ref() {
                    Some(s) => { s.play(); Ok(()) }
                    None => Err("Сейчас ничего не загружено".into()),
                };
                (reply, result)
            }
            Command::Pause(reply) => {
                let result = match sink.as_ref() {
                    Some(s) => { s.pause(); Ok(()) }
                    None => Err("Сейчас ничего не воспроизводится".into()),
                };
                (reply, result)
            }
            Command::Stop(reply) => {
                let result = match sink.take() {
                    Some(s) => { s.stop(); Ok(()) }
                    None => Err("Сейчас ничего не воспроизводится".into()),
                };
                (reply, result)
            }
            Command::Seek(seconds, reply) => {
                let result = match sink.as_ref() {
                    Some(s) if seconds.is_finite() && seconds >= 0.0 && seconds <= 86_400.0 =>
                        s.try_seek(Duration::from_secs_f64(seconds))
                            .map_err(|e| format!("Перемотка не поддерживается этим файлом: {e}")),
                    Some(_) => Err("Некорректная позиция перемотки".into()),
                    None => Err("Сейчас ничего не воспроизводится".into()),
                };
                (reply, result)
            }
            Command::Volume(value, reply) => {
                let result = if value.is_finite() && (0.0..=1.0).contains(&value) {
                    volume = value;
                    if let Some(s) = sink.as_ref() { s.set_volume(volume); }
                    Ok(())
                } else {
                    Err("Громкость должна быть от 0 до 1".into())
                };
                (reply, result)
            }
        };
        let _ = reply.send(result);
    }
}
