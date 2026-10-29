//! Help window: the sections written in `help/*.md` (built into the program) and a
//! reference of blocks made from the library, with search.
//!
//! Pages are Markdown shown by [`crate::markdown`]. A link `help:<page>#<anchor>`
//! goes to another page; block pages are `blocks/<type>`, the list of all blocks is `blocks`.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText, TextEdit};
use flp_core::{DiagKind, Library, LibraryBlock, ParamKind};

use crate::app::EditorApp;
use crate::markdown;

/// Page shown when the help is opened without a target.
pub const START: &str = "quickstart";
/// Page with the explanations of check and build errors.
pub const ERRORS: &str = "errors";
/// Anchor on the errors page that explains a check problem of this kind.
pub fn error_anchor(kind: DiagKind) -> &'static str {
    match kind {
        DiagKind::BadId => "недопустимый-id-блока",
        DiagKind::DuplicateId => "одинаковые-id",
        DiagKind::UnknownType => "неизвестный-тип-блока",
        DiagKind::UnknownParam => "нет-такого-параметра",
        DiagKind::MissingParam => "не-задан-параметр",
        DiagKind::ParamValue => "недопустимое-значение-параметра",
        DiagKind::Singleton => "блок-допускается-один-раз",
        DiagKind::Condition => "условие-блока-не-вычисляется",
        DiagKind::ConstantType => "константа-не-подходит-к-входу",
        DiagKind::InvertType => "инверсия-не-подходит-к-порту",
        DiagKind::LinkEnds => "в-связи-нет-блока-или-порта",
        DiagKind::LinkType => "типы-порта-и-входа-не-совпадают",
        DiagKind::InputBusy => "на-вход-уже-подан-другой-источник",
        DiagKind::InputEmpty => "вход-без-источника",
        DiagKind::PinBusy => "вывод-gpio-уже-занят",
        DiagKind::PinUnavailable => "вывод-gpio-недоступен",
        DiagKind::PinWarning => "предупреждение-о-выводе-gpio",
        DiagKind::VariableName => "имя-переменной",
        DiagKind::VariableType => "тип-переменной",
        DiagKind::VariableLink => "связь-с-переменной",
        DiagKind::VariableWriters => "несколько-записывающих-блоков",
        DiagKind::VariableUnused => "переменная-не-используется",
        DiagKind::LayoutOrphan => "лишние-координаты-в-разметке",
        DiagKind::Loop => "цикл-без-памяти",
        DiagKind::Uart => "последовательный-порт",
    }
}

/// List of all blocks.
pub const BLOCKS: &str = "blocks";

/// Section id and source, in the order of the tree.
const FILES: &[(&str, &str)] = &[
    ("quickstart", include_str!("../../../help/01-quickstart.md")),
    ("editor", include_str!("../../../help/02-editor.md")),
    ("schema", include_str!("../../../help/03-schema.md")),
    ("library", include_str!("../../../help/04-library.md")),
    ("build", include_str!("../../../help/05-build.md")),
    ("debug", include_str!("../../../help/06-debug.md")),
    ("ai", include_str!("../../../help/07-ai.md")),
    ("errors", include_str!("../../../help/08-errors.md")),
    ("news", include_str!("../../../help/09-news.md")),
];

pub struct Page {
    pub id: String,
    pub title: String,
    pub text: String,
}

/// Page for a section source: the title is its first heading.
fn section(id: &str, text: &str) -> Page {
    let title = text
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or(id)
        .trim()
        .to_owned();
    Page {
        id: id.to_owned(),
        title,
        text: text.to_owned(),
    }
}

fn kind_name(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::Bool => "да/нет",
        ParamKind::Int => "целое",
        ParamKind::Float => "дробное",
        ParamKind::Text => "текст",
        ParamKind::Duration => "время",
        ParamKind::Pin => "пин",
    }
}

/// Table cells must not contain `|` or line breaks.
fn cell(text: &str) -> String {
    text.replace('|', "/").replace('\n', " ")
}

