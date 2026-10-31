//! Board editor: a user board of `user_library/boards/<id>/` — which chip it carries,
//! what is wired to each pin, the peripherals soldered on it (PCF8574 expanders as
//! ready-made blocks) and a description with pictures of the schemes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Id, RichText, ScrollArea, TextEdit, Ui};
use flp_core::boards::{
    boards_dir, save_board, BoardDef, BoardExpanderPinDef, BoardInfo, BoardPin, DeviceDef,
    UserBoard, DOCS_FILE,
};
use flp_core::pins::{Chip, PinFunction, PinInfo, BOARD_MODELS};
use flp_core::ParamValue;

use crate::app::{EditorApp, LogLevel};
use crate::pictures::Pictures;

/// Chip families a user board may be built on.
const CHIPS: [Chip; 3] = [Chip::Esp32, Chip::Esp32C3, Chip::Esp32S3];

/// Peripherals the editor knows how to set up: block type, caption, name of the new
/// block, prefix of the channel names.
const DEVICE_KINDS: [(&str, &str, &str, &str); 2] = [
    ("pcf8574_out", "Выходы на PCF8574 (реле)", "Реле", "K"),
    ("pcf8574_in", "Входы на PCF8574 (оптовходы)", "Входы", "X"),
];

/// Pictures a description may show.
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "bmp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Board,
    Pins,
    Devices,
    Docs,
}

/// One row of the pin table: a GPIO of the chip with what the author says about it.
#[derive(Debug, Clone, PartialEq)]
struct PinRow {
    info: PinInfo,
    /// Functions the board gives the pin: a subset of what the chip can do.
    functions: BTreeSet<PinFunction>,
    name: String,
    note: String,
    busy: String,
}

/// One row of the table of expander pins: a pin of the chip behind a device.
#[derive(Debug, Clone, PartialEq)]
struct ExpanderPinRow {
    /// Id of the device the chip belongs to.
    device: String,
    pin: i64,
    name: String,
    note: String,
    /// `Din` and/or `Dout`.
    functions: BTreeSet<PinFunction>,
}

/// What an expander pin can do: the chip only reads and drives levels.
const EXPANDER_FUNCTIONS: [PinFunction; 2] = [PinFunction::Din, PinFunction::Dout];

/// Everything the window edits.
#[derive(Debug, Clone, PartialEq)]
struct Form {
    id: String,
    name: String,
    chip: Chip,
    fqbn: String,
    version: u32,
    /// Board menu options, one `Key=Value` per line.
    options: String,
    pins: Vec<PinRow>,
    expander_pins: Vec<ExpanderPinRow>,
    devices: Vec<DeviceDef>,
    docs: String,
    pictures: Pictures,
}

pub struct BoardEditor {
    /// Id the board was loaded from or saved with; `None` for a board not saved yet.
    original: Option<String>,
    form: Form,
    /// The form as last loaded or saved.
    clean: Form,
    tab: Tab,
    /// The id was typed by hand: it no longer follows the name.
    id_by_hand: bool,
    /// Closing with unsaved changes: asking what to do.
    confirm_close: bool,
    /// Asking before the board folder is deleted.
    confirm_delete: bool,
}

impl PinRow {
    /// A pin with everything the chip can do on it checked.
    fn new(info: PinInfo) -> Self {
        Self {
            functions: info.functions().into_iter().collect(),
            info,
            name: String::new(),
            note: String::new(),
            busy: String::new(),
        }
    }
}

impl Form {
    fn new_board(id: String) -> Self {
        let chip = Chip::Esp32;
        Self {
            id,
            name: String::new(),
            chip,
            fqbn: default_fqbn(chip).to_owned(),
            version: 1,
            options: String::new(),
            pins: Vec::new(),
            expander_pins: Vec::new(),
            devices: Vec::new(),
            docs: String::new(),
            pictures: Pictures::default(),
        }
    }

    fn from_board(board: &UserBoard) -> Self {
        let def = &board.def;
        let chip = def.chip().unwrap_or(Chip::Esp32);
        let options = def
            .info
            .options
            .iter()
            .map(|(k, v)| format!("{k}={v}\n"))
            .collect();
        Self {
            id: def.info.id.clone(),
            name: def.info.name.clone(),
            chip,
            fqbn: def.info.fqbn.clone(),
            version: def.info.version,
            options,
            pins: pin_rows(chip, &def.pins),
            expander_pins: def
                .expander_pins
                .iter()
                .map(|p| ExpanderPinRow {
                    device: p.device.clone(),
                    pin: p.pin,
                    name: p.name.clone(),
                    note: p.note.clone(),
                    functions: p.functions.as_deref().map_or_else(
                        || EXPANDER_FUNCTIONS.into(),
                        |f| f.iter().copied().collect(),
                    ),
                })
                .collect(),
            devices: def.devices.clone(),
            docs: board
                .docs_path()
                .and_then(|p| fs::read_to_string(p).ok())
                .unwrap_or_default(),
            pictures: Pictures::in_dir(&board.dir),
        }
    }

    /// Changes the chip: the pins the new chip also has stay with their names, the
    /// functions the new chip lacks are unchecked.
    fn set_chip(&mut self, chip: Chip) {
        if self.chip == chip {
            return;
        }
        if self.fqbn.trim().is_empty() || self.fqbn == default_fqbn(self.chip) {
            self.fqbn = default_fqbn(chip).to_owned();
        }
        self.chip = chip;
        let old = std::mem::take(&mut self.pins);
        self.pins = old
            .into_iter()
            .filter_map(|mut row| {
                row.info = chip.pin(row.info.gpio).filter(|p| !p.flash)?;
                let info = &row.info;
                row.functions.retain(|f| info.chip_can(*f));
                Some(row)
            })
            .collect();
    }

    /// Adds a GPIO of the chip to the table with everything the chip can do on it
    /// checked. `false` if the chip has no such usable pin or it is in the table.
    fn add_pin(&mut self, gpio: i64) -> bool {
        let Some(info) = self.chip.pin(gpio).filter(|p| !p.flash) else {
            return false;
        };
        if self.pins.iter().any(|p| p.info.gpio == gpio) {
            return false;
        }
        self.pins.push(PinRow::new(info));
        self.pins.sort_by_key(|p| p.info.gpio);
        true
    }

    /// Devices whose chip is an expander, with the number of pins it has.
    fn expander_devices(&self) -> impl Iterator<Item = (&DeviceDef, i64)> {
        self.devices
            .iter()
            .filter_map(|d| Some((d, i64::from(d.expander()?.chip.pins()))))
    }

