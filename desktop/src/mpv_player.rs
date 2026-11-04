use anyhow::{Context, Result, bail};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, net::windows::named_pipe::{ClientOptions, NamedPipeClient}, time::{sleep, timeout}};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const IPC_TIMEOUT: Duration = Duration::from_secs(3);

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

    async fn connect(&mut self) -> Result<NamedPipeClient> {
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
            let log_path = std::env::temp_dir().join("AERIS").join("mpv.log");
            if let Some(parent) = log_path.parent() {
                std::fs::create_dir_all(parent).context("Не удалось создать папку логов AERIS")?;
            }
            command
                .arg(format!("--log-file={}", log_path.to_string_lossy()))
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

    async fn command(&mut self, command: Value) -> Result<Value> {
        let mut pipe = self.connect().await?;
        let mut message = serde_json::to_vec(&json!({
            "command": command,
            "request_id": 1
        }))?;
        message.push(b'\n');
        pipe.write_all(&message).await.context("Не удалось отправить команду медиаплееру")?;
        pipe.flush().await.context("Не удалось завершить отправку команды медиаплееру")?;

        // A successful pipe write only proves that bytes reached the pipe.
        // Read mpv's IPC acknowledgement so a rejected loadfile/control command
        // is reported to the phone instead of being silently treated as success.
        timeout(IPC_TIMEOUT, async {
            let mut reader = BufReader::new(pipe);
            loop {
                let mut line = String::new();
                let count = reader.read_line(&mut line).await?;
                if count == 0 {
                    bail!("mpv закрыл IPC-канал без ответа");
                }
                let response: Value = serde_json::from_str(&line)
                    .context("mpv вернул некорректный ответ IPC")?;
                // mpv may emit asynchronous event messages on the same pipe.
                // Ignore them and wait for the acknowledgement for our request.
                if response.get("request_id").and_then(Value::as_u64) != Some(1) {
                    continue;
                }
                let error = response.get("error").and_then(Value::as_str).unwrap_or("unknown");
                if error != "success" {
                    bail!("mpv отклонил команду: {error}");
                }
                return Ok::<Value, anyhow::Error>(response.get("data").cloned().unwrap_or(Value::Null));
            }
        }).await.context("mpv не ответил на команду за 3 секунды")?
    }

    async fn command_ok(&mut self, command: Value) -> Result<()> {
        self.command(command).await.map(|_| ())
    }

    async fn property(&mut self, name: &str) -> Result<Value> {
        self.command(json!(["get_property", name])).await
    }

    pub async fn playback_status(&mut self) -> Result<Value> {
        let path = self.property("path").await.unwrap_or(Value::Null);
        let title = self.property("media-title").await.unwrap_or(Value::Null);
        let metadata = self.property("metadata").await.unwrap_or(Value::Null);
        let metadata_title = metadata.get("title").cloned().unwrap_or(Value::Null);
        let metadata_artist = metadata.get("artist").cloned().or_else(|| metadata.get("performer").cloned()).unwrap_or(Value::Null);
        let duration = self.property("duration").await.unwrap_or(Value::Null);
        let position = self.property("time-pos").await.unwrap_or(Value::Null);
        let paused = self.property("pause").await.unwrap_or(Value::Bool(false));
        let volume = self.property("volume").await.unwrap_or(Value::Null);
        let idle = self.property("idle-active").await.unwrap_or(Value::Bool(true));
        Ok(json!({"path":path,"title":title,"metadata_title":metadata_title,"metadata_artist":metadata_artist,"duration":duration,"position":position,"paused":paused,"volume":volume,"idle":idle}))
    }

    pub async fn play_file(&mut self, path: &Path) -> Result<()> {
        let path = path.to_string_lossy().to_string();
        self.command_ok(json!(["loadfile", path, "replace"])).await
    }

    pub async fn play(&mut self) -> Result<()> {
        self.command_ok(json!(["set_property", "pause", false])).await
    }

    pub async fn pause(&mut self) -> Result<()> {
        self.command_ok(json!(["set_property", "pause", true])).await
    }

    pub async fn toggle_pause(&mut self) -> Result<()> {
        self.command_ok(json!(["cycle", "pause"])).await
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.command_ok(json!(["stop"])).await
    }

    pub async fn seek(&mut self, seconds: f64) -> Result<()> {
        self.command_ok(json!(["seek", seconds, "absolute"])).await
    }

    pub async fn seek_relative(&mut self, seconds: f64) -> Result<()> {
        self.command_ok(json!(["seek", seconds, "relative"])).await
    }

    pub async fn set_volume(&mut self, percent: f64) -> Result<()> {
        self.command_ok(json!(["set_property", "volume", percent])).await
    }

    pub async fn change_volume(&mut self, delta: f64) -> Result<()> {
        self.command_ok(json!(["add", "volume", delta])).await
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

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