/// Reference page of one block: ports and parameters from `block.toml`, then its `docs.md`.
fn block_page(block: &LibraryBlock) -> Page {
    let info = &block.def.info;
    let mut text = format!("# {}\n\n", info.name);
    if !info.description.is_empty() {
        text += &format!("{}\n\n", info.description);
    }
    text += &format!("Тип: `{}`. Группа: {}.\n\n", info.kind, info.group);
    if !block.def.inputs.is_empty() {
        text += "## Входы\n\n| Порт | Тип | Описание |\n|---|---|---|\n";
        for p in &block.def.inputs {
            text += &format!(
                "| {} | {} | {} |\n",
                cell(p.title()),
                p.ty.name(),
                cell(&p.description)
            );
        }
        text.push('\n');
    }
    if !block.def.outputs.is_empty() {
        text += "## Выходы\n\n| Порт | Тип | Описание |\n|---|---|---|\n";
        for p in &block.def.outputs {
            text += &format!(
                "| {} | {} | {} |\n",
                cell(p.title()),
                p.ty.name(),
                cell(&p.description)
            );
        }
        text.push('\n');
    }
    if !block.def.params.is_empty() {
        text += "## Параметры\n\n| Параметр | Тип | Описание |\n|---|---|---|\n";
        for p in &block.def.params {
            text += &format!(
                "| {} | {} | {} |\n",
                cell(p.title()),
                kind_name(p.kind),
                cell(&p.description)
            );
        }
        text.push('\n');
    }
    if let Some(docs) = block
        .docs_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
    {
        text += &docs;
    }
    Page {
        id: format!("{BLOCKS}/{}", info.kind),
        title: info.name.clone(),
        text,
    }
}

/// Block group in the tree: its name and the blocks as (page id, title).
type Group = (String, Vec<(String, String)>);

/// Pages of all visible blocks and the groups for the tree; the list page comes first.
fn block_pages(lib: &Library) -> (Vec<Page>, Vec<Group>) {
    let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut pages = Vec::new();
    for block in lib.iter().filter(|b| !b.def.info.hidden) {
        let page = block_page(block);
        groups
            .entry(block.def.info.group.clone())
            .or_default()
            .push((page.id.clone(), page.title.clone()));
        pages.push(page);
    }
    let mut index = String::from("# Справочник блоков\n");
    let mut tree = Vec::new();
    for (group, mut blocks) in groups {
        blocks.sort_by_key(|b| b.1.to_lowercase());
        index += &format!("\n## {group}\n\n");
        for (id, title) in &blocks {
            index += &format!("- [{title}](help:{id})\n");
        }
        tree.push((group, blocks));
    }
    pages.insert(
        0,
        Page {
            id: BLOCKS.to_owned(),
            title: "Справочник блоков".to_owned(),
            text: index,
        },
    );
    (pages, tree)
}

/// One search result.
#[derive(Debug, PartialEq)]
pub struct Hit {
    pub id: String,
    pub title: String,
    /// The first line of the page with the query.
    pub context: String,
}

/// Pages whose title or text contain `query`, ignoring case. Empty query finds nothing.
pub fn search<'a>(pages: impl Iterator<Item = &'a Page>, query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for page in pages {
        let line = page
            .text
            .lines()
            .find(|l| l.to_lowercase().contains(&query));
        if line.is_none() && !page.title.to_lowercase().contains(&query) {
            continue;
        }
        let context = line.unwrap_or("").trim().trim_start_matches('#').trim();
        hits.push(Hit {
            id: page.id.clone(),
            title: page.title.clone(),
            context: context.chars().take(90).collect(),
        });
    }
    hits
}

pub struct HelpWindow {
    sections: Vec<Page>,
    blocks: Vec<Page>,
    groups: Vec<Group>,
    current: String,
    /// Heading to scroll to; used once, on the frame after the jump.
    anchor: Option<String>,
    history: Vec<String>,
    query: String,
    /// The library was reloaded: block pages are rebuilt on the next frame.
    pub blocks_dirty: bool,
}

impl HelpWindow {
    pub fn new(lib: &Library, target: &str) -> Self {
        let (blocks, groups) = block_pages(lib);
        let mut window = Self {
            sections: FILES.iter().map(|(id, text)| section(id, text)).collect(),
            blocks,
            groups,
            current: START.to_owned(),
            anchor: None,
            history: Vec::new(),
            query: String::new(),
            blocks_dirty: false,
        };
        window.goto(target);
        window
    }