    /// Adds pin `pin` of the expander of device `device` to the table, named after the
    /// device's channel, with the function the device is for. `false` if there is no
    /// such device or pin, or the pin is in the table.
    fn add_expander_pin(&mut self, device: &str, pin: i64) -> bool {
        let Some((dev, pins)) = self.expander_devices().find(|(d, _)| d.id == device) else {
            return false;
        };
        let taken = self
            .expander_pins
            .iter()
            .any(|p| p.device == device && p.pin == pin);
        if taken || !(0..pins).contains(&pin) {
            return false;
        }
        let function = if dev.kind.ends_with("_in") {
            PinFunction::Din
        } else {
            PinFunction::Dout
        };
        let row = ExpanderPinRow {
            device: device.to_owned(),
            pin,
            name: usize::try_from(pin)
                .ok()
                .and_then(|i| dev.channels.get(i))
                .cloned()
                .unwrap_or_default(),
            note: String::new(),
            functions: BTreeSet::from([function]),
        };
        self.expander_pins.push(row);
        self.sort_expander_pins();
        true
    }

    /// Rows in the order of the devices, then by pin.
    fn sort_expander_pins(&mut self) {
        let devices = &self.devices;
        self.expander_pins
            .sort_by_key(|p| (devices.iter().position(|d| d.id == p.device), p.pin));
    }

    /// The device was removed: its pins go with it.
    fn forget_device(&mut self, device: &str) {
        self.expander_pins.retain(|p| p.device != device);
    }

    /// The device got another id: its pins follow.
    fn rename_device(&mut self, from: &str, to: &str) {
        for p in self.expander_pins.iter_mut().filter(|p| p.device == from) {
            p.device = to.to_owned();
        }
    }

    /// `board.toml` made of the form, or what is wrong with it (in Russian).
    fn to_def(&self) -> Result<BoardDef, String> {
        let mut options = BTreeMap::new();
        for line in self
            .options
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            let (key, value) = line.split_once('=').ok_or_else(|| {
                crate::i18n::fmt("настройка «{0}» — нужен вид Ключ=Значение", &[&line])
            })?;
            options.insert(key.trim().to_owned(), value.trim().to_owned());
        }
        if self.pins.is_empty() {
            return Err(
                crate::i18n::tr("добавьте хотя бы один вывод платы (вкладка «Выводы»)").to_owned(),
            );
        }
        let pins = self
            .pins
            .iter()
            .map(|p| BoardPin {
                gpio: p.info.gpio,
                name: p.name.trim().to_owned(),
                note: p.note.trim().to_owned(),
                busy: Some(p.busy.trim().to_owned()).filter(|b| !b.is_empty()),
                functions: Some(p.functions.iter().copied().collect()),
            })
            .collect();
        let devices = self
            .devices
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d.name = d.name.trim().to_owned();
                d.description = d.description.trim().to_owned();
                for (i, c) in d.channels.iter_mut().enumerate() {
                    *c = c.trim().to_owned();
                    if c.is_empty() {
                        *c = format!("P{i}");
                    }
                }
                d
            })
            .collect();
        let expander_pins = self
            .expander_pins
            .iter()
            .map(|p| BoardExpanderPinDef {
                device: p.device.clone(),
                pin: p.pin,
                name: p.name.trim().to_owned(),
                note: p.note.trim().to_owned(),
                functions: Some(p.functions.iter().copied().collect()),
            })
            .collect();
        let def = BoardDef {
            info: BoardInfo {
                id: self.id.trim().to_owned(),
                name: self.name.trim().to_owned(),
                chip: self.chip.id().unwrap_or("esp32").to_owned(),
                fqbn: self.fqbn.trim().to_owned(),
                version: self.version.max(1),
                options,
            },
            pins,
            expander_pins,
            devices,
        };
        def.validate(None)?;
        Ok(def)
    }
}

fn default_fqbn(chip: Chip) -> &'static str {
    match chip {
        Chip::Esp32C3 | Chip::Esp32C3SuperMini => "esp32:esp32:esp32c3",
        Chip::Esp32S3 => "esp32:esp32:esp32s3",
        Chip::Esp32 => "esp32:esp32:esp32",
    }
}

/// The pin table of the form. A board without `[[pin]]` brings out every pin of the
/// chip (those that are not wired to the flash), so the table starts with them.
fn pin_rows(chip: Chip, board: &[BoardPin]) -> Vec<PinRow> {
    if board.is_empty() {
        return chip
            .pins()
            .into_iter()
            .filter(|p| !p.flash)
            .map(PinRow::new)
            .collect();
    }
    let mut rows: Vec<PinRow> = board
        .iter()
        .filter_map(|p| {
            let mut row = PinRow::new(chip.pin(p.gpio)?);
            row.name.clone_from(&p.name);
            row.note.clone_from(&p.note);
            row.busy = p.busy.clone().unwrap_or_default();
            if let Some(f) = &p.functions {
                row.functions = f.iter().copied().collect();
            }
            Some(row)
        })
        .collect();
    rows.sort_by_key(|p| p.info.gpio);
    rows
}

/// Id for a board called `name`: Latin letters and digits of the name, `board` if none.
fn ascii_id(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out: String = out.trim_end_matches('_').chars().take(48).collect();
    if out.is_empty() {
        "board".to_owned()
    } else {
        out
    }
}

/// An id no board uses, made from `base`.
fn free_id(app: &EditorApp, base: &str) -> String {
    let user = app.user_lib_dir();
    let taken = |id: &str| {
        BOARD_MODELS.iter().any(|m| m.id == id)
            || app.lib.boards().get(id).is_some()
            || user
                .as_deref()
                .is_some_and(|u| boards_dir(u).join(id).exists())
    };
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|id| !taken(id))
        .expect("some id is free")
}

/// An id of a device not used on the board yet.
fn free_device_id(devices: &[DeviceDef], base: &str) -> String {
    let taken = |id: &str| devices.iter().any(|d| d.id == id);
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|id| !taken(id))
        .expect("some id is free")
}

impl BoardEditor {
    /// A new board.
    pub fn new_board(app: &EditorApp) -> Self {
        Self::with(None, Form::new_board(free_id(app, "board")))
    }

    /// Opens a user board for editing.
    pub fn edit(board: &UserBoard) -> Self {
        Self::with(Some(board.def.info.id.clone()), Form::from_board(board))
    }

    /// A new board based on a user board: its pins, peripherals and description.
    pub fn copy_of(app: &EditorApp, board: &UserBoard) -> Self {
        let mut form = Form::from_board(board);
        form.name = crate::i18n::fmt("{0} (копия)", &[&form.name]);
        form.id = free_id(app, &ascii_id(&form.name));
        form.version = 1;
        form.pictures = Pictures::copied_from(&board.dir);
        Self::with(None, form)
    }

