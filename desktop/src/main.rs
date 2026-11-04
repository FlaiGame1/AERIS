#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod aeris_protocol;
mod mpv_player;

use eframe::egui;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use tokio::sync::Notify;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab { Player, Connection }

struct AerisDesktop {
    receiver_thread: Option<JoinHandle<()>>,
    shutdown: Option<Arc<Notify>>,
    ready_rx: Option<mpsc::Receiver<String>>,
    status: String,
    device_name: String,
    tab: Tab,
    pairing_code: String,
    lan_address: String,
    track_title: String,
    track_artist: String,
    connected_device: String,
    playback_position: f64,
    playback_duration: f64,
    cover_path: String,
    cover_loaded_path: String,
    cover_texture: Option<egui::TextureHandle>,
    background_texture: Option<egui::TextureHandle>,
}

impl Default for AerisDesktop {
    fn default() -> Self {
        Self {
            receiver_thread: None,
            shutdown: None,
            ready_rx: None,
            status: "Приёмник остановлен".to_owned(),
            device_name: "AERIS Desktop".to_owned(),
            tab: Tab::Player,
            pairing_code: String::new(),
            lan_address: String::new(),
            track_title: "Ожидание воспроизведения".to_owned(),
            track_artist: "AERIS • Desktop player".to_owned(),
            connected_device: String::new(),
            playback_position: 0.0,
            playback_duration: 0.0,
            cover_path: String::new(),
            cover_loaded_path: String::new(),
            cover_texture: None,
            background_texture: None,
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
                let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build();
                match runtime {
                    Ok(runtime) => {
                        if let Err(error) = runtime.block_on(aeris_protocol::run_server(thread_shutdown, ready_tx)) {
                            let _ = startup_error_tx.send(format!("Ошибка AERIS Protocol: {error:#}"));
                        }
                    }
                    Err(error) => { let _ = startup_error_tx.send(format!("Не удалось запустить runtime: {error}")); }
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
        if let Some(shutdown) = self.shutdown.take() { shutdown.notify_one(); }
        if let Some(thread) = self.receiver_thread.take() {
            if thread.join().is_err() { self.status = "Поток приёмника завершился с ошибкой".to_owned(); }
            else { self.status = "Приёмник остановлен".to_owned(); }
        } else { self.status = "Приёмник остановлен".to_owned(); }
        self.ready_rx = None;
        self.pairing_code.clear();
        self.lan_address.clear();
    }

    fn check_receiver(&mut self) {
        if let Some(rx) = &self.ready_rx {
            loop {
                match rx.try_recv() {
                    Ok(message) => {
                        if let Some(path) = message.strip_prefix("COVER_PATH:") {
                            self.cover_path = path.to_owned();
                            continue;
                        }
                        if let Some(payload) = message.strip_prefix("PLAYBACK:") {
                            if let Ok(state) = serde_json::from_str::<serde_json::Value>(payload) {
                                self.playback_position = state.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
                                self.playback_duration = state.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
                                if let Some(title) = state.get("metadata_title").and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()) {
                                    if self.track_title == "Ожидание воспроизведения" { self.track_title = title.to_owned(); }
                                }
                                if let Some(artist) = state.get("metadata_artist").and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()) {
                                    if self.track_artist == "AERIS • Desktop player" { self.track_artist = artist.to_owned(); }
                                }
                            }
                            continue;
                        }
                        if let Some(device) = message.strip_prefix("Подключено новое устройство: ") {
                            self.connected_device = device.to_owned();
                        } else if let Some(device) = message.strip_prefix("Повторно подключено устройство: ") {
                            self.connected_device = device.to_owned();
                        }
                        if let Some(line) = message.lines().find(|line| line.starts_with("Код подключения:")) {
                            self.pairing_code = line.trim_start_matches("Код подключения:").trim().to_owned();
                        }
                        if let Some(line) = message.lines().find(|line| line.starts_with("Адрес:")) {
                            self.lan_address = line.trim_start_matches("Адрес:").trim().to_owned();
                        }
                        if let Some(name) = message.strip_prefix("Воспроизведение: ") {
                            self.cover_path.clear();
                            self.cover_loaded_path.clear();
                            self.cover_texture = None;
                            self.background_texture = None;
                            let mut parts = name.splitn(2, " · ");
                            self.track_title = parts.next().unwrap_or(name).to_owned();
                            self.track_artist = parts.next().unwrap_or("Неизвестный исполнитель").to_owned();
                            self.playback_position = 0.0;
                        }
                        self.status = message;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }
        let finished = self.receiver_thread.as_ref().map(JoinHandle::is_finished).unwrap_or(false);
        if finished {
            if let Some(thread) = self.receiver_thread.take() {
                if thread.join().is_err() { self.status = "Приёмник аварийно завершился".to_owned(); }
            }
            self.shutdown = None;
            self.ready_rx = None;
        }
    }

    fn player_tab(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_width().min(650.0);
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("NOW PLAYING").size(10.0).strong().color(egui::Color32::from_rgb(205, 183, 236)));
            ui.add_space(18.0);
            let side = (available - 64.0).min(390.0).max(190.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
            let painter = ui.painter();
            if let Some(texture) = &self.cover_texture {
                painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            } else {
                painter.rect_filled(rect, 28.0, egui::Color32::from_rgb(71, 52, 104));
                painter.rect_filled(egui::Rect::from_min_max(rect.min + egui::vec2(0.0, side * 0.52), rect.max), 28.0, egui::Color32::from_rgb(37, 63, 83));
                painter.circle_filled(rect.center(), side * 0.245, egui::Color32::from_rgb(28, 29, 42));
                painter.circle_filled(rect.center(), side * 0.16, egui::Color32::from_rgb(105, 85, 133));
                painter.circle_filled(rect.center(), side * 0.035, egui::Color32::from_rgb(231, 215, 246));
                painter.text(rect.center() + egui::vec2(0.0, side * 0.34), egui::Align2::CENTER_CENTER, "AERIS", egui::FontId::proportional(15.0), egui::Color32::from_white_alpha(190));
            }
            ui.add_space(18.0);

            let title = if self.track_title.is_empty() { "Ожидание воспроизведения" } else { &self.track_title };
            ui.label(egui::RichText::new(title).size(22.0).strong().color(egui::Color32::from_rgb(250, 247, 255)));
            ui.add_space(5.0);
            ui.label(egui::RichText::new(&self.track_artist).size(14.0).color(egui::Color32::from_rgb(190, 181, 204)));
            ui.add_space(22.0);

            let (timeline, response) = ui.allocate_exact_size(egui::vec2(side, 22.0), egui::Sense::hover());
            let line_y = timeline.center().y;
            let progress = if self.playback_duration > 0.0 { (self.playback_position / self.playback_duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
            let radius = 7.0;
            let track_left = timeline.left() + radius;
            let track_right = timeline.right() - radius;
            let track_width = (track_right - track_left).max(1.0);
            let knob_x = track_left + track_width * progress;
            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(track_left, line_y - 2.0), egui::pos2(track_right, line_y + 2.0)), 3.0, egui::Color32::from_rgb(91, 82, 107));
            if progress > 0.0 {
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(track_left, line_y - 2.0), egui::pos2(knob_x, line_y + 2.0)), 3.0, egui::Color32::from_rgb(194, 164, 235));
            }
            ui.painter().circle_filled(egui::pos2(knob_x, line_y), radius, egui::Color32::from_rgb(235, 218, 255));
            if response.hovered() {
                if let Some(pointer) = response.hover_pos() {
                    let x = pointer.x.clamp(track_left, track_right);
                    let bubble = egui::Rect::from_center_size(egui::pos2(x, timeline.top() - 13.0), egui::vec2(52.0, 22.0));
                    ui.painter().rect_filled(bubble, 7.0, egui::Color32::from_rgb(232, 218, 249));
                    let hovered_seconds = if self.playback_duration > 0.0 { ((x - track_left) / track_width * self.playback_duration as f32) as u64 } else { 0 };
                    let stamp = format!("{:02}:{:02}", hovered_seconds / 60, hovered_seconds % 60);
                    ui.painter().text(bubble.center(), egui::Align2::CENTER_CENTER, stamp, egui::FontId::proportional(11.0), egui::Color32::from_rgb(42, 32, 56));
                }
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("{:02}:{:02}", self.playback_position as u64 / 60, self.playback_position as u64 % 60)).size(11.0).color(egui::Color32::from_rgb(190,181,204)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(if self.playback_duration > 0.0 { format!("{:02}:{:02}", self.playback_duration as u64 / 60, self.playback_duration as u64 % 60) } else { "--:--".to_owned() }).size(11.0).color(egui::Color32::from_rgb(190,181,204)));
                });
            });
            ui.add_space(18.0);
            ui.label(egui::RichText::new("Передай трек с телефона через плагин AERIS").size(12.0).color(egui::Color32::from_rgb(177,166,194)));
        });
    }

    fn connection_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        ui.label(egui::RichText::new("Подключение телефона").size(24.0).strong().color(egui::Color32::from_rgb(250,247,255)));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Запусти приёмник, затем открой плагин AERIS в AyuGram или exteraGram.").size(13.0).color(egui::Color32::from_rgb(190,181,204)));
        ui.add_space(22.0);

        let running = self.receiver_thread.is_some();
        let accent = egui::Color32::from_rgb(194, 164, 235);
        let button = if running { "Остановить приёмник" } else { "Запустить приёмник" };
        if ui.add_sized([ui.available_width(), 48.0], egui::Button::new(egui::RichText::new(button).strong().color(egui::Color32::from_rgb(36,27,48))).fill(accent).rounding(egui::Rounding::same(15.0))).clicked() {
            if running { self.stop_receiver(); } else { self.start_receiver(); }
        }
        ui.add_space(18.0);

        egui::Frame::none().fill(egui::Color32::from_rgb(38,34,49)).rounding(egui::Rounding::same(18.0)).inner_margin(egui::Margin::same(20.0)).show(ui, |ui| {
            ui.label(egui::RichText::new("КОД СОПРЯЖЕНИЯ").size(10.0).strong().color(egui::Color32::from_rgb(190,181,204)));
            ui.add_space(8.0);
            let code = if self.pairing_code.is_empty() { "••••••" } else { &self.pairing_code };
            ui.label(egui::RichText::new(code).size(34.0).strong().monospace().color(egui::Color32::WHITE));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(if running { "Введи этот код в настройках подключения плагина." } else { "Код появится после запуска приёмника." }).size(12.0).color(egui::Color32::from_rgb(190,181,204)));
            if !self.lan_address.is_empty() {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("АДРЕС").size(10.0).strong().color(egui::Color32::from_rgb(190,181,204)));
                ui.label(egui::RichText::new(&self.lan_address).size(13.0).monospace().color(egui::Color32::WHITE));
            }
        });
        ui.add_space(18.0);
        ui.label(egui::RichText::new("Состояние").size(12.0).strong().color(egui::Color32::from_rgb(190,181,204)));
        ui.label(egui::RichText::new(&self.status).size(12.0).color(egui::Color32::from_rgb(230,224,238)));
        if !self.connected_device.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(format!("Подключено устройство: {}", self.connected_device)).size(13.0).strong().color(egui::Color32::from_rgb(190, 230, 202)));
        }
        ui.add_space(12.0);
        ui.label(egui::RichText::new("После сопряжения выбери в плагине сообщение с музыкой или видео и отправь его на AERIS. Полная синхронизация пока не включена.").size(12.0).color(egui::Color32::from_rgb(165,155,181)));
    }
}

