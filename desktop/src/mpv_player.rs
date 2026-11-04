use anyhow::{Context, Result, bail};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::windows::named_pipe::{Client, ClientOptions}, time::sleep};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct MpvPlayer {
    child: Option<Child>,
    pipe_path: String,
    exe_path: PathBuf,
}

impl MpvPlayer {
    pub fn new() -> Self {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let pipe_path = format!(r"\\.\pipe\aeris-mpv-{id}");
        Self { child: None, pipe_path, exe_path: locate_mpv() }
    }

    async fn connect(&mut self) -> Result<Client> {
        let exited = self.child.as_mut()
            .map(|child| child.try_wait().map(|status| status.is_some()))
            .transpose()?
            .unwrap_or(false);
        if exited {
            self.child.take();
        }

        if self.child.is_none() {
            if !self.exe_path.is_file() {
                bail!("Не найден mpv.exe. Проверь, что папка mpv находится рядом с AERIS Desktop.");
            }
            let mut command = Command::new(&self.exe_path);
            command
                .arg("--no-config")
                .arg("--idle=yes")
                .arg("--force-window=no")
                .arg("--terminal=no")
                .arg("--input-terminal=no")
                .arg("--keep-open=no")
                .arg("--title=AERIS Player")
                .arg(format!("--input-ipc-server={}", self.pipe_path))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(windows)]
            command.creation_flags(CREATE_NO_WINDOW);
            self.child = Some(command.spawn().with_context(|| {
                format!("Не удалось запустить медиадвижок: {}", self.exe_path.display())
            })?);
        }

        let mut last_error = None;
        for _ in 0..50 {
            match ClientOptions::new().open(&self.pipe_path) {
                Ok(pipe) => return Ok(pipe),
                Err(error) => {
                    last_error = Some(error);
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
        bail!("Не удалось подключиться к mpv через локальный канал: {}", last_error.map(|e| e.to_string()).unwrap_or_else(|| "тайм-аут".into()));
    }

    async fn command(&mut self, command: serde_json::Value) -> Result<()> {
        let mut pipe = self.connect().await?;
        let mut message = serde_json::to_vec(&json!({ "command": command }))?;
        message.push(b'\n');
        pipe.write_all(&message).await.context("Не удалось отправить команду медиаплееру")?;
        pipe.flush().await.context("Не удалось завершить отправку команды медиаплееру")?;
        Ok(())
    }

    pub async fn play_file(&mut self, path: &Path) -> Result<()> {
        let path = path.to_string_lossy().to_string();
        self.command(json!(["loadfile", path, "replace"])).await
    }

    pub async fn play(&mut self) -> Result<()> {
        self.command(json!(["set_property", "pause", false])).await
    }

    pub async fn pause(&mut self) -> Result<()> {
        self.command(json!(["set_property", "pause", true])).await
    }

    pub async fn toggle_pause(&mut self) -> Result<()> {
        self.command(json!(["cycle", "pause"])).await
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.command(json!(["stop"])).await
    }

    pub async fn seek(&mut self, seconds: f64) -> Result<()> {
        self.command(json!(["seek", seconds, "absolute"])).await
    }

    pub async fn seek_relative(&mut self, seconds: f64) -> Result<()> {
        self.command(json!(["seek", seconds, "relative"])).await
    }

    pub async fn set_volume(&mut self, percent: f64) -> Result<()> {
        self.command(json!(["set_property", "volume", percent])).await
    }

    pub async fn change_volume(&mut self, delta: f64) -> Result<()> {
        self.command(json!(["add", "volume", delta])).await
    }
}

fn locate_mpv() -> PathBuf {
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let directory = executable.parent().unwrap_or(Path::new("."));
    let candidates = [
        directory.join("mpv").join("mpv.exe"),
        directory.join("mpv.exe"),
    ];
    candidates.into_iter().find(|path| path.is_file()).unwrap_or_else(|| directory.join("mpv").join("mpv.exe"))
}
