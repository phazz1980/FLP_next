//! Two calm themes. The light one is a warm grey rather than white, the dark one
//! is a soft graphite; accents and signal colours are muted.

use eframe::egui::{self, Color32, Stroke, Theme, ThemePreference};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Light,
    Dark,
}

/// Colours of the working area.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub sheet: Color32,
    pub sheet_header: Color32,
    pub grid: Color32,
    pub block_fill: Color32,
    pub block_header: Color32,
    pub block_border: Color32,
    pub tag_fill: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub ok: Color32,
    pub bool_signal: Color32,
    pub int_signal: Color32,
    pub float_signal: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn palette(kind: ThemeKind) -> Palette {
    match kind {
        ThemeKind::Light => Palette {
            canvas: rgb(214, 210, 202),
            sheet: rgb(234, 231, 224),
            sheet_header: rgb(222, 218, 209),
            grid: rgb(224, 220, 212),
            block_fill: rgb(243, 241, 236),
            block_header: rgb(214, 222, 229),
            block_border: rgb(138, 144, 150),
            tag_fill: rgb(222, 230, 222),
            text: rgb(46, 49, 53),
            weak: rgb(112, 116, 121),
            accent: rgb(79, 127, 168),
            error: rgb(186, 74, 66),
            warning: rgb(176, 128, 40),
            ok: rgb(72, 136, 88),
            bool_signal: rgb(86, 146, 104),
            int_signal: rgb(82, 126, 176),
            float_signal: rgb(184, 128, 62),
        },
        ThemeKind::Dark => Palette {
            canvas: rgb(28, 30, 34),
            sheet: rgb(36, 39, 44),
            sheet_header: rgb(44, 48, 54),
            grid: rgb(42, 45, 51),
            block_fill: rgb(48, 52, 59),
            block_header: rgb(58, 66, 78),
            block_border: rgb(98, 104, 112),
            tag_fill: rgb(48, 62, 54),
            text: rgb(214, 217, 222),
            weak: rgb(140, 146, 154),
            accent: rgb(110, 156, 196),
            error: rgb(212, 104, 96),
            warning: rgb(206, 166, 82),
            ok: rgb(110, 176, 126),
            bool_signal: rgb(104, 170, 124),
            int_signal: rgb(104, 150, 204),
            float_signal: rgb(206, 150, 86),
        },
    }
}

/// Installs both themes into egui and selects `kind`.
pub fn apply(ctx: &egui::Context, kind: ThemeKind) {
    ctx.set_visuals_of(Theme::Light, visuals(ThemeKind::Light));
    ctx.set_visuals_of(Theme::Dark, visuals(ThemeKind::Dark));
    ctx.set_theme(match kind {
        ThemeKind::Light => ThemePreference::Light,
        ThemeKind::Dark => ThemePreference::Dark,
    });
}

fn visuals(kind: ThemeKind) -> egui::Visuals {
    let p = palette(kind);
    let mut v = match kind {
        ThemeKind::Light => egui::Visuals::light(),
        ThemeKind::Dark => egui::Visuals::dark(),
    };
    let (panel, window, field, faint) = match kind {
        ThemeKind::Light => (
            rgb(226, 223, 216),
            rgb(232, 229, 223),
            rgb(240, 238, 233),
            rgb(219, 215, 207),
        ),
        ThemeKind::Dark => (
            rgb(38, 41, 46),
            rgb(42, 45, 51),
            rgb(30, 32, 36),
            rgb(46, 50, 56),
        ),
    };
    v.panel_fill = panel;
    v.window_fill = window;
    v.extreme_bg_color = field;
    v.faint_bg_color = faint;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.error;
    v.warn_fg_color = p.warning;
    v.widgets.noninteractive.bg_fill = panel;
    v.widgets.inactive.weak_bg_fill = faint;
    v.widgets.inactive.bg_fill = faint;
    v
}
