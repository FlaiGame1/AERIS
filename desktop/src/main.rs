#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod aeris_protocol;
mod mpv_player;

use eframe::egui;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use tokio::sync::Notify;

struct AerisDesktop {
    receiver_thread: Option<JoinHandle<()>>,
    shutdown: Option<Arc<Notify>>,
    ready_rx: Option<mpsc::Receiver<String>>,
    status: String,
    device_name: String,
}

impl Default for AerisDesktop {
    fn default() -> Self {
        Self {
            receiver_thread: None,
            shutdown: None,
            ready_rx: None,
            status: "Приёмник остановлен".to_owned(),
            device_name: "AERIS Desktop — этот компьютер".to_owned(),
        }
    }
}

impl AerisDesktop {
    fn start_receiver(&mut self) {
        let shutdown = Arc::new(Notify::new());
        let thread_shutdown = shutdown.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let startup_error_tx = ready_tx.clone();

        let thread = thread::Builder::new()
            .name("aeris-protocol-server".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => {
                        if let Err(error) = runtime.block_on(aeris_protocol::run_server(thread_shutdown, ready_tx)) {
                            let _ = startup_error_tx.send(format!("Ошибка AERIS Protocol: {error:#}"));
                        }
                    }
                    Err(error) => {
                        let _ = startup_error_tx.send(format!("Не удалось запустить runtime: {error}"));
                    }
                }
            });

        match thread {
            Ok(handle) => {
                self.shutdown = Some(shutdown);
                self.ready_rx = Some(ready_rx);
                self.receiver_thread = Some(handle);
                self.status = "Запускаем AERIS Protocol…".to_owned();
            }
            Err(error) => self.status = format!("Не удалось создать поток: {error}"),
        }
    }

    fn stop_receiver(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.notify_one();
        }
        if let Some(thread) = self.receiver_thread.take() {
            if thread.join().is_err() {
                self.status = "Поток приёмника завершился с ошибкой".to_owned();
            } else {
                self.status = "Приёмник остановлен".to_owned();
            }
        } else {
            self.status = "Приёмник остановлен".to_owned();
        }
        self.ready_rx = None;
    }

    fn check_receiver(&mut self) {
        if let Some(rx) = &self.ready_rx {
            loop {
                match rx.try_recv() {
                    Ok(message) => self.status = message,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }

        let finished = self.receiver_thread.as_ref().map(JoinHandle::is_finished).unwrap_or(false);
        if finished {
            if let Some(thread) = self.receiver_thread.take() {
                if thread.join().is_err() {
                    self.status = "Приёмник аварийно завершился".to_owned();
                } else if self.status.starts_with("Приёмник запущен") {
                    self.status = "Приёмник остановлен".to_owned();
                }
            }
            self.shutdown = None;
            self.ready_rx = None;
        }
    }
}

impl Drop for AerisDesktop {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown.notify_one();
        }
        if let Some(thread) = self.receiver_thread.take() {
            let _ = thread.join();
        }
    }
}

impl eframe::App for AerisDesktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_receiver();
        let bg = egui::Color32::from_rgb(18, 25, 35);
        let panel = egui::Color32::from_rgb(27, 37, 50);
        let accent = egui::Color32::from_rgb(42, 155, 220);
        let muted = egui::Color32::from_rgb(155, 174, 193);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg).inner_margin(egui::Margin::same(24.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("AERIS").size(15.0).strong().color(accent));
                    ui.heading(egui::RichText::new("DESKTOP").size(30.0).strong().color(egui::Color32::WHITE));
                    ui.label(egui::RichText::new("Транслируй. Слушай. Смотри.").size(14.0).color(muted));
                });

                ui.add_space(28.0);
                egui::Frame::none().fill(panel).rounding(egui::Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(20.0)).show(ui, |ui| {
                        ui.label(egui::RichText::new("УСТРОЙСТВО").size(11.0).strong().color(muted));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(&self.device_name).size(18.0).strong().color(egui::Color32::WHITE));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(&self.status).size(13.0).color(muted));
                        ui.add_space(18.0);
                        let running = self.receiver_thread.is_some();
                        let button_text = if running { "Остановить сервер" } else { "Запустить сервер" };
                        if ui.add_sized([ui.available_width(), 46.0],
                            egui::Button::new(egui::RichText::new(button_text).size(15.0).strong())
                                .fill(accent).rounding(egui::Rounding::same(10.0))).clicked() {
                            if running { self.stop_receiver(); } else { self.start_receiver(); }
                        }
                    });

                ui.add_space(18.0);
                egui::Frame::none().fill(panel).rounding(egui::Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(18.0)).show(ui, |ui| {
                        ui.label(egui::RichText::new("КАК ПОДКЛЮЧИТЬСЯ").size(11.0).strong().color(muted));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("01").color(accent).strong());
                            ui.label(egui::RichText::new("Подключи телефон и ПК к одной Wi-Fi сети").color(egui::Color32::WHITE));
                        });
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("02").color(accent).strong());
                            ui.label(egui::RichText::new("Введи адрес и код подключения в плагине").color(egui::Color32::WHITE));
                        });
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("03").color(accent).strong());
                            ui.label(egui::RichText::new("После первого сопряжения телефон запоминается").color(egui::Color32::WHITE));
                        });
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("04").color(accent).strong());
                            ui.label(egui::RichText::new("Если Telegram недоступен — открой браузерную загрузку из блока устройства").color(egui::Color32::WHITE));
                        });
                    });
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("AERIS Protocol v1 • WebSocket • LAN").size(11.0).color(muted));
                    ui.label(egui::RichText::new("AERIS 0.6.0 • mpv + браузерная загрузка").size(10.0).color(muted));
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn main() -> eframe::Result<()> {
    if let Ok(file) = std::fs::File::create(std::env::temp_dir().join("aeris-desktop.log")) {
        let _ = tracing_subscriber::fmt().with_writer(file).with_ansi(false).try_init();
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 640.0])
            .with_min_inner_size([400.0, 560.0])
            .with_title("AERIS Desktop"),
        ..Default::default()
    };
    eframe::run_native("AERIS Desktop", options, Box::new(|_cc| Ok(Box::new(AerisDesktop::default()))))
}