impl Drop for AerisDesktop {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() { shutdown.notify_one(); }
        if let Some(thread) = self.receiver_thread.take() { let _ = thread.join(); }
    }
}

impl eframe::App for AerisDesktop {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_receiver();
        if !self.cover_path.is_empty() && self.cover_path != self.cover_loaded_path {
            if let Ok(bytes) = std::fs::read(&self.cover_path) {
                if let Ok(decoded) = image::load_from_memory(&bytes) {
                    let cover = decoded.resize_to_fill(720, 720, image::imageops::FilterType::Lanczos3).to_rgba8();
                    let cover_color = egui::ColorImage::from_rgba_unmultiplied(
                        [cover.width() as usize, cover.height() as usize], cover.as_raw()
                    );
                    self.cover_texture = Some(ctx.load_texture("aeris-track-cover", cover_color, egui::TextureOptions::LINEAR));

                    let blurred = image::imageops::blur(&decoded.resize_to_fill(1000, 1000, image::imageops::FilterType::Triangle), 28.0);
                    let background_color = egui::ColorImage::from_rgba_unmultiplied(
                        [blurred.width() as usize, blurred.height() as usize], blurred.as_raw()
                    );
                    self.background_texture = Some(ctx.load_texture("aeris-blurred-background", background_color, egui::TextureOptions::LINEAR));
                    self.cover_loaded_path = self.cover_path.clone();
                }
            }
        }
        let bg = egui::Color32::from_rgb(24, 22, 32);
        let muted = egui::Color32::from_rgb(173, 163, 190);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(bg).inner_margin(egui::Margin::same(24.0))).show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            if let Some(texture) = &self.background_texture {
                ui.painter().image(texture.id(), panel_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::from_white_alpha(72));
                ui.painter().rect_filled(panel_rect, 0.0, egui::Color32::from_black_alpha(155));
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("AERIS").size(17.0).strong().color(egui::Color32::from_rgb(216,190,247)));
                ui.label(egui::RichText::new("MATERIAL 3").size(10.0).strong().color(muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let state = if !self.connected_device.is_empty() { "● УСТРОЙСТВО ПОДКЛЮЧЕНО" } else if self.receiver_thread.is_some() { "● ПРИЁМНИК АКТИВЕН" } else { "○ НЕ ПОДКЛЮЧЕНО" };
                    ui.label(egui::RichText::new(state).size(10.0).strong().color(muted));
                });
            });
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                let player = ui.selectable_label(self.tab == Tab::Player, egui::RichText::new("Плеер").size(14.0).strong());
                if player.clicked() { self.tab = Tab::Player; }
                let connection = ui.selectable_label(self.tab == Tab::Connection, egui::RichText::new("Подключение").size(14.0).strong());
                if connection.clicked() { self.tab = Tab::Connection; }
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            match self.tab {
                Tab::Player => self.player_tab(ui),
                Tab::Connection => self.connection_tab(ui),
            }
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("AERIS {}  •  Material 3 preview", env!("CARGO_PKG_VERSION"))).size(10.0).color(muted));
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
        viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 760.0]).with_min_inner_size([420.0, 600.0]).with_title("AERIS — Material 3"),
        ..Default::default()
    };
    eframe::run_native("AERIS — Material 3", options, Box::new(|_cc| Ok(Box::new(AerisDesktop::default()))))
}