    fn with(original: Option<String>, form: Form) -> Self {
        Self {
            original,
            clean: form.clone(),
            form,
            tab: Tab::Board,
            id_by_hand: false,
            confirm_close: false,
            confirm_delete: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.form != self.clean
    }

    /// Asks "save changes?" on the next frame, as when its window is closed.
    pub fn ask_to_close(&mut self) {
        self.confirm_close = true;
    }
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut ed) = app.board_editor.take() else {
        return;
    };
    let title = match &ed.original {
        Some(id) => crate::i18n::fmt("Редактор платы — {0}", &[&id]),
        None => crate::i18n::tr("Редактор платы — новая плата").to_owned(),
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new(title)
        .id(Id::unique("board_editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(860.0)
        .default_height(620.0)
        .show(ctx, |ui| close = body(app, ui, &mut ed));
    if !open && ed.dirty() {
        ed.confirm_close = true;
    } else if !open {
        close = true;
    }
    if ed.confirm_close && !close {
        close = confirm_close(app, ctx, &mut ed);
    }
    if ed.confirm_delete && !close {
        close = confirm_delete(app, ctx, &mut ed);
    }
    if !close {
        app.board_editor = Some(ed);
    }
}

/// Modal question on closing with unsaved changes; returns true when the window
/// should close (saved or changes dropped).
fn confirm_close(app: &mut EditorApp, ctx: &egui::Context, ed: &mut BoardEditor) -> bool {
    let def = ed.form.to_def();
    let name = ed.form.name.trim().to_owned();
    let error = app.theme.palette.error;
    let mut answer = None;
    egui::Modal::new(Id::unique("board_editor_close")).show(ctx, |ui| {
        ui.label(RichText::new(crate::i18n::tr("Сохранить изменения?")).strong());
        ui.label(crate::i18n::fmt(
            "В плате «{0}» есть несохранённые изменения.",
            &[&name],
        ));
        if let Err(e) = &def {
            ui.colored_label(error, crate::i18n::fmt("Сохранить нельзя: {0}", &[&e]));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(def.is_ok(), egui::Button::new(crate::i18n::tr("Сохранить")))
                .clicked()
            {
                answer = Some(1);
            }
            if ui.button(crate::i18n::tr("Выйти без сохранения")).clicked() {
                answer = Some(2);
            }
            if ui.button(crate::i18n::tr("Отмена")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(0);
            }
        });
    });
    match answer {
        Some(1) => {
            let saved = save(app, ed);
            ed.confirm_close = !saved;
            saved
        }
        Some(2) => true,
        Some(_) => {
            ed.confirm_close = false;
            false
        }
        None => false,
    }
}

/// Modal question before the folder of the board is deleted; true when it was.
fn confirm_delete(app: &mut EditorApp, ctx: &egui::Context, ed: &mut BoardEditor) -> bool {
    let Some(id) = ed.original.clone() else {
        ed.confirm_delete = false;
        return false;
    };
    let name = ed.form.name.trim().to_owned();
    let mut answer = None;
    egui::Modal::new(Id::unique("board_editor_delete")).show(ctx, |ui| {
        ui.label(RichText::new(crate::i18n::fmt("Удалить плату «{0}»?", &[&name])).strong());
        ui.label(crate::i18n::tr(
            "Папка платы будет удалена вместе с описанием и картинками. \
             Проекты на этой плате останутся, но блоки платы в них перестанут работать, \
             пока плату не вернут.",
        ));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::tr("Удалить")).clicked() {
                answer = Some(true);
            }
            if ui.button(crate::i18n::tr("Отмена")).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    match answer {
        Some(true) => {
            ed.confirm_delete = false;
            delete(app, &id)
        }
        Some(false) => {
            ed.confirm_delete = false;
            false
        }
        None => false,
    }
}

/// Window contents; returns true when the window should close.
fn body(app: &mut EditorApp, ui: &mut Ui, ed: &mut BoardEditor) -> bool {
    // Until the first save the id follows the name, unless it was typed by hand.
    if ed.original.is_none() && !ed.id_by_hand {
        let base = ascii_id(&ed.form.name);
        // The board's own id is not "taken" by itself.
        let id = if base == ed.form.id {
            base
        } else {
            free_id(app, &base)
        };
        ed.clean.id.clone_from(&id);
        ed.form.id = id;
    }
    if ed.tab == Tab::Docs {
        take_dropped_pictures(ui.ctx(), &mut ed.form);
    }
    let def = ed.form.to_def();
    let mut close = false;
    egui::Panel::bottom("board_editor_footer").show(ui, |ui| {
        status(ui, app, &def);
        close = footer(app, ui, ed, &def);
    });
    egui::Panel::top("board_editor_header").show(ui, |ui| {
        ui.horizontal(|ui| {
            let tabs = [
                (Tab::Board, crate::i18n::tr("Плата")),
                (Tab::Pins, crate::i18n::tr("Выводы")),
                (Tab::Devices, crate::i18n::tr("Периферия")),
                (Tab::Docs, crate::i18n::tr("Описание")),
            ];
            for (tab, text) in tabs {
                ui.selectable_value(&mut ed.tab, tab, text);
            }
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        ScrollArea::both()
            .id_salt("board_editor_tab")
            .auto_shrink([false, false])
            .show(ui, |ui| match ed.tab {
                Tab::Board => board_tab(ui, ed),
                Tab::Pins => pins_tab(ui, &mut ed.form),
                Tab::Devices => devices_tab(ui, &mut ed.form),
                Tab::Docs => crate::blockedit::docs_editor(
                    ui,
                    Id::unique("board_docs_text"),
                    &mut ed.form.docs,
                    &mut ed.form.pictures,
                    true,
                    crate::i18n::tr("платы"),
                ),
            });
    });
    close
}

/// Picture files dropped onto the window go into the description.
fn take_dropped_pictures(ctx: &egui::Context, form: &mut Form) {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|f| f.path().to_path_buf())
            .filter(|p| is_picture(p))
            .collect()
    });
    for file in dropped {
        let target = form.pictures.add(file);
        form.docs.push_str(&format!("\n![описание]({target})\n"));
    }
}

fn is_picture(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        IMAGE_EXTENSIONS
            .iter()
            .any(|known| e.eq_ignore_ascii_case(known))
    })
}

fn status(ui: &mut Ui, app: &EditorApp, def: &Result<BoardDef, String>) {
    let pal = &app.theme.palette;
    match def {
        Err(e) => ui.colored_label(pal.error, crate::i18n::fmt("Описание: {0}", &[&e])),
        Ok(_) => ui.colored_label(pal.ok, crate::i18n::tr("Описание платы в порядке")),
    };
}

