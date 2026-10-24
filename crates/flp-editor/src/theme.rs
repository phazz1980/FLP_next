//! Colour themes. Two built-in ones (a warm grey light theme and a soft graphite dark
//! one) plus user themes stored as TOML files in the user's config folder.
//!
//! A theme file lists colours as `key = [r, g, b]`; missing keys fall back to the
//! built-in theme with the same base (light or dark), so old files keep working.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Stroke, ThemePreference};
use serde::{Deserialize, Serialize};

pub const LIGHT_NAME: &str = "Стандартная";
pub const DARK_NAME: &str = "Тёмная";

macro_rules! palette {
    ($( $group:literal { $( $field:ident : $label:literal ),* $(,)? } )*) => {
        /// Colours of the interface and the working area.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Palette {
            $($(pub $field: Color32,)*)*
        }

        impl Palette {
            /// Every colour as (group, key, label, value) in display order.
            pub fn entries_mut(&mut self) -> Vec<(&'static str, &'static str, &'static str, &mut Color32)> {
                vec![$($(($group, stringify!($field), $label, &mut self.$field),)*)*]
            }
        }
    };
}

palette! {
    "Интерфейс" {
        ui_panel: "Панели",
        ui_window: "Окна",
        ui_field: "Поля ввода",
        ui_faint: "Кнопки и полосы",
        text: "Текст",
        weak: "Второстепенный текст",
        accent: "Акцент и выделение",
        error: "Ошибка",
        warning: "Предупреждение",
        ok: "Успех",
    }
    "Область схемы" {
        canvas: "Фон",
        sheet: "Плата",
        sheet_header: "Заголовок платы",
        grid: "Сетка",
        block_fill: "Блок",
        block_header: "Заголовок блока",
        block_border: "Рамка блока",
        tag_fill: "Тег (переменная)",
    }
    "Типы данных" {
        bool_signal: "Boolean",
        int_signal: "Integer",
        float_signal: "Float",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    /// Base egui look (widget shapes, shadows) and fallback for missing colours.
    pub dark: bool,
    /// Built-in themes are not saved or deleted; edit a copy instead.
    pub builtin: bool,
    pub palette: Palette,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub fn light() -> Theme {
    Theme {
        name: LIGHT_NAME.to_owned(),
        dark: false,
        builtin: true,
        palette: Palette {
            ui_panel: rgb(226, 223, 216),
            ui_window: rgb(232, 229, 223),
            ui_field: rgb(240, 238, 233),
            ui_faint: rgb(219, 215, 207),
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
            // FLProg default data-type colors.
            bool_signal: rgb(0, 0, 0),
            int_signal: rgb(0, 0, 255),
            float_signal: rgb(127, 63, 63),
        },
    }
}

pub fn dark() -> Theme {
    Theme {
        name: DARK_NAME.to_owned(),
        dark: true,
        builtin: true,
        palette: Palette {
            ui_panel: rgb(38, 41, 46),
            ui_window: rgb(42, 45, 51),
            ui_field: rgb(30, 32, 36),
            ui_faint: rgb(46, 50, 56),
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
            // FLProg hues, lightened to stay visible on a dark sheet.
            bool_signal: rgb(225, 225, 225),
            int_signal: rgb(96, 136, 255),
            float_signal: rgb(196, 120, 120),
        },
    }
}

/// On-disk form of a theme.
#[derive(Serialize, Deserialize)]
struct ThemeFile {
    name: String,
    #[serde(default)]
    dark: bool,
    #[serde(default)]
    colors: BTreeMap<String, [u8; 3]>,
}

impl Theme {
    pub fn to_toml(&self) -> String {
        let mut p = self.palette;
        let colors = p
            .entries_mut()
            .into_iter()
            .map(|(_, key, _, c)| (key.to_owned(), [c.r(), c.g(), c.b()]))
            .collect();
        let file = ThemeFile {
            name: self.name.clone(),
            dark: self.dark,
            colors,
        };
        toml::to_string_pretty(&file).unwrap_or_default()
    }

    pub fn from_toml(text: &str) -> Result<Self, String> {
        let file: ThemeFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut theme = if file.dark { dark() } else { light() };
        theme.name = file.name;
        theme.builtin = false;
        for (_, key, _, c) in theme.palette.entries_mut() {
            if let Some([r, g, b]) = file.colors.get(key) {
                *c = rgb(*r, *g, *b);
            }
        }
        Ok(theme)
    }

    /// File name for a user theme: the name with unsafe characters replaced.
    fn file_name(&self) -> String {
        let stem: String = self
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{}.toml", stem.trim())
    }
}

/// `%APPDATA%\FLP_next` (or `~/.config/FLP_next` elsewhere).
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|d| d.join("FLP_next"))
}

