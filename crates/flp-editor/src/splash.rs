//! Start-up splash: big logo, name and version over the whole window for three seconds.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::Instant;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Sense, Vec2};

/// How long the splash stays, seconds; the last part fades out.
const SHOW_SECS: f64 = 3.0;
const FADE_SECS: f64 = 0.3;

static START: OnceLock<Instant> = OnceLock::new();
static DISMISSED: AtomicBool = AtomicBool::new(false);
static AREA: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::unique("splash"));
static LOGO: OnceLock<Option<egui::TextureHandle>> = OnceLock::new();

/// The largest PNG image of the application icon (`assets/icon.ico`).
fn icon_image() -> Option<egui::ColorImage> {
    let ico: &[u8] = include_bytes!("../assets/icon.ico");
    let count = usize::from(u16::from_le_bytes([*ico.get(4)?, *ico.get(5)?]));
    let mut best: Option<(u32, usize, usize)> = None;
    for i in 0..count {
        let e = ico.get(6 + 16 * i..22 + 16 * i)?;
        // A width of 0 stands for 256.
        let width = if e[0] == 0 { 256 } else { u32::from(e[0]) };
        let size = u32::from_le_bytes(e[8..12].try_into().ok()?) as usize;
        let offset = u32::from_le_bytes(e[12..16].try_into().ok()?) as usize;
        if best.is_none_or(|(w, ..)| width > w) {
            best = Some((width, offset, size));
        }
    }
    let (_, offset, size) = best?;
    let png = image::load_from_memory_with_format(
        ico.get(offset..offset + size)?,
        image::ImageFormat::Png,
    )
    .ok()?
    .to_rgba8();
    let size = [png.width() as usize, png.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, png.as_raw()))
}

/// Draws the splash on top of everything until the time is over or it is clicked.
pub fn show(ctx: &egui::Context) {
    if DISMISSED.load(Ordering::Relaxed) {
        return;
    }
    let elapsed = START.get_or_init(Instant::now).elapsed().as_secs_f64();
    if elapsed >= SHOW_SECS {
        DISMISSED.store(true, Ordering::Relaxed);
        return;
    }
    ctx.request_repaint();
    let alpha = ((SHOW_SECS - elapsed) / FADE_SECS).clamp(0.0, 1.0) as f32;
    // Loaded outside the area closure: `load_texture` takes the context lock too.
    let logo = LOGO.get_or_init(|| {
        icon_image().map(|img| ctx.load_texture("splash_logo", img, egui::TextureOptions::LINEAR))
    });

    let rect = ctx.content_rect();
    let visuals = ctx.global_style().visuals.clone();
    let background = visuals.panel_fill.gamma_multiply(alpha);
    let text = visuals.strong_text_color().gamma_multiply(alpha);
    let weak = visuals.weak_text_color().gamma_multiply(alpha);

    let response = egui::Area::new(*AREA)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(rect.size(), Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, CornerRadius::ZERO, background);

            let unit = (rect.width().min(rect.height()) / 520.0).clamp(0.6, 2.0);
            let center = rect.center();
            if let Some(logo) = logo {
                let side = 220.0 * unit;
                let image =
                    Rect::from_center_size(center - Vec2::new(0.0, 85.0 * unit), Vec2::splat(side));
                painter.image(
                    logo.id(),
                    image,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE.gamma_multiply(alpha),
                );
            }
            painter.text(
                center + Vec2::new(0.0, 85.0 * unit),
                Align2::CENTER_CENTER,
                "CrabLogic",
                FontId::proportional(84.0 * unit),
                text,
            );
            painter.text(
                center + Vec2::new(0.0, 152.0 * unit),
                Align2::CENTER_CENTER,
                format!("v{}", crate::update::CURRENT),
                FontId::proportional(30.0 * unit),
                weak,
            );
            response
        })
        .inner;
    if response.clicked() {
        DISMISSED.store(true, Ordering::Relaxed);
    }
}