/// Save, delete and close buttons; returns true when the window should close.
fn footer(
    app: &mut EditorApp,
    ui: &mut Ui,
    ed: &mut BoardEditor,
    def: &Result<BoardDef, String>,
) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        let can_save = def.is_ok() && (ed.dirty() || ed.original.is_none());
        if ui
            .add_enabled(can_save, egui::Button::new(crate::i18n::tr("Сохранить")))
            .clicked()
        {
            save(app, ed);
        }
        if ui.button(crate::i18n::tr("Закрыть")).clicked() {
            if ed.dirty() {
                ed.confirm_close = true;
            } else {
                close = true;
            }
        }
        ui.separator();
        let boards: Vec<UserBoard> = app.lib.boards().iter().cloned().collect();
        // The copy replaces this window, so unsaved changes must be saved first.
        ui.add_enabled_ui(!boards.is_empty() && !ed.dirty(), |ui| {
            ui.menu_button(
                crate::i18n::tr("Создать на основе ▾"),
                |ui| {
                    for b in &boards {
                        if ui.button(&b.def.info.name).clicked() {
                            app.board_editor = Some(BoardEditor::copy_of(app, b));
                            close = true;
                            ui.close();
                        }
                    }
                },
            )
            .response
            .on_hover_text(concat!(
                "Новая плата с выводами, периферией и описанием выбранной ",
                "(сначала сохраните изменения)",
            ));
        });
        if let Some(id) = ed.original.clone() {
            if ui
                .button(crate::i18n::tr("Экспорт..."))
                .on_hover_text(crate::i18n::tr(
                    "Сохранить плату одним файлом .crabboard (то, что сохранено в библиотеке)",
                ))
                .clicked()
            {
                crate::boardio::export(app, &id);
            }
        }
        if ed.original.is_some()
            && ui
                .button(crate::i18n::tr("Удалить..."))
                .on_hover_text(crate::i18n::tr("Удалить плату из библиотеки"))
                .clicked()
        {
            ed.confirm_delete = true;
        }
    });
    if let Some(dir) = board_dir(app, ed) {
        ui.weak(crate::i18n::fmt("Папка: {0}", &[&dir.display()]));
    }
    close
}

/// Folder of the board: where it lies or will be saved to.
fn board_dir(app: &EditorApp, ed: &BoardEditor) -> Option<PathBuf> {
    if let Some(b) = ed
        .original
        .as_deref()
        .and_then(|id| app.lib.boards().get(id))
    {
        return Some(b.dir.clone());
    }
    Some(boards_dir(&app.user_lib_dir()?).join(&ed.form.id))
}

fn board_tab(ui: &mut Ui, ed: &mut BoardEditor) {
    let saved = ed.original.is_some();
    let f = &mut ed.form;
    egui::Grid::new("board_main")
        .num_columns(2)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            ui.label(crate::i18n::tr("Название"));
            ui.add(
                TextEdit::singleline(&mut f.name)
                    .hint_text("Kincony KC868-A6")
                    .desired_width(300.0),
            )
            .on_hover_text(crate::i18n::tr(
                "Так плата называется в списке плат и в проектах",
            ));
            ui.end_row();

            ui.label(crate::i18n::tr("Идентификатор"));
            if saved {
                ui.label(RichText::new(&f.id).monospace())
                    .on_hover_text(crate::i18n::tr(
                        "Имя папки и значение board.model в проектах: после сохранения не меняется",
                    ));
            } else if ui
                .add(
                    TextEdit::singleline(&mut f.id)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(300.0),
                )
                .on_hover_text(crate::i18n::tr(
                    "Латинские буквы a–z, цифры и «_». Пока плата не сохранена, \
                     идентификатор делается из названия",
                ))
                .changed()
            {
                ed.id_by_hand = true;
            }
            ui.end_row();

            ui.label(crate::i18n::tr("Чип"));
            let mut chip = f.chip;
            egui::ComboBox::from_id_salt("board_chip")
                .selected_text(chip.name())
                .show_ui(ui, |ui| {
                    for c in CHIPS {
                        ui.selectable_value(&mut chip, c, c.name());
                    }
                });
            ui.end_row();
            f.set_chip(chip);

            ui.label(crate::i18n::tr("Плата (FQBN)"));
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut f.fqbn).desired_width(260.0))
                    .on_hover_text(crate::i18n::tr(
                        "Имя платы для arduino-cli, например esp32:esp32:esp32",
                    ));
                let chip = f.chip;
                ui.menu_button("▾", |ui| {
                    for (fqbn, name) in crate::boardcfg::BOARDS {
                        if Chip::from_fqbn(fqbn) != Some(chip) {
                            continue;
                        }
                        if ui.button(*name).on_hover_text(*fqbn).clicked() {
                            f.fqbn = (*fqbn).to_owned();
                            ui.close();
                        }
                    }
                });
            });
            ui.end_row();

            ui.label(crate::i18n::tr("Версия"));
            ui.add(egui::DragValue::new(&mut f.version).range(1..=9999))
                .on_hover_text(crate::i18n::tr(
                    "Увеличивайте, когда меняете выводы или периферию платы",
                ));
            ui.end_row();

            ui.label(crate::i18n::tr("Настройки меню"));
            ui.add(
                TextEdit::multiline(&mut f.options)
                    .font(egui::TextStyle::Monospace)
                    .hint_text("PartitionScheme=default\nFlashSize=4M")
                    .desired_rows(3)
                    .desired_width(300.0),
            )
            .on_hover_text(crate::i18n::tr(
                "Настройки меню платы для arduino-cli, по строке «Ключ=Значение». \
                 Подставляются, когда проекту выбирают эту плату",
            ));
            ui.end_row();
        });
}

/// What the chip says about a pin, for the pin table.
fn pin_features(info: &PinInfo) -> String {
    let mut out = Vec::new();
    match info.adc {
        1 => out.push("ADC1"),
        2 => out.push("ADC2"),
        _ => {}
    }
    if info.input_only {
        out.push("только вход");
    }
    if info.flash {
        out.push("флеш — нельзя");
    }
    if info.strapping {
        out.push("загрузка");
    }
    out.join(", ")
}

/// Column captions of the functions and what they stand for.
fn function_caption(function: PinFunction) -> &'static str {
    match function {
        PinFunction::Din => crate::i18n::tr("Вх"),
        PinFunction::Dout => crate::i18n::tr("Вых"),
        PinFunction::Pwm => crate::i18n::tr("ШИМ"),
        PinFunction::Adc => crate::i18n::tr("АЦП"),
        PinFunction::Dac => crate::i18n::tr("ЦАП"),
    }
}