pub fn themes_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("themes"))
}

/// Built-in themes followed by user themes sorted by name.
pub fn load_all() -> Vec<Theme> {
    let mut user = Vec::new();
    if let Some(entries) = themes_dir().and_then(|d| fs::read_dir(d).ok()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                if let Some(t) = fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| Theme::from_toml(&s).ok())
                {
                    user.push(t);
                }
            }
        }
    }
    user.sort_by(|a, b| a.name.cmp(&b.name));
    let mut all = vec![light(), dark()];
    all.extend(
        user.into_iter()
            .filter(|t| t.name != LIGHT_NAME && t.name != DARK_NAME),
    );
    all
}

pub fn save(theme: &Theme) -> Result<(), String> {
    let dir = themes_dir().ok_or("не найдена папка настроек пользователя")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // A renamed theme may still have its old file; drop files holding the same name.
    remove(&theme.name)?;
    fs::write(dir.join(theme.file_name()), theme.to_toml()).map_err(|e| e.to_string())
}

/// Deletes the user theme file(s) holding `name`.
pub fn remove(name: &str) -> Result<(), String> {
    let Some(entries) = themes_dir().and_then(|d| fs::read_dir(d).ok()) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let same = fs::read_to_string(&path)
            .ok()
            .and_then(|s| Theme::from_toml(&s).ok())
            .is_some_and(|t| t.name == name);
        if same {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    theme: Option<String>,
    /// Project folder opened last time.
    last_project: Option<PathBuf>,
    /// Autosave period in minutes; 0 turns autosave off.
    autosave_minutes: Option<u32>,
}

fn load_settings() -> Settings {
    config_dir()
        .and_then(|d| fs::read_to_string(d.join("editor.toml")).ok())
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn update_settings(change: impl FnOnce(&mut Settings)) {
    let Some(dir) = config_dir() else { return };
    let mut settings = load_settings();
    change(&mut settings);
    if fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(
            dir.join("editor.toml"),
            toml::to_string(&settings).unwrap_or_default(),
        );
    }
}

/// Name of the theme chosen last time.
pub fn saved_choice() -> Option<String> {
    load_settings().theme
}

pub fn save_choice(name: &str) {
    update_settings(|s| s.theme = Some(name.to_owned()));
}

/// Project folder opened last time.
pub fn last_project() -> Option<PathBuf> {
    load_settings().last_project
}

pub fn save_last_project(path: &Path) {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    update_settings(|s| s.last_project = Some(path));
}

pub const DEFAULT_AUTOSAVE_MINUTES: u32 = 2;

/// Autosave period in minutes; 0 means off.
pub fn autosave_minutes() -> u32 {
    load_settings()
        .autosave_minutes
        .unwrap_or(DEFAULT_AUTOSAVE_MINUTES)
}

pub fn save_autosave_minutes(minutes: u32) {
    update_settings(|s| s.autosave_minutes = Some(minutes));
}

/// Folder for projects created without asking: `Documents/FLP_next` when it exists.
pub fn default_projects_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let home = PathBuf::from(home);
    let docs = home.join("Documents");
    Some(if docs.is_dir() { docs } else { home }.join("FLP_next"))
}

/// Makes `theme` the egui look.
pub fn apply(ctx: &egui::Context, theme: &Theme) {
    let slot = if theme.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_visuals_of(slot, visuals(theme));
    ctx.set_theme(if theme.dark {
        ThemePreference::Dark
    } else {
        ThemePreference::Light
    });
}

fn visuals(theme: &Theme) -> egui::Visuals {
    let p = &theme.palette;
    let mut v = if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.ui_panel;
    v.window_fill = p.ui_window;
    v.extreme_bg_color = p.ui_field;
    v.faint_bg_color = p.ui_faint;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.error_fg_color = p.error;
    v.warn_fg_color = p.warning;
    v.widgets.noninteractive.bg_fill = p.ui_panel;
    v.widgets.inactive.weak_bg_fill = p.ui_faint;
    v.widgets.inactive.bg_fill = p.ui_faint;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_round_trip() {
        let mut t = dark();
        t.name = "Моя".into();
        t.builtin = false;
        t.palette.int_signal = rgb(1, 2, 3);
        assert_eq!(Theme::from_toml(&t.to_toml()).unwrap(), t);
    }

    #[test]
    fn missing_colors_fall_back_to_base() {
        let t = Theme::from_toml("name = \"x\"\n[colors]\nbool_signal = [9, 9, 9]\n").unwrap();
        assert_eq!(t.palette.bool_signal, rgb(9, 9, 9));
        assert_eq!(t.palette.canvas, light().palette.canvas);
    }
}
