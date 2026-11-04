use anyhow::{Context, Result, bail};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::{fs::File, io::BufReader, path::Path, time::Duration};

pub struct AudioPlayer {
    stream: Option<OutputStream>,
    sink: Option<Sink>,
    title: String,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self { stream: None, sink: None, title: String::new() }
    }
}

impl AudioPlayer {
    pub fn play_file(&mut self, path: &Path, title: &str) -> Result<()> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("Не удалось открыть аудиоустройство Windows")?;
        let file = File::open(path).context("Не удалось открыть аудиофайл")?;
        let decoder = Decoder::try_from(BufReader::new(file))
            .context("Формат аудио не поддерживается встроенным плеером AERIS")?;
        let sink = Sink::connect_new(stream.mixer());
        sink.append(decoder);
        sink.play();
        self.stream = Some(stream);
        self.sink = Some(sink);
        self.title = title.to_owned();
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        let sink = self.sink.as_ref().context("Сейчас ничего не загружено")?;
        sink.play();
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        let sink = self.sink.as_ref().context("Сейчас ничего не загружено")?;
        sink.pause();
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() { sink.stop(); }
        self.stream = None;
        self.title.clear();
    }

    pub fn seek(&self, seconds: f64) -> Result<()> {
        if !seconds.is_finite() || seconds < 0.0 { bail!("Позиция перемотки должна быть неотрицательным числом"); }
        let sink = self.sink.as_ref().context("Сейчас ничего не загружено")?;
        sink.try_seek(Duration::from_secs_f64(seconds))
            .context("Не удалось перемотать аудио")?;
        Ok(())
    }

    pub fn set_volume(&self, volume: f32) -> Result<()> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) { bail!("Громкость должна быть от 0 до 1"); }
        let sink = self.sink.as_ref().context("Сейчас ничего не загружено")?;
        sink.set_volume(volume);
        Ok(())
    }

    pub fn status(&self) -> serde_json::Value {
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