fn pins_tab(ui: &mut Ui, f: &mut Form) {
    ui.weak(crate::i18n::tr(
        "Добавьте выводы, которые плата выводит наружу, назовите их так, как они подписаны \
         на плате, и отметьте, что вывод умеет. Отметить можно только то, что умеет сам чип. \
         В проекте блок видит только выводы с подходящей функцией.",
    ));
    ui.menu_button(crate::i18n::tr("+ Вывод ▾"), |ui| {
        let free: Vec<PinInfo> = f
            .chip
            .pins()
            .into_iter()
            .filter(|p| !p.flash && !f.pins.iter().any(|row| row.info.gpio == p.gpio))
            .collect();
        if free.is_empty() {
            ui.weak(crate::i18n::tr("Все выводы чипа уже добавлены"));
        }
        ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for info in free {
                let text = format!("GPIO{}   {}", info.gpio, pin_features(&info));
                if ui.button(text).clicked() {
                    f.add_pin(info.gpio);
                    ui.close();
                }
            }
        });
    });
    ui.add_space(4.0);
    if f.pins.is_empty() {
        ui.weak(crate::i18n::tr("Выводов нет."));
        expander_pins_section(ui, f);
        return;
    }
    let mut remove = None;
    egui::Grid::new("board_pins")
        .num_columns(11)
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("GPIO").strong());
            ui.label(RichText::new(crate::i18n::tr("Чип")).strong());
            ui.label(RichText::new(crate::i18n::tr("Название на плате")).strong());
            for function in PinFunction::ALL {
                ui.label(RichText::new(function_caption(function)).strong())
                    .on_hover_text(function.label());
            }
            ui.label(RichText::new(crate::i18n::tr("Заметка")).strong());
            ui.label(RichText::new(crate::i18n::tr("Занят чем (предупреждение)")).strong());
            ui.label("");
            ui.end_row();
            for (i, row) in f.pins.iter_mut().enumerate() {
                ui.label(RichText::new(format!("GPIO{}", row.info.gpio)).monospace());
                let features = pin_features(&row.info);
                let response = ui.label(RichText::new(&features).weak());
                if let Some(w) = &row.info.warning {
                    response.on_hover_text(w);
                }
                ui.add(TextEdit::singleline(&mut row.name).desired_width(140.0));
                for function in PinFunction::ALL {
                    let can = row.info.chip_can(function);
                    let mut on = can && row.functions.contains(&function);
                    let response = ui
                        .add_enabled(can, egui::Checkbox::without_text(&mut on))
                        .on_hover_text(if can {
                            function.label().to_owned()
                        } else {
                            crate::i18n::fmt("{0}: этого чип не умеет", &[&function.label()])
                        });
                    if response.changed() {
                        if on {
                            row.functions.insert(function);
                        } else {
                            row.functions.remove(&function);
                        }
                    }
                }
                ui.add(TextEdit::singleline(&mut row.note).desired_width(180.0));
                ui.add(
                    TextEdit::singleline(&mut row.busy)
                        .hint_text(crate::i18n::tr("например: шина I²C расширителей"))
                        .desired_width(200.0),
                )
                .on_hover_text(crate::i18n::tr(
                    "Если заполнено, блок на этом выводе получает предупреждение с этим текстом",
                ));
                if ui
                    .button("✕")
                    .on_hover_text(crate::i18n::tr("Убрать вывод"))
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        f.pins.remove(i);
    }
    expander_pins_section(ui, f);
}

/// Table of the pins of expanders soldered on the board (devices of the «Устройства» tab).
fn expander_pins_section(ui: &mut Ui, f: &mut Form) {
    ui.add_space(10.0);
    ui.label(RichText::new(crate::i18n::tr("Выводы расширителей")).strong());
    let devices: Vec<(String, String, Vec<String>, i64)> = f
        .expander_devices()
        .map(|(d, n)| (d.id.clone(), d.name.clone(), d.channels.clone(), n))
        .collect();
    if devices.is_empty() {
        ui.weak(crate::i18n::tr(
            "Расширителей на плате нет. Добавьте устройство PCF8574 на вкладке «Устройства» — \
             его выводы появятся здесь, и блок «Вход» или «Выход» сможет встать на них.",
        ));
        return;
    }
    ui.weak(crate::i18n::tr(
        "Какие выводы микросхем-расширителей плата выводит наружу. В проекте блок встаёт \
         на такой вывод в свойствах входа или выхода, а расширитель добавляется сам.",
    ));
    ui.menu_button(
        crate::i18n::tr("+ Вывод расширителя ▾"),
        |ui| {
            for (id, name, channels, count) in &devices {
                ui.menu_button(name, |ui| {
                    let free: Vec<i64> = (0..*count)
                        .filter(|n| {
                            !f.expander_pins
                                .iter()
                                .any(|p| p.device == *id && p.pin == *n)
                        })
                        .collect();
                    if free.is_empty() {
                        ui.weak(crate::i18n::tr("Все выводы уже добавлены"));
                    }
                    for pin in free {
                        let channel = usize::try_from(pin).ok().and_then(|i| channels.get(i));
                        let text = match channel {
                            Some(c) if !c.is_empty() => format!("P{pin} · {c}"),
                            _ => format!("P{pin}"),
                        };
                        if ui.button(text).clicked() {
                            f.add_expander_pin(id, pin);
                            ui.close();
                        }
                    }
                });
            }
        },
    );
    ui.add_space(4.0);
    if f.expander_pins.is_empty() {
        ui.weak(crate::i18n::tr("Выводов расширителей нет."));
        return;
    }
    let titles: BTreeMap<&str, &str> = devices
        .iter()
        .map(|(id, name, ..)| (id.as_str(), name.as_str()))
        .collect();
    let mut remove = None;
    egui::Grid::new("board_expander_pins")
        .num_columns(7)
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new(crate::i18n::tr("Устройство")).strong());
            ui.label(RichText::new(crate::i18n::tr("Вывод")).strong());
            ui.label(RichText::new(crate::i18n::tr("Название на плате")).strong());
            for function in EXPANDER_FUNCTIONS {
                ui.label(RichText::new(function_caption(function)).strong())
                    .on_hover_text(function.label());
            }
            ui.label(RichText::new(crate::i18n::tr("Заметка")).strong());
            ui.label("");
            ui.end_row();
            for (i, row) in f.expander_pins.iter_mut().enumerate() {
                ui.label(titles.get(row.device.as_str()).copied().unwrap_or("?"));
                ui.label(RichText::new(format!("P{}", row.pin)).monospace());
                ui.add(TextEdit::singleline(&mut row.name).desired_width(140.0));
                for function in EXPANDER_FUNCTIONS {
                    let mut on = row.functions.contains(&function);
                    if ui
                        .add(egui::Checkbox::without_text(&mut on))
                        .on_hover_text(function.label())
                        .changed()
                    {
                        if on {
                            row.functions.insert(function);
                        } else {
                            row.functions.remove(&function);
                        }
                    }
                }
                ui.add(TextEdit::singleline(&mut row.note).desired_width(180.0));
                if ui
                    .button("✕")
                    .on_hover_text(crate::i18n::tr("Убрать вывод"))
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        f.expander_pins.remove(i);
    }
}

