//! Start-up splash: big logo, name and version over the whole window for two seconds.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, OnceLock};
use std::time::Instant;

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

/// How long the splash stays, seconds; the last part fades out.
const SHOW_SECS: f64 = 2.0;
const FADE_SECS: f64 = 0.3;

static START: OnceLock<Instant> = OnceLock::new();
static DISMISSED: AtomicBool = AtomicBool::new(false);
static AREA: LazyLock<egui::Id> = LazyLock::new(|| egui::Id::unique("splash"));

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

    let rect = ctx.content_rect();
    let visuals = ctx.global_style().visuals.clone();
    let background = visuals.panel_fill.gamma_multiply(alpha);
    let text = visuals.strong_text_color().gamma_multiply(alpha);
    let accent = visuals.hyperlink_color.gamma_multiply(alpha);
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
            logo(
                painter,
                center - Vec2::new(0.0, 95.0 * unit),
                unit,
                text,
                accent,
            );
            painter.text(
                center + Vec2::new(0.0, 45.0 * unit),
                Align2::CENTER_CENTER,
                "CrabLogic",
                FontId::proportional(84.0 * unit),
                text,
            );
            painter.text(
                center + Vec2::new(0.0, 112.0 * unit),
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

/// Two function blocks joined by a wire: the FBD scheme in miniature.
fn logo(painter: &egui::Painter, center: Pos2, unit: f32, body: Color32, accent: Color32) {
    let block = Vec2::new(86.0, 64.0) * unit;
    let gap = 60.0 * unit;
    let left = Rect::from_center_size(center - Vec2::new((block.x + gap) / 2.0, 0.0), block);
    let right = Rect::from_center_size(center + Vec2::new((block.x + gap) / 2.0, 0.0), block);
    let wire = Stroke::new(6.0 * unit, accent);
    let radius = CornerRadius::same((10.0 * unit) as u8);
    let wire_y = center.y;
    painter.line_segment(
        [
            Pos2::new(left.right(), wire_y),
            Pos2::new(right.left(), wire_y),
        ],
        wire,
    );
    for r in [left, right] {
        painter.rect_filled(r, radius, accent);
        painter.rect_filled(r.shrink(6.0 * unit), radius, body.gamma_multiply(0.18));
        // Ports on both sides of the block.
        for x in [r.left(), r.right()] {
            painter.circle_filled(Pos2::new(x, wire_y), 7.0 * unit, body);
        }
    }
    // A line inside each block: the "function" of the block.
    for r in [left, right] {
        painter.line_segment(
            [
                Pos2::new(r.left() + 20.0 * unit, wire_y),
                Pos2::new(r.right() - 20.0 * unit, wire_y),
            ],
            Stroke::new(5.0 * unit, body),
        );
    }
}
