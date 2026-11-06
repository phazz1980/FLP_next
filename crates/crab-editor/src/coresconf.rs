//! "ESP32 cores" window: which chips have their core on disk and downloading the
//! missing ones. A chip without a core is greyed out in the board picker.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use crab_build::cores::{self, CorePack};
use eframe::egui;

struct Job {
    chip: &'static str,
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    lines: Arc<Mutex<Vec<String>>>,
    rx: mpsc::Receiver<Result<(), String>>,
}

#[derive(Default)]
pub struct CoresConf {
    open: bool,
    /// Presence of each pack in `cores::PACKS` order.
    status: Vec<bool>,
    job: Option<Job>,
    message: Option<(bool, String)>,
}

impl CoresConf {
    pub fn show(&mut self) {
        self.refresh();
        self.message = None;
        self.open = true;
    }

    /// A core download is running (also with the window closed).
    pub fn downloading(&self) -> bool {
        self.job.is_some()
    }

    fn refresh(&mut self) {
        self.status = cores::PACKS.iter().map(cores::is_installed).collect();
    }

    fn start(&mut self, pack: &'static CorePack, ctx: &egui::Context) {
        let done = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        let (d, t, c, l, ctx) = (
            done.clone(),
            total.clone(),
            cancel.clone(),
            lines.clone(),
            ctx.clone(),
        );
        std::thread::spawn(move || {
            let mut on_line = |s: &str| {
                l.lock().unwrap().push(s.to_owned());
                ctx.request_repaint();
            };
            let repaint = ctx.clone();
            let mut progress = |got: u64, all: u64| {
                d.store(got, Ordering::Relaxed);
                t.store(all, Ordering::Relaxed);
                repaint.request_repaint();
            };
            let result =
                cores::download(pack, &c, &mut on_line, &mut progress).map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.job = Some(Job {
            chip: pack.chip,
            done,
            total,
            cancel,
            lines,
            rx,
        });
        self.message = None;
    }

    fn poll(&mut self) {
        let Some(job) = &self.job else { return };
        if let Ok(result) = job.rx.try_recv() {
            self.message = Some(match result {
                Ok(()) => (true, crate::i18n::tr("Готово").to_owned()),
                Err(e) => (false, e),
            });
            self.job = None;
            self.refresh();
        }
    }

    pub fn window(&mut self, ctx: &egui::Context) {
        self.poll();
        if !self.open {
            return;
        }
        let mut open = true;
        let mut start: Option<&'static CorePack> = None;
        let mut refresh = false;
        egui::Window::new(crate::i18n::tr("Ядра ESP32"))
            .open(&mut open)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.weak(crate::i18n::tr(
                    "Плата с чипом без скачанного ядра недоступна для выбора. Ядра ищутся в папке esp32core рядом с программой и в Arduino15.",
                ));
                ui.add_space(6.0);
                let busy = self.job.is_some();
                egui::Grid::new("cores_grid")
                    .num_columns(3)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        for (i, pack) in cores::PACKS.iter().enumerate() {
                            ui.strong(pack.title);
                            let have = self.status.get(i).copied().unwrap_or(false);
                            if have {
                                ui.label(
                                    egui::RichText::new(crate::i18n::tr("✔ установлено"))
                                        .color(egui::Color32::from_rgb(70, 160, 90)),
                                );
                                ui.label("");
                            } else {
                                ui.label(
                                    egui::RichText::new(crate::i18n::tr("не скачано"))
                                        .color(ui.visuals().warn_fg_color),
                                );
                                let label = crate::i18n::fmt("Скачать ({0} МБ)", &[&pack.size_mb]);
                                if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                                    start = Some(pack);
                                }
                            }
                            ui.end_row();
                        }
                    });
                if let Some(job) = &self.job {
                    ui.separator();
                    let done = job.done.load(Ordering::Relaxed);
                    let total = job.total.load(Ordering::Relaxed);
                    let fraction = if total > 0 {
                        done as f32 / total as f32
                    } else {
                        0.0
                    };
                    let text = if total > 0 {
                        format!("{job_chip}: {} / {} МБ", done >> 20, total >> 20, job_chip = job.chip)
                    } else {
                        job.chip.to_owned()
                    };
                    ui.add(egui::ProgressBar::new(fraction).text(text));
                    if let Some(last) = job.lines.lock().unwrap().last() {
                        ui.weak(last);
                    }
                    if ui.button(crate::i18n::tr("Отмена")).clicked() {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                }
                if let Some((ok, text)) = &self.message {
                    ui.separator();
                    let color = if *ok {
                        egui::Color32::from_rgb(70, 160, 90)
                    } else {
                        ui.visuals().error_fg_color
                    };
                    ui.label(egui::RichText::new(text).color(color));
                }
                ui.separator();
                if ui.button(crate::i18n::tr("Обновить статус")).clicked() {
                    refresh = true;
                }
            });
        if let Some(pack) = start {
            self.start(pack, ctx);
        }
        if refresh {
            self.refresh();
        }
        if !open && self.job.is_none() {
            self.open = false;
        }
        // The download goes on with the window closed; the window stays until it ends.
    }
}