fn int_param(dev: &DeviceDef, key: &str, default: i64) -> i64 {
    match dev.params.get(key) {
        Some(ParamValue::Int(v)) => *v,
        _ => default,
    }
}

fn bool_param(dev: &DeviceDef, key: &str) -> bool {
    matches!(dev.params.get(key), Some(ParamValue::Bool(true)))
}

/// Addresses of PCF8574 (0x20–0x27) and PCF8574A (0x38–0x3F).
fn pcf_addresses() -> impl Iterator<Item = i64> {
    (0x20..=0x27).chain(0x38..=0x3F)
}

fn address_caption(address: i64) -> String {
    let chip = if address >= 0x38 {
        "PCF8574A"
    } else {
        "PCF8574"
    };
    format!("0x{address:02X} ({chip})")
}

fn new_device(
    devices: &[DeviceDef],
    (kind, _, name, prefix): (&str, &str, &str, &str),
) -> DeviceDef {
    let base = kind.trim_start_matches("pcf8574_");
    DeviceDef {
        id: free_device_id(devices, if base == "out" { "relays" } else { "inputs" }),
        kind: kind.to_owned(),
        name: name.to_owned(),
        description: String::new(),
        channels: (1..=8).map(|i| format!("{prefix}{i}")).collect(),
        params: BTreeMap::from([
            ("address".to_owned(), ParamValue::Int(0x20)),
            ("sda".to_owned(), ParamValue::Int(-1)),
            ("scl".to_owned(), ParamValue::Int(-1)),
            ("inverted".to_owned(), ParamValue::Bool(false)),
        ]),
    }
}

fn devices_tab(ui: &mut Ui, f: &mut Form) {
    ui.weak(crate::i18n::tr(
        "Периферия, распаянная на плате: каждая запись становится блоком в библиотеке \
         («Название платы: Реле») с уже заданными адресом и выводами.",
    ));
    ui.menu_button(
        crate::i18n::tr("+ Добавить устройство ▾"),
        |ui| {
            for kind in DEVICE_KINDS {
                if ui.button(kind.1).clicked() {
                    let device = new_device(&f.devices, kind);
                    f.devices.push(device);
                    ui.close();
                }
            }
        },
    );
    ui.add_space(4.0);
    let mut remove = None;
    let board_pins: Vec<(i64, String)> = f
        .pins
        .iter()
        .filter(|p| !p.name.trim().is_empty())
        .map(|p| (p.info.gpio, p.name.trim().to_owned()))
        .collect();
    let ids: Vec<String> = f.devices.iter().map(|d| d.id.clone()).collect();
    for (i, dev) in f.devices.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&dev.name).strong());
                    ui.label(RichText::new(format!("({})", dev.kind)).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(crate::i18n::tr("Удалить")).clicked() {
                            remove = Some(i);
                        }
                    });
                });
                device_fields(ui, dev, &board_pins);
            });
        });
    }
    for (i, old) in ids.iter().enumerate() {
        if let Some(new) = f.devices.get(i).map(|d| d.id.clone()).filter(|n| n != old) {
            f.rename_device(old, &new);
        }
    }
    if let Some(i) = remove {
        let gone = f.devices.remove(i);
        f.forget_device(&gone.id);
    }
    if f.devices.is_empty() {
        ui.weak(crate::i18n::tr("Устройств нет."));
    }
}

fn device_fields(ui: &mut Ui, dev: &mut DeviceDef, board_pins: &[(i64, String)]) {
    egui::Grid::new("device")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label(crate::i18n::tr("Название блока"));
            ui.add(TextEdit::singleline(&mut dev.name).desired_width(240.0));
            ui.end_row();

            ui.label(crate::i18n::tr("Идентификатор"));
            ui.add(
                TextEdit::singleline(&mut dev.id)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(240.0),
            )
            .on_hover_text(crate::i18n::tr(
                "Часть типа блока: <плата>_<идентификатор>. Латинские буквы, цифры, «_»",
            ));
            ui.end_row();

            ui.label(crate::i18n::tr("Описание"));
            ui.add(TextEdit::singleline(&mut dev.description).desired_width(420.0))
                .on_hover_text(crate::i18n::tr(
                    "Подсказка блока в библиотеке; пусто — как у исходного блока",
                ));
            ui.end_row();

            if dev.kind.starts_with("pcf8574") {
                ui.label(crate::i18n::tr("Адрес I²C"));
                let mut address = int_param(dev, "address", 0x20);
                egui::ComboBox::from_id_salt("device_address")
                    .selected_text(address_caption(address))
                    .show_ui(ui, |ui| {
                        for a in pcf_addresses() {
                            ui.selectable_value(&mut address, a, address_caption(a));
                        }
                    });
                dev.params
                    .insert("address".into(), ParamValue::Int(address));
                ui.end_row();

                for (key, caption) in [
                    ("sda", crate::i18n::tr("Вывод SDA")),
                    ("scl", crate::i18n::tr("Вывод SCL")),
                ] {
                    ui.label(caption);
                    let mut gpio = int_param(dev, key, -1);
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut gpio).range(-1..=48))
                            .on_hover_text(crate::i18n::tr(
                                "Номер GPIO; -1 — выводы I²C по умолчанию",
                            ));
                        let name = board_pins.iter().find(|(g, _)| *g == gpio);
                        match name {
                            Some((_, name)) => ui.weak(name),
                            None if gpio < 0 => ui.weak(crate::i18n::tr("по умолчанию")),
                            None => ui.weak(""),
                        };
                    });
                    dev.params.insert(key.into(), ParamValue::Int(gpio));
                    ui.end_row();
                }

                ui.label(crate::i18n::tr("Инверсия"));
                let mut inverted = bool_param(dev, "inverted");
                ui.checkbox(&mut inverted, crate::i18n::tr("включается нулём на выводе"))
                    .on_hover_text(crate::i18n::tr("Реле Kincony включаются низким уровнем"));
                dev.params
                    .insert("inverted".into(), ParamValue::Bool(inverted));
                ui.end_row();
            } else {
                ui.label(crate::i18n::tr("Параметры"));
                ui.weak(crate::i18n::tr("Этого устройства — в board.toml"));
                ui.end_row();
            }

            ui.label(crate::i18n::tr("Каналы"));
            ui.horizontal_wrapped(|ui| {
                for (i, channel) in dev.channels.iter_mut().enumerate() {
                    ui.add(
                        TextEdit::singleline(channel)
                            .hint_text(format!("P{i}"))
                            .desired_width(48.0),
                    );
                }
                if ui
                    .add_enabled(dev.channels.len() < 8, egui::Button::new("+"))
                    .on_hover_text(crate::i18n::tr("Добавить канал"))
                    .clicked()
                {
                    dev.channels.push(format!("P{}", dev.channels.len()));
                }
                if ui
                    .add_enabled(dev.channels.len() > 1, egui::Button::new("−"))
                    .on_hover_text(crate::i18n::tr("Убрать последний канал"))
                    .clicked()
                {
                    dev.channels.pop();
                }
            });
            ui.end_row();
        });
}