    /// Goes to `page` or `page#anchor`.
    pub fn goto(&mut self, target: &str) {
        let (id, anchor) = match target.split_once('#') {
            Some((id, anchor)) => (id, Some(anchor.to_owned())),
            None => (target, None),
        };
        if id != self.current {
            let from = std::mem::replace(&mut self.current, id.to_owned());
            self.history.push(from);
        }
        self.anchor = anchor;
        self.query.clear();
    }

    fn page(&self, id: &str) -> Option<&Page> {
        self.sections
            .iter()
            .chain(&self.blocks)
            .find(|p| p.id == id)
    }

    fn rebuild_blocks(&mut self, lib: &Library) {
        (self.blocks, self.groups) = block_pages(lib);
        self.blocks_dirty = false;
    }

    /// The tree on the left; returns the page to open.
    fn tree(&self, ui: &mut egui::Ui) -> Option<String> {
        let mut go = None;
        for page in &self.sections {
            if ui
                .selectable_label(self.current == page.id, &page.title)
                .clicked()
            {
                go = Some(page.id.clone());
            }
        }
        ui.separator();
        egui::CollapsingHeader::new("Справочник блоков")
            .default_open(self.current.starts_with(BLOCKS))
            .show(ui, |ui| {
                if ui
                    .selectable_label(self.current == BLOCKS, "Все блоки")
                    .clicked()
                {
                    go = Some(BLOCKS.to_owned());
                }
                for (group, blocks) in &self.groups {
                    let open = blocks.iter().any(|(id, _)| *id == self.current);
                    egui::CollapsingHeader::new(group)
                        .id_salt(("help_group", group))
                        .open(open.then_some(true))
                        .show(ui, |ui| {
                            for (id, title) in blocks {
                                if ui.selectable_label(self.current == *id, title).clicked() {
                                    go = Some(id.clone());
                                }
                            }
                        });
                }
            });
        go
    }

