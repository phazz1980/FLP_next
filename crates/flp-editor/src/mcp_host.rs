//! The editor's own MCP server (HTTP on localhost), started by hand from the menu.
//! While it runs, an AI client can read and change the project open here.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;
use flp_mcp::http::{HttpServer, SharedProject};

#[derive(Default)]
pub struct McpHost {
    server: Option<HttpServer>,
    shared: SharedProject,
    /// The window with the prompt for the AI is shown.
    window: bool,
    error: Option<String>,
}

impl McpHost {
    pub fn running(&self) -> bool {
        self.server.is_some()
    }

    /// Starts the server (and shows the prompt) or stops it.
    pub fn toggle(&mut self, library: Option<PathBuf>) {
        if self.server.take().is_some() {
            self.window = false;
            return;
        }
        self.shared = Arc::new(Mutex::new(None));
        match HttpServer::start(library, self.shared.clone()) {
            Ok(server) => {
                self.server = Some(server);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!(
                    "Не удалось запустить MCP-сервер на порту {}: {e}",
                    flp_mcp::http::PORT
                ));
            }
        }
        self.window = true;
    }

    /// URL and token of the running server.
    pub fn endpoint(&self) -> Option<(String, String)> {
        self.server.as_ref().map(|s| (s.url(), s.token.clone()))
    }

    pub fn show_window(&mut self) {
        self.window = true;
    }

    /// Tells the server which project the editor has open now.
    pub fn set_project(&self, path: Option<&Path>) {
        if self.server.is_none() {
            return;
        }
        if let Ok(mut p) = self.shared.lock() {
            if p.as_deref() != path {
                *p = path.map(Path::to_path_buf);
            }
        }
    }

    /// The window with the connection config and the prompt to give to the AI.
    pub fn window(&mut self, ctx: &egui::Context) {
        if !self.window {
            return;
        }
        let mut open = true;
        egui::Window::new(crate::i18n::tr("ИИ по MCP"))
            .open(&mut open)
            .default_width(560.0)
            .show(ctx, |ui| {
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                let Some(server) = &self.server else { return };
                ui.label(format!(
                    "{} {}",
                    crate::i18n::tr("Сервер запущен:"),
                    server.url()
                ));
                ui.label(crate::i18n::tr(
                    "ИИ работает с проектом, открытым в этом окне. Сервер действует, пока редактор открыт или пока вы не выключите его в меню.",
                ));
                let prompt = prompt(server);
                ui.separator();
                ui.label(crate::i18n::tr("Промпт для ИИ (вставьте его в чат):"));
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        let mut text = prompt.as_str();
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                ui.horizontal(|ui| {
                    if ui
                        .button(crate::i18n::tr("Скопировать промпт"))
                        .clicked()
                    {
                        ui.ctx().copy_text(prompt.clone());
                    }
                    if ui
                        .button(crate::i18n::tr("Скопировать только конфиг"))
                        .clicked()
                    {
                        ui.ctx().copy_text(server.config_snippet());
                    }
                });
            });
        if !open {
            self.window = false;
        }
    }
}

/// Text for the AI: how to connect and how to work with the project.
fn prompt(server: &HttpServer) -> String {
    format!(
        "Подключись к MCP-серверу CrabLogic и помоги с проектом, открытым сейчас в моём редакторе.\n\
         \n\
         1. Если сервера `crablogic` ещё нет в настройках MCP (файл `.mcp.json` проекта или конфиг клиента), \
         добавь его и переподключи MCP:\n\
         {}\n\
         \n\
         2. Работай только через инструменты этого сервера и только с проектом из окна редактора — \
         другие файлы проектов не открывай.\n\
         3. Начни с `get_schema` и `list_block_types`: схема — блоки с id и связи «блок.порт» → «блок.порт». \
         После каждой правки смотри поле `check` в ответе и исправляй ошибки.\n\
         4. Правки сразу записываются в файл проекта; редактор подхватывает их сам.\n\
         5. Если сервер ответил, что проекта нет, скажи мне открыть проект в редакторе.\n\
         \n\
         Моя задача: ",
        server.config_snippet()
    )
}