/// Writes the board into the library and reloads it; returns whether it worked.
fn save(app: &mut EditorApp, ed: &mut BoardEditor) -> bool {
    let def = match ed.form.to_def() {
        Ok(def) => def,
        Err(e) => {
            app.info(
                LogLevel::Error,
                crate::i18n::msg("Плата не сохранена: {0}", &[&e]),
            );
            return false;
        }
    };
    let Some(user_dir) = app.user_lib_dir() else {
        app.info(
            LogLevel::Error,
            crate::i18n::tr("Плата не сохранена: нет папки библиотеки"),
        );
        return false;
    };
    let id = def.info.id.clone();
    let new = ed.original.as_deref() != Some(id.as_str());
    if new && (app.lib.boards().get(&id).is_some() || boards_dir(&user_dir).join(&id).exists()) {
        app.info(
            LogLevel::Error,
            crate::i18n::msg(
                "Плата не сохранена: идентификатор «{0}» уже занят, выберите другой",
                &[&id],
            ),
        );
        return false;
    }
    match write_board(&user_dir, &def, &mut ed.form) {
        Ok(dir) => {
            app.info(
                LogLevel::Ok,
                crate::i18n::msg(
                    "Плата «{0}» сохранена в {1}",
                    &[&def.info.name, &dir.display()],
                ),
            );
            ed.original = Some(id);
            ed.clean = ed.form.clone();
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(
                LogLevel::Error,
                crate::i18n::msg("Плата не сохранена: {0}", &[&e]),
            );
            false
        }
    }
}

/// `board.toml`, `docs.md` and `images/` of the board in its folder under `user_dir`.
fn write_board(user_dir: &Path, def: &BoardDef, form: &mut Form) -> Result<PathBuf, String> {
    let dir = save_board(user_dir, def).map_err(|e| e.to_string())?;
    let docs = dir.join(DOCS_FILE);
    if form.docs.trim().is_empty() {
        let _ = fs::remove_file(&docs);
    } else {
        fs::write(&docs, &form.docs).map_err(|e| format!("{}: {e}", docs.display()))?;
    }
    form.pictures.apply(&dir, &form.docs)?;
    Ok(dir)
}

