#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod aeris_protocol;
mod mpv_player;

use eframe::egui;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use tokio::sync::Notify;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Player, Connect }

struct AerisDesktop {
    receiver_thread: Option<JoinHandle<()>>,
    shutdown: Option<Arc<Notify>>,
    ready_rx: Option<mpsc::Receiver<String>>,
    status: String,
    device_name: String,
    page: Page,
    demo_position: f32,
}

impl Default for AerisDesktop {
    fn default() -> Self {
        Self {
            receiver_thread: None,
            shutdown: None,
            ready_rx: None,
            status: "Приёмник остановлен".to_owned(),
            device_name: "AERIS Desktop".to_owned(),
            page: Page::Player,
            demo_position: 0.0,
        }
    }
}

impl AerisDesktop {
    fn start_receiver(&mut self) {
        let shutdown = Arc::new(Notify::new());
        let thread_shutdown = shutdown.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let startup_error_tx = ready_tx.clone();
        let thread = thread::Builder::new().name("aeris-protocol-server".to_owned()).spawn(move || {
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
            if thread.join().is_err() { self.status = "Поток приёмника завершился с ошибкой".into(); }
            else { self.status = "Приёмник остановлен".into(); }
        } else { self.status = "Приёмник остановлен".into(); }
        self.ready_rx = None;
    }

    fn check_receiver(&mut self) {
        if let Some(rx) = &self.ready_rx {
            loop {
                match rx.try_recv() {
                    Ok(message) => self.status = message,
                    Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => break,
                }
            }
        }
        if self.receiver_thread.as_ref().map(JoinHandle::is_finished).unwrap_or(false) {
            if let Some(thread) = self.receiver_thread.take() {
                if thread.join().is_err() { self.status = "Приёмник аварийно завершился".into(); }
            }
            self.shutdown = None;
            self.ready_rx = None;
        }
    }

    fn status_value(&self, key: &str) -> String {
        self.status.lines().find_map(|line| line.strip_prefix(key).map(str::trim).map(str::to_owned)).unwrap_or_else(|| "—".into())
    }

    fn timestamp(seconds: f32) -> String {
        let total = seconds.max(0.0) as u32;
        format!("{:02}:{:02}", total / 60, total % 60)
    }

    fn player_page(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_width().min(560.0);
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("A E R I S").size(13.0).strong().color(egui::Color32::from_rgb(226, 205, 255)));
            ui.add_space(12.0);
            let side = (available - 36.0).min(ui.available_height() - 180.0).clamp(180.0, 420.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 28.0, egui::Color32::from_rgb(77, 53, 105));
            painter.rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(side * 0.12, side * 0.12), egui::vec2(side * 0.76, side * 0.76)), 24.0, egui::Color32::from_rgb(130, 88, 147));
            painter.circle_filled(rect.center() + egui::vec2(side * 0.16, -side * 0.13), side * 0.27, egui::Color32::from_rgba_unmultiplied(239, 175, 202, 190));
            painter.circle_filled(rect.center() + egui::vec2(-side * 0.17, side * 0.18), side * 0.22, egui::Color32::from_rgba_unmultiplied(69, 43, 102, 210));
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "AERIS", egui::FontId::proportional(side * 0.11), egui::Color32::WHITE);
            ui.add_space(20.0);
            ui.set_width(available);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("00:00").size(11.0).color(egui::Color32::from_rgb(236, 221, 246)));
                let response = ui.add_sized([ui.available_width(), 18.0], egui::Slider::new(&mut self.demo_position, 0.0..=240.0).show_value(false));
                if response.hovered() {
                    if let Some(pointer) = response.hover_pos() {
                        let fraction = ((pointer.x - response.rect.left()) / response.rect.width()).clamp(0.0, 1.0);
                        let hovered = fraction * 240.0;
                        ui.painter().text(egui::pos2(response.rect.left(), response.rect.top() - 8.0), egui::Align2::LEFT_BOTTOM, Self::timestamp(hovered), egui::FontId::proportional(12.0), egui::Color32::WHITE);
                    }
                }
            });
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Ожидаем воспроизведение").size(23.0).strong().color(egui::Color32::WHITE));
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Подключи телефон через вкладку «Подключение»").size(14.0).color(egui::Color32::from_rgb(204, 190, 218)));
        });
    }

    fn connect_page(&mut self, ui: &mut egui::Ui) {
        let running = self.receiver_thread.is_some();
        let address = self.status_value("Адрес:");
        let code = self.status_value("Код подключения:");
        let panel = egui::Color32::from_rgb(47, 37, 61);
        ui.add_space(22.0);
        ui.label(egui::RichText::new("ПОДКЛЮЧЕНИЕ ТЕЛЕФОНА").size(11.0).strong().color(egui::Color32::from_rgb(207, 179, 237)));
        ui.add_space(8.0);
        ui.heading(egui::RichText::new("AERIS Link").size(30.0).color(egui::Color32::WHITE));
        ui.label(egui::RichText::new("Подключи AyuGram / exteraGram к этому компьютеру").size(14.0).color(egui::Color32::from_rgb(194, 181, 207)));
        ui.add_space(22.0);
        egui::Frame::none().fill(panel).rounding(egui::Rounding::same(22.0)).inner_margin(egui::Margin::same(20.0)).show(ui, |ui| {
            ui.label(egui::RichText::new("АДРЕС КОМПЬЮТЕРА").size(10.0).strong().color(egui::Color32::from_rgb(194, 181, 207)));
            ui.add_space(5.0);
            ui.label(egui::RichText::new(address).size(18.0).strong().color(egui::Color32::WHITE).monospace());
            ui.add_space(18.0);
            ui.label(egui::RichText::new("КОД СОПРЯЖЕНИЯ").size(10.0).strong().color(egui::Color32::from_rgb(194, 181, 207)));
            ui.add_space(5.0);
            ui.label(egui::RichText::new(code).size(30.0).strong().color(egui::Color32::WHITE).monospace());
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Код действует 3 минуты. После первого сопряжения плагин запомнит устройство.").size(12.0).color(egui::Color32::from_rgb(194, 181, 207)));
        });
        ui.add_space(16.0);
        let button = if running { "Остановить приёмник" } else { "Запустить приёмник" };
        if ui.add_sized([ui.available_width(), 48.0], egui::Button::new(egui::RichText::new(button).size(15.0).strong()).fill(egui::Color32::from_rgb(177, 133, 219)).rounding(egui::Rounding::same(16.0))).clicked() {
            if running { self.stop_receiver(); } else { self.start_receiver(); }
        }
        ui.add_space(18.0);
        ui.label(egui::RichText::new("НА ТЕЛЕФОНЕ").size(10.0).strong().color(egui::Color32::from_rgb(207, 179, 237)));
        ui.add_space(6.0);
        ui.label(egui::RichText::new("1. Открой настройки плагина AERIS.\n2. Введи IP-адрес и код выше.\n3. Нажми «Проверить подключение».\n4. Вернись в плеер Telegram и отправь трек через меню AERIS.").size(14.0).color(egui::Color32::WHITE));
        ui.add_space(18.0);
        ui.label(egui::RichText::new(format!("Состояние: {}", self.status.lines().next().unwrap_or(&self.status))).size(12.0).color(egui::Color32::from_rgb(194, 181, 207)));
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
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(27, 20, 36);
        visuals.window_fill = egui::Color32::from_rgb(37, 27, 48);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(43, 32, 55);
        visuals.selection.bg_fill = egui::Color32::from_rgb(177, 133, 219);
        ctx.set_visuals(visuals);

        egui::TopBottomPanel::top("top_bar").frame(egui::Frame::none().fill(egui::Color32::from_rgb(27, 20, 36)).inner_margin(egui::Margin::symmetric(22.0, 16.0))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("AERIS").size(19.0).strong().color(egui::Color32::WHITE));
                ui.add_space(18.0);
                for (page, label) in [(Page::Player, "Плеер"), (Page::Connect, "Подключение")] {
                    let selected = self.page == page;
                    if ui.selectable_label(selected, egui::RichText::new(label).size(14.0)).clicked() { self.page = page; }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let running = self.receiver_thread.is_some();
                    let dot = if running { egui::Color32::from_rgb(133, 218, 169) } else { egui::Color32::from_rgb(145, 132, 154) };
                    ui.label(egui::RichText::new(if running { "Приёмник активен" } else { "Офлайн" }).size(11.0).color(egui::Color32::from_rgb(202, 190, 211)));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, dot);
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::from_rgb(35, 25, 46)).inner_margin(egui::Margin::same(22.0))).show(ctx, |ui| {
            match self.page { Page::Player => self.player_page(ui), Page::Connect => self.connect_page(ui) }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn main() -> eframe::Result<()> {
    if let Ok(file) = std::fs::File::create(std::env::temp_dir().join("aeris-desktop.log")) {
        let _ = tracing_subscriber::fmt().with_writer(file).with_ansi(false).try_init();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([620.0, 820.0]).with_min_inner_size([480.0, 680.0]).with_title("AERIS Desktop"),
        ..Default::default()
    };
    eframe::run_native("AERIS Desktop", options, Box::new(|_cc| Ok(Box::new(AerisDesktop::default()))))
}