    /// The open page (or the search results); returns the page to open.
    fn content(&mut self, ui: &mut egui::Ui) -> Option<String> {
        if !self.query.trim().is_empty() {
            let hits = search(self.sections.iter().chain(&self.blocks), &self.query);
            if hits.is_empty() {
                ui.weak("Ничего не найдено");
            }
            let mut go = None;
            for hit in hits {
                if ui.link(RichText::new(&hit.title).strong()).clicked() {
                    go = Some(hit.id);
                }
                if !hit.context.is_empty() {
                    ui.weak(hit.context);
                }
                ui.add_space(4.0);
            }
            return go;
        }
        let anchor = self.anchor.take();
        match self.page(&self.current) {
            Some(page) => markdown::show_linked(ui, &page.text, anchor.as_deref()),
            None => {
                ui.label("Такой страницы нет.");
                ui.link("К содержанию").clicked().then(|| START.to_owned())
            }
        }
    }
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut help) = app.help.take() else {
        return;
    };
    if help.blocks_dirty {
        help.rebuild_blocks(&app.lib);
    }
    let mut open = true;
    let mut go = None;
    egui::Window::new("Справка")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(860.0)
        .default_height(580.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!help.history.is_empty(), egui::Button::new("← Назад"))
                    .clicked()
                {
                    if let Some(prev) = help.history.pop() {
                        help.current = prev;
                        help.anchor = None;
                        help.query.clear();
                    }
                }
                ui.add(
                    TextEdit::singleline(&mut help.query)
                        .hint_text("Поиск по справке")
                        .desired_width(240.0),
                );
                if !help.query.is_empty() && ui.button("×").clicked() {
                    help.query.clear();
                }
            });
            ui.separator();
            let height = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(210.0);
                    egui::ScrollArea::vertical()
                        .id_salt("help_tree")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| go = help.tree(ui));
                });
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("help_text")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| go = help.content(ui).or(go.take()));
                });
            });
        });
    if let Some(target) = go {
        help.goto(&target);
    }
    if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.help = Some(help);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str, title: &str, text: &str) -> Page {
        Page {
            id: id.into(),
            title: title.into(),
            text: text.into(),
        }
    }

    #[test]
    fn every_help_file_is_built_in() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../help");
        let files = std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                let path = e.as_ref().unwrap().path();
                path.extension().is_some_and(|x| x == "md")
            })
            .count();
        assert_eq!(files, FILES.len());
    }

    #[test]
    fn section_titles_come_from_headings() {
        for (id, text) in FILES {
            let page = section(id, text);
            assert_ne!(page.title, *id, "в разделе {id} нет заголовка «# …»");
        }
    }

    /// Every kind of check problem has its own subsection on the errors page, and
    /// the subsections hold nothing but those kinds.
    #[test]
    fn every_diag_kind_has_an_errors_subsection() {
        let text = FILES.iter().find(|(id, _)| *id == ERRORS).unwrap().1;
        let headings: Vec<String> = text
            .lines()
            .filter_map(|l| l.strip_prefix("### "))
            .map(markdown::slug)
            .collect();
        for &kind in DiagKind::ALL {
            let anchor = error_anchor(kind);
            assert!(
                headings.iter().any(|h| h == anchor),
                "нет подраздела «{anchor}» для вида {}",
                kind.id()
            );
            assert!(
                text.contains(&format!("*Вид `{}`.*", kind.id())),
                "в подразделе «{anchor}» не указан вид {}",
                kind.id()
            );
        }
        let anchors: Vec<_> = DiagKind::ALL.iter().map(|&k| error_anchor(k)).collect();
        for h in &headings {
            assert!(anchors.contains(&h.as_str()), "подраздел «{h}» без вида");
        }
    }

    /// Every `help:` link in the sections goes to an existing section, block list or anchor.
    #[test]
    fn links_between_sections_are_whole() {
        let pages: Vec<Page> = FILES.iter().map(|(id, t)| section(id, t)).collect();
        for page in &pages {
            let mut rest = page.text.as_str();
            while let Some(at) = rest.find("](help:") {
                rest = &rest[at + "](help:".len()..];
                let end = rest.find(')').expect("ссылка без закрывающей скобки");
                let target = &rest[..end];
                let (id, anchor) = match target.split_once('#') {
                    Some((id, anchor)) => (id, Some(anchor)),
                    None => (target, None),
                };
                if id == BLOCKS || id.starts_with("blocks/") {
                    continue;
                }
                let found = pages.iter().find(|p| p.id == id);
                let found = found.unwrap_or_else(|| panic!("{}: нет раздела «{id}»", page.id));
                if let Some(anchor) = anchor {
                    let ok = found
                        .text
                        .lines()
                        .filter_map(|l| l.trim_start_matches('#').strip_prefix(' '))
                        .any(|h| markdown::slug(h) == anchor);
                    assert!(ok, "{}: в «{id}» нет заголовка «{anchor}»", page.id);
                }
            }
        }
    }

    #[test]
    fn search_ignores_case_and_finds_cyrillic() {
        let pages = [
            page("a", "Схема", "# Схема\n\nБлоки соединяют связями."),
            page("b", "Сборка", "# Сборка\n\narduino-cli собирает скетч."),
        ];
        let hits = search(pages.iter(), "СВЯЗЯМИ");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "a");
        assert_eq!(hits[0].context, "Блоки соединяют связями.");
        assert_eq!(search(pages.iter(), "сборка").len(), 1);
        assert!(search(pages.iter(), "  ").is_empty());
        assert!(search(pages.iter(), "нет такого").is_empty());
    }

    #[test]
    fn block_without_docs_gets_a_page_from_block_toml() {
        let toml = "[block]\ntype = \"t\"\nname = \"Тест\"\ngroup = \"Группа\"\n\
                    description = \"Описание.\"\n\n[[input]]\nname = \"in\"\ntype = \"bool\"\n";
        let def: flp_core::BlockDef = toml::from_str(toml).unwrap();
        let block = LibraryBlock {
            def,
            dir: std::path::PathBuf::from("нет-такой-папки"),
            inline_templates: Default::default(),
        };
        let page = block_page(&block);
        assert_eq!(page.id, "blocks/t");
        assert!(page.text.starts_with("# Тест"));
        assert!(page.text.contains("## Входы"));
        assert!(!page.text.contains("## Параметры"));
    }
}