/// Deletes the folder of the user board `id`; returns whether it is gone.
fn delete(app: &mut EditorApp, id: &str) -> bool {
    let (Some(board), Some(user_dir)) = (app.lib.boards().get(id).cloned(), app.user_lib_dir())
    else {
        return false;
    };
    let root = boards_dir(&user_dir);
    if !board.dir.starts_with(&root) || board.dir == root {
        app.info(
            LogLevel::Error,
            crate::i18n::msg("Плата «{0}» не удалена: её папка вне библиотеки", &[&id]),
        );
        return false;
    }
    match fs::remove_dir_all(&board.dir) {
        Ok(()) => {
            app.info(
                LogLevel::Ok,
                crate::i18n::msg("Плата «{0}» удалена", &[&board.def.info.name]),
            );
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(
                LogLevel::Error,
                crate::i18n::msg(
                    "Плата «{0}» не удалена: {1}: {2}",
                    &[&id, &board.dir.display(), &e],
                ),
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use flp_core::Library;

    use super::*;

    fn user_dir(name: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("flp_boardedit_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        // The device blocks of the boards are configured copies of these built-in blocks.
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../library");
        for block in ["pcf8574_out", "pcf8574_in"] {
            let to = builtin.join(block);
            fs::create_dir_all(&to).unwrap();
            for file in fs::read_dir(source.join(block)).unwrap().flatten() {
                if file.path().is_file() {
                    fs::copy(file.path(), to.join(file.file_name())).unwrap();
                }
            }
        }
        let user = flp_core::user_library_dir(&builtin);
        (builtin, user)
    }

    fn filled_form() -> Form {
        let mut form = Form::new_board("kc868_a6".into());
        form.name = "Kincony KC868-A6".into();
        for gpio in [39, 4, 36, 15] {
            assert!(form.add_pin(gpio));
        }
        for row in &mut form.pins {
            if [4, 15, 36].contains(&row.info.gpio) {
                row.name = format!("N{}", row.info.gpio);
            }
            if row.info.gpio == 15 {
                row.busy = "шина I2C".into();
            }
            if row.info.gpio == 36 {
                // the board gives it to the analog input only
                row.functions = [PinFunction::Adc].into();
            }
        }
        let mut relays = new_device(&[], DEVICE_KINDS[0]);
        relays
            .params
            .insert("address".into(), ParamValue::Int(0x24));
        relays
            .params
            .insert("inverted".into(), ParamValue::Bool(true));
        form.devices.push(relays);
        // Relays 1 and 3 are brought out; the board's name of the pin is the channel's.
        assert!(form.add_expander_pin("relays", 2));
        assert!(form.add_expander_pin("relays", 0));
        // A picture is a block of its own: it stands on a line alone.
        form.docs = "Описание\n\n![схема](images/scheme.png)\n".into();
        form
    }

    #[test]
    fn a_new_board_starts_with_an_empty_pin_table() {
        let mut form = Form::new_board("mine".into());
        form.name = "Моя плата".into();
        assert!(form.pins.is_empty());
        assert!(form.to_def().unwrap_err().contains("хотя бы один вывод"));
        // A pin comes with what the chip can do on it; twice, flash and foreign ones do not go in.
        assert!(form.add_pin(25));
        assert!(!form.add_pin(25));
        assert!(!form.add_pin(6));
        assert!(!form.add_pin(40));
        assert_eq!(form.pins[0].functions, BTreeSet::from(PinFunction::ALL));
        form.pins[0].functions.remove(&PinFunction::Pwm);
        form.pins[0].name = "DAC1".into();
        let def = form.to_def().unwrap();
        assert_eq!(def.pins.len(), 1);
        assert_eq!(
            def.pins[0].functions.as_deref(),
            Some(
                &[
                    PinFunction::Din,
                    PinFunction::Dout,
                    PinFunction::Adc,
                    PinFunction::Dac
                ][..]
            )
        );
        // Pins stay in GPIO order.
        assert!(form.add_pin(4));
        assert_eq!(form.pins[0].info.gpio, 4);
        // No name — no board.
        form.name.clear();
        assert!(form.to_def().is_err());
        // A bad menu line is explained.
        form.name = "x".into();
        form.options = "PartitionScheme".into();
        assert!(form.to_def().unwrap_err().contains("Ключ=Значение"));
    }

    #[test]
    fn expander_pins_follow_their_devices() {
        let mut form = filled_form();
        // Sorted by pin, named after the channel, a relay is an output.
        let rows: Vec<_> = form
            .expander_pins
            .iter()
            .map(|p| (p.pin, p.name.as_str()))
            .collect();
        assert_eq!(rows, [(0, "K1"), (2, "K3")]);
        assert_eq!(
            form.expander_pins[0].functions,
            BTreeSet::from([PinFunction::Dout])
        );
        // Twice, a pin the chip lacks, a device that is not there.
        assert!(!form.add_expander_pin("relays", 2));
        assert!(!form.add_expander_pin("relays", 8));
        assert!(!form.add_expander_pin("nowhere", 1));
        // Only the board's own wording goes to the file.
        form.expander_pins[0].name = " Реле 1 ".into();
        form.expander_pins[0].functions.insert(PinFunction::Din);
        let def = form.to_def().unwrap();
        assert_eq!(def.expander_pins.len(), 2);
        assert_eq!(def.expander_pins[0].name, "Реле 1");
        assert_eq!(
            def.expander_pins[0].functions.as_deref(),
            Some(&[PinFunction::Din, PinFunction::Dout][..])
        );
        // The device got another id: the pins follow it; removed: they go.
        form.rename_device("relays", "k");
        form.devices[0].id = "k".into();
        assert!(form.to_def().is_ok());
        assert!(form.expander_pins.iter().all(|p| p.device == "k"));
        form.forget_device("k");
        assert!(form.expander_pins.is_empty());
    }

    #[test]
    fn an_orphan_expander_pin_is_refused_in_russian() {
        let mut form = filled_form();
        form.expander_pins[0].device = "ghost".into();
        assert!(form.to_def().unwrap_err().contains("нет устройства"));
        let mut form = filled_form();
        form.expander_pins[0].functions.insert(PinFunction::Pwm);
        assert!(form.to_def().unwrap_err().contains("только вход и выход"));
    }

    #[test]
    fn saving_writes_the_board_and_loading_gives_the_same_form() {
        let (builtin, user) = user_dir("save");
        let pic = builtin.join("scheme.png");
        fs::write(&pic, [1u8, 2, 3]).unwrap();
        let mut form = filled_form();
        form.pictures.add(pic);
        let def = form.to_def().unwrap();
        assert_eq!(def.pins.len(), 4);
        assert_eq!(def.devices[0].channels.len(), 8);

        let dir = write_board(&user, &def, &mut form).unwrap();
        assert_eq!(dir, boards_dir(&user).join("kc868_a6"));
        assert_eq!(fs::read(dir.join("images/scheme.png")).unwrap(), [1, 2, 3]);
        assert!(fs::read_to_string(dir.join("docs.md"))
            .unwrap()
            .contains("images/scheme.png"));

        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let board = lib.boards().get("kc868_a6").unwrap();
        let loaded = Form::from_board(board);
        assert_eq!(loaded.to_def().unwrap(), def);
        assert_eq!(loaded.docs, form.docs);
        assert_eq!(
            loaded.pictures.path("images/scheme.png"),
            Some(dir.join("images/scheme.png"))
        );

        // A copy takes the pictures along and gets a free id of its own.
        let mut copy = loaded.clone();
        copy.pictures = Pictures::copied_from(&board.dir);
        copy.id = "kc868_a6_2".into();
        let def = copy.to_def().unwrap();
        let dir = write_board(&user, &def, &mut copy).unwrap();
        assert!(dir.join("images/scheme.png").is_file());

        // The description without the picture drops the file.
        let mut loaded = loaded;
        loaded.docs = "Просто текст".into();
        let def = loaded.to_def().unwrap();
        let dir = write_board(&user, &def, &mut loaded).unwrap();
        assert!(!dir.join("images").exists());
        let _ = fs::remove_dir_all(builtin.parent().unwrap());
    }

    #[test]
    fn an_old_board_without_a_table_shows_every_usable_pin_of_the_chip() {
        let (builtin, _) = user_dir("old");
        let def = BoardDef {
            info: BoardInfo {
                id: "old".into(),
                name: "Старая".into(),
                chip: "esp32".into(),
                fqbn: "esp32:esp32:esp32".into(),
                version: 1,
                options: BTreeMap::new(),
            },
            pins: Vec::new(),
            expander_pins: Vec::new(),
            devices: Vec::new(),
        };
        let board = UserBoard {
            def,
            dir: builtin.clone(),
        };
        let form = Form::from_board(&board);
        assert_eq!(
            form.pins.len(),
            Chip::Esp32.pins().iter().filter(|p| !p.flash).count()
        );
        let p36 = form.pins.iter().find(|p| p.info.gpio == 36).unwrap();
        assert_eq!(
            p36.functions,
            BTreeSet::from([PinFunction::Din, PinFunction::Adc])
        );
        assert!(form.to_def().is_ok());
        let _ = fs::remove_dir_all(builtin.parent().unwrap());
    }

    #[test]
    fn the_chip_change_keeps_what_the_new_chip_also_has() {
        let mut form = Form::new_board("mine".into());
        assert!(form.add_pin(4) && form.add_pin(25) && form.add_pin(34));
        form.pins
            .iter_mut()
            .find(|p| p.info.gpio == 4)
            .unwrap()
            .name = "OUT".into();
        form.set_chip(Chip::Esp32C3);
        assert_eq!(form.fqbn, "esp32:esp32:esp32c3");
        assert_eq!(
            form.pins.iter().find(|p| p.info.gpio == 4).unwrap().name,
            "OUT"
        );
        // The pins of the old chip that the new one has not are gone, and so are
        // the functions it lacks: GPIO4 of the C3 has no PWM less than the ESP32, but
        // the DAC of GPIO25 would not survive on a chip that has the pin.
        assert!(form
            .pins
            .iter()
            .all(|p| Chip::Esp32C3.pin(p.info.gpio).is_some()));
        assert!(form
            .pins
            .iter()
            .all(|p| !p.functions.contains(&PinFunction::Dac)));
        assert!(form.pins.iter().all(|p| p.info.gpio != 34));
        // A hand-typed FQBN stays.
        form.fqbn = "esp32:esp32:mine".into();
        form.set_chip(Chip::Esp32S3);
        assert_eq!(form.fqbn, "esp32:esp32:mine");
    }

    #[test]
    fn ids_are_plain_ascii_and_devices_do_not_repeat() {
        assert_eq!(ascii_id("Kincony KC868-A6"), "kincony_kc868_a6");
        assert_eq!(ascii_id("Моя плата"), "board");
        assert_eq!(ascii_id("Плата (A6)"), "a6");
        let mut devices = vec![new_device(&[], DEVICE_KINDS[0])];
        devices.push(new_device(&devices, DEVICE_KINDS[0]));
        assert_eq!(
            devices.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["relays", "relays_2"]
        );
    }
}
