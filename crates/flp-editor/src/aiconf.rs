//! AI provider settings window: which service the chat uses and its key, instead of
//! environment variables. Saved in `editor.toml`; applied to the process environment,
//! which the provider (`flp_ai::provider::from_env`) and the `claude` child read.

use eframe::egui;

use crate::theme::{self, AiSettings};

pub const KIND_CLAUDE: &str = "claude";
pub const KIND_OPENAI: &str = "openai";

#[derive(Default)]
pub struct AiConf {
    open: bool,
    draft: AiSettings,
}

impl AiConf {
    pub fn show(&mut self) {
        self.draft = theme::ai_settings();
        self.open = true;
    }

    pub fn window(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut save = false;
        egui::Window::new(crate::i18n::tr("Настройки ИИ"))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                let d = &mut self.draft;
                ui.strong(crate::i18n::tr("Встроенный чат (по API)"));
                egui::ComboBox::from_id_salt("ai_kind")
                    .selected_text(kind_name(&d.kind))
                    .show_ui(ui, |ui| {
                        for k in [KIND_CLAUDE, KIND_OPENAI] {
                            ui.selectable_value(&mut d.kind, k.to_owned(), kind_name(k));
                        }
                    });
                egui::Grid::new("ai_api").num_columns(2).show(ui, |ui| {
                    if d.kind == KIND_OPENAI {
                        row(ui, "Адрес API", &mut d.base_url, false, "https://api.deepseek.com");
                    }
                    row(ui, "Ключ", &mut d.api_key, true, "");
                    row(ui, "Модель", &mut d.model, false, model_hint(&d.kind));
                });
                ui.separator();
                ui.strong(crate::i18n::tr("Claude Code (когда включён MCP)"));
                ui.weak(crate::i18n::tr(
                    "Пусто — свой вход Claude Code. Для DeepSeek и др.: адрес с Anthropic-совместимым API, ключ и модель.",
                ));
                egui::Grid::new("ai_cc").num_columns(2).show(ui, |ui| {
                    row(ui, "Адрес API", &mut d.cc_base_url, false, "https://api.deepseek.com/anthropic");
                    row(ui, "Ключ", &mut d.cc_key, true, "");
                    row(ui, "Модель", &mut d.cc_model, false, "deepseek-chat");
                });
                ui.separator();
                ui.weak(crate::i18n::tr("Ключи хранятся в editor.toml на этом компьютере открытым текстом."));
                if ui.button(crate::i18n::tr("Сохранить")).clicked() {
                    save = true;
                }
            });
        if save {
            theme::save_ai_settings(&self.draft);
            apply(&self.draft);
            open = false;
        }
        self.open = open;
    }
}

fn kind_name(kind: &str) -> &'static str {
    match kind {
        KIND_OPENAI => "OpenAI-совместимый (DeepSeek, OpenAI, ...)",
        _ => "Claude (Anthropic)",
    }
}

fn model_hint(kind: &str) -> &'static str {
    if kind == KIND_OPENAI {
        "deepseek-chat"
    } else {
        flp_ai::provider::DEFAULT_MODEL
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: &mut String, secret: bool, hint: &str) {
    ui.label(crate::i18n::tr(label));
    ui.add(
        egui::TextEdit::singleline(value)
            .password(secret)
            .hint_text(hint)
            .desired_width(300.0),
    );
    ui.end_row();
}

/// Puts the saved settings into the environment the provider reads. Empty fields
/// leave the existing environment untouched.
pub fn apply(s: &AiSettings) {
    let set = |name: &str, value: &str| {
        if !value.trim().is_empty() {
            std::env::set_var(name, value.trim());
        }
    };
    if s.kind == KIND_OPENAI {
        set("FLP_AI_BASE_URL", &s.base_url);
        set("FLP_AI_KEY", &s.api_key);
    } else if s.kind == KIND_CLAUDE {
        std::env::remove_var("FLP_AI_BASE_URL");
        set("ANTHROPIC_API_KEY", &s.api_key);
    }
    set("FLP_AI_MODEL", &s.model);
}

/// Environment for the `claude` child that serves the chat in MCP mode.
pub fn claude_code_env(s: &AiSettings) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    let mut put = |name: &'static str, value: &str| {
        if !value.trim().is_empty() {
            env.push((name, value.trim().to_owned()));
        }
    };
    put("ANTHROPIC_BASE_URL", &s.cc_base_url);
    put("ANTHROPIC_AUTH_TOKEN", &s.cc_key);
    put("ANTHROPIC_MODEL", &s.cc_model);
    put("ANTHROPIC_SMALL_FAST_MODEL", &s.cc_model);
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_env_skips_empty() {
        let s = AiSettings {
            cc_base_url: " https://x/anthropic ".into(),
            cc_model: "deepseek-chat".into(),
            ..Default::default()
        };
        let env = claude_code_env(&s);
        assert!(env.contains(&("ANTHROPIC_BASE_URL", "https://x/anthropic".to_owned())));
        assert!(env.iter().all(|(n, _)| *n != "ANTHROPIC_AUTH_TOKEN"));
        assert_eq!(env.len(), 3);
    }
}
