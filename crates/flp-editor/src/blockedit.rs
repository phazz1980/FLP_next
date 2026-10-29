//! "Block editor" window: create a library block or change one — header, inputs,
//! outputs, parameters, the ESP32 code template and docs — with a live picture of
//! the block and a trial generation of its code.

use std::fs;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Id, RichText, ScrollArea, TextEdit, Ui};
use flp_core::ident::ident_from_name;
use flp_core::library::ESP32_CPP_TEMPLATE;
use flp_core::vars::{VAR_GET, VAR_SET};
use flp_core::{
    BlockDef, Choice, LibraryBlock, ParamDef, ParamKind, ParamShow, ParamValue, PinMode, PortDef,
    SignalType,
};

use crate::app::{EditorApp, LogLevel};
use crate::dialogs::{type_name, value_text};
use crate::theme::Palette;

const PARAM_KINDS: [ParamKind; 6] = [
    ParamKind::Bool,
    ParamKind::Int,
    ParamKind::Float,
    ParamKind::Text,
    ParamKind::Duration,
    ParamKind::Pin,
];

const NEW_TEMPLATE: &str =
    "// @global\n\n// @setup\n\n// @loop\n{{ output.out }} = {{ input.in }};\n";

const SECTIONS_HINT: &str = "Разделы: // @global — переменные блока, // @setup — код в setup(), \
// @loop — код цикла, // @functions — функции. С пометкой once (// @global once) раздел \
попадает в скетч один раз на все блоки. Имена в разделах без once начинайте с {{ id }}.";

fn show_label(show: ParamShow) -> &'static str {
    match show {
        ParamShow::Named => "имя=значение",
        ParamShow::Value => "значение",
        ParamShow::Secret => "пароль (p***1)",
        ParamShow::Hidden => "не показывать",
    }
}

fn kind_label(kind: ParamKind) -> &'static str {
    match kind {
        ParamKind::Bool => "bool — да/нет",
        ParamKind::Int => "int — целое",
        ParamKind::Float => "float — дробное",
        ParamKind::Text => "string — строка",
        ParamKind::Duration => "duration — время",
        ParamKind::Pin => "pin — вывод",
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PortRow {
    name: String,
    /// Caption in Russian; empty: the name is shown.
    label: String,
    ty: SignalType,
    /// Empty: no default (inputs only).
    default: String,
    when: String,
    description: String,
}

impl PortRow {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            label: String::new(),
            ty: SignalType::Bool,
            default: String::new(),
            when: String::new(),
            description: String::new(),
        }
    }

    fn from_def(p: &PortDef) -> Self {
        Self {
            name: p.name.clone(),
            label: p.label.clone().unwrap_or_default(),
            ty: p.ty,
            default: p.default.as_ref().map(value_text).unwrap_or_default(),
            when: p.when.clone().unwrap_or_default(),
            description: p.description.clone(),
        }
    }

    fn to_def(&self) -> Result<PortDef, String> {
        Ok(PortDef {
            name: self.name.trim().to_owned(),
            label: optional(&self.label),
            ty: self.ty,
            default: parse_signal(self.ty, &self.default)
                .map_err(|e| format!("«{}»: {e}", self.name))?,
            when: optional(&self.when),
            description: self.description.trim().to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ParamRow {
    name: String,
    label: String,
    kind: ParamKind,
    default: String,
    min: String,
    max: String,
    when: String,
    description: String,
    show: ParamShow,
    /// `local` and `[[param.choice]]` from `block.toml`, kept as is (no column for them).
    local: Option<String>,
    choices: Vec<Choice>,
    /// For pins: direction and sharing, edited next to the type.
    mode: PinMode,
    shared: bool,
}

impl ParamRow {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            label: String::new(),
            kind: ParamKind::Int,
            default: String::new(),
            min: String::new(),
            max: String::new(),
            when: String::new(),
            description: String::new(),
            show: ParamShow::Named,
            local: None,
            choices: Vec::new(),
            mode: PinMode::Io,
            shared: false,
        }
    }

    fn from_def(p: &ParamDef) -> Self {
        let num = |v: Option<f64>| v.map(|v| format!("{v}")).unwrap_or_default();
        Self {
            name: p.name.clone(),
            label: p.label.clone().unwrap_or_default(),
            kind: p.kind,
            default: p.default.as_ref().map(value_text).unwrap_or_default(),
            min: num(p.min),
            max: num(p.max),
            when: p.when.clone().unwrap_or_default(),
            description: p.description.clone(),
            show: p.show,
            local: p.local.clone(),
            choices: p.choices.clone(),
            mode: p.mode,
            shared: p.shared,
        }
    }

    fn to_def(&self) -> Result<ParamDef, String> {
        let err = |e: String| format!("«{}»: {e}", self.name);
        Ok(ParamDef {
            name: self.name.trim().to_owned(),
            label: optional(&self.label),
            kind: self.kind,
            default: parse_param(self.kind, &self.default).map_err(err)?,
            min: parse_number(&self.min).map_err(err)?,
            max: parse_number(&self.max).map_err(err)?,
            when: optional(&self.when),
            description: self.description.trim().to_owned(),
            show: self.show,
            local: self.local.clone(),
            mode: if self.kind == ParamKind::Pin {
                self.mode
            } else {
                PinMode::Io
            },
            shared: self.kind == ParamKind::Pin && self.shared,
            runtime: false,
            choices: self.choices.clone(),
        })
    }
}

/// Everything the window edits; compared with the saved copy to detect changes.
#[derive(Debug, Clone, PartialEq)]
struct Form {
    /// Type of the block: made from the name for a new block, fixed afterwards
    /// (schemas refer to it).
    kind: String,
    name: String,
    symbol: String,
    /// Section and folders in the user library (`Раздел/Папка`), empty in the root.
    group: String,
    description: String,
    memory: bool,
    single: bool,
    /// `[block.uart]`: kept as it is, the form has no field for it yet.
    uart: Option<flp_core::UartUse>,
    /// Comma-separated.
    includes: String,
    libraries: String,
    inputs: Vec<PortRow>,
    outputs: Vec<PortRow>,
    params: Vec<ParamRow>,
    template: String,
    docs: String,
    /// Arduino libraries and the example attached to the block.
    attach: Attach,
}

/// Files that live next to `block.toml`: `lib/<Name>/...` and `example.flpn`.
#[derive(Debug, Clone, PartialEq, Default)]
struct Attach {
    libs: Vec<AttachedLib>,
    example: Example,
    /// Pictures of the docs not in the block folder yet: name in `images/` → file to copy.
    images: Vec<(String, PathBuf)>,
    /// The block folder, where the saved pictures are.
    home: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
struct AttachedLib {
    name: String,
    /// Folder to copy in on save; `None` - already in the block folder.
    src: Option<PathBuf>,
    /// Header put into the block's includes when the library was attached.
    header: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
enum Example {
    #[default]
    None,
    /// Already in the block folder.
    Saved,
    /// Project file to copy in on save.
    New(PathBuf),
}

impl Attach {
    fn from_dir(dir: &Path) -> Self {
        let mut libs: Vec<AttachedLib> = fs::read_dir(dir.join(flp_core::blockfile::LIB_DIR))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| AttachedLib {
                name: e.file_name().to_string_lossy().into_owned(),
                src: None,
                header: None,
            })
            .collect();
        libs.sort_by(|a, b| a.name.cmp(&b.name));
        let example = if dir.join(flp_core::blockfile::EXAMPLE_FILE).is_file() {
            Example::Saved
        } else {
            Example::None
        };
        Self {
            libs,
            example,
            images: Vec::new(),
            home: Some(dir.to_owned()),
        }
    }

    /// The same attachments for a copy of the block: everything is copied from `dir`.
    fn copied_from(dir: &Path) -> Self {
        let mut a = Self::from_dir(dir);
        for lib in &mut a.libs {
            lib.src = Some(dir.join(flp_core::blockfile::LIB_DIR).join(&lib.name));
        }
        if a.example == Example::Saved {
            a.example = Example::New(dir.join(flp_core::blockfile::EXAMPLE_FILE));
        }
        let pictures = fs::read_dir(dir.join(flp_core::blockfile::IMAGES_DIR));
        for e in pictures.into_iter().flatten().flatten() {
            if e.path().is_file() {
                let name = e.file_name().to_string_lossy().into_owned();
                a.images.push((name, e.path()));
            }
        }
        a.home = None;
        a
    }

    /// File of a docs picture `images/<name>`: waiting to be saved or already in the folder.
    fn image_path(&self, target: &str) -> Option<PathBuf> {
        let name = target.strip_prefix("images/")?;
        if name.contains('/') || !crate::markdown::is_local_image(target) {
            return None;
        }
        if let Some((_, src)) = self.images.iter().find(|(n, _)| n == name) {
            return Some(src.clone());
        }
        let home = self.home.as_ref()?;
        Some(home.join(flp_core::blockfile::IMAGES_DIR).join(name))
    }

    /// Takes a picture file into the docs; returns its target `images/<name>`.
    fn add_image(&mut self, src: PathBuf) -> String {
        // A plain ASCII name: the target stays easy to type and safe in the docs and the archive.
        let mut stem = String::new();
        for c in src
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
        {
            if c.is_ascii_alphanumeric() || c == '-' {
                stem.push(c);
            } else if !stem.ends_with('_') {
                stem.push('_');
            }
        }
        let stem = stem.trim_matches('_');
        let stem = if stem.is_empty() { "image" } else { stem };
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| "png".into());
        let mut name = format!("{stem}.{ext}");
        let mut n = 1;
        while self
            .image_path(&format!("images/{name}"))
            .is_some_and(|p| p.is_file() || self.images.iter().any(|(i, _)| *i == name))
        {
            n += 1;
            name = format!("{stem}_{n}.{ext}");
        }
        self.images.push((name.clone(), src));
        format!("images/{name}")
    }

    /// Makes the block folder `dir` hold exactly these attachments and the pictures `docs` uses.
    fn apply(&mut self, dir: &Path, docs: &str) -> Result<(), String> {
        let lib_root = dir.join(flp_core::blockfile::LIB_DIR);
        let err = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        if let Ok(entries) = fs::read_dir(&lib_root) {
            for e in entries.flatten() {
                if !self.libs.iter().any(|l| e.file_name() == *l.name) {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
        }
        for lib in &mut self.libs {
            let Some(src) = lib.src.take() else { continue };
            let dest = lib_root.join(&lib.name);
            let _ = fs::remove_dir_all(&dest);
            copy_dir(&src, &dest).map_err(|e| err(&src, e))?;
        }
        if self.libs.is_empty() {
            let _ = fs::remove_dir(&lib_root);
        }
        let example = dir.join(flp_core::blockfile::EXAMPLE_FILE);
        match &self.example {
            Example::None => {
                let _ = fs::remove_file(&example);
            }
            Example::Saved => {}
            Example::New(src) => {
                fs::copy(src, &example).map_err(|e| err(src, e))?;
                self.example = Example::Saved;
            }
        }
        self.apply_images(dir, docs)?;
        self.home = Some(dir.to_owned());
        Ok(())
    }

    /// Copies the pictures the docs use into `images/` and drops the others.
    fn apply_images(&mut self, dir: &Path, docs: &str) -> Result<(), String> {
        let root = dir.join(flp_core::blockfile::IMAGES_DIR);
        let used: Vec<String> = crate::markdown::image_targets(docs)
            .iter()
            .filter_map(|t| t.strip_prefix("images/").map(str::to_owned))
            .collect();
        for (name, src) in std::mem::take(&mut self.images) {
            if !used.contains(&name) || src == root.join(&name) {
                continue;
            }
            let err = |e: std::io::Error| format!("{}: {e}", src.display());
            fs::create_dir_all(&root).map_err(err)?;
            fs::copy(&src, root.join(&name)).map_err(err)?;
        }
        for e in fs::read_dir(&root).into_iter().flatten().flatten() {
            if !used.contains(&e.file_name().to_string_lossy().into_owned()) {
                let _ = fs::remove_file(e.path());
            }
        }
        let _ = fs::remove_dir(&root);
        Ok(())
    }
}

fn copy_dir(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dest.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

impl Form {
    fn new_block(kind: String, group: String) -> Self {
        Self {
            kind,
            name: "Новый блок".into(),
            symbol: String::new(),
            group,
            description: String::new(),
            memory: false,
            single: false,
            uart: None,
            includes: String::new(),
            libraries: String::new(),
            inputs: vec![PortRow::new("in")],
            outputs: vec![PortRow::new("out")],
            params: Vec::new(),
            template: NEW_TEMPLATE.into(),
            docs: String::new(),
            attach: Attach::default(),
        }
    }

    fn from_block(block: &LibraryBlock) -> Self {
        let def = &block.def;
        let info = &def.info;
        let template = block
            .template(ESP32_CPP_TEMPLATE)
            .and_then(Result::ok)
            .unwrap_or_default();
        let docs = block
            .docs_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        Self {
            kind: info.kind.clone(),
            name: info.name.clone(),
            symbol: info.symbol.clone().unwrap_or_default(),
            group: info.group.clone(),
            description: info.description.clone(),
            memory: info.memory,
            single: info.single,
            uart: info.uart.clone(),
            includes: info.includes.join(", "),
            libraries: info.libraries.join(", "),
            inputs: def.inputs.iter().map(PortRow::from_def).collect(),
            outputs: def.outputs.iter().map(PortRow::from_def).collect(),
            params: def.params.iter().map(ParamRow::from_def).collect(),
            template,
            docs,
            attach: Attach::from_dir(&block.dir),
        }
    }

    /// Group (`input`, `output`, `param`) and trimmed name of every row, in table order.
    fn row_names(&self) -> Vec<(&'static str, String)> {
        let rows = |group, names: Vec<&String>| {
            names
                .into_iter()
                .map(|n| (group, n.trim().to_owned()))
                .collect::<Vec<_>>()
        };
        let mut all = rows("input", self.inputs.iter().map(|p| &p.name).collect());
        all.extend(rows(
            "output",
            self.outputs.iter().map(|p| &p.name).collect(),
        ));
        all.extend(rows("param", self.params.iter().map(|p| &p.name).collect()));
        all
    }

    /// Block definition from the form, checked the same way the library checks it.
    fn to_def(&self) -> Result<BlockDef, String> {
        let kind = self.kind.trim().to_owned();
        if self.name.trim().is_empty() {
            return Err("у блока должно быть название".into());
        }
        if kind == VAR_GET || kind == VAR_SET {
            return Err(format!("тип «{kind}» занят встроенным блоком переменной"));
        }
        let def = BlockDef {
            info: flp_core::library::BlockInfo {
                kind: kind.clone(),
                name: self.name.trim().to_owned(),
                symbol: optional(&self.symbol),
                group: self.group.trim().to_owned(),
                shared: String::new(),
                version: 1,
                description: self.description.trim().to_owned(),
                memory: self.memory,
                includes: list(&self.includes),
                libraries: list(&self.libraries),
                single: self.single,
                hidden: false,
                pullable: false,
                uart: self.uart.clone(),
            },
            inputs: self
                .inputs
                .iter()
                .map(PortRow::to_def)
                .collect::<Result<_, _>>()?,
            outputs: self
                .outputs
                .iter()
                .map(PortRow::to_def)
                .collect::<Result<_, _>>()?,
            params: self
                .params
                .iter()
                .map(ParamRow::to_def)
                .collect::<Result<_, _>>()?,
        };
        def.validate(&kind)?;
        Ok(def)
    }
}

fn optional(text: &str) -> Option<String> {
    let t = text.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn list(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Main header of an Arduino library folder: `<name>.h` in `src/` (or the folder
/// itself for old layouts), else the only header there.
fn library_header(dir: &Path, name: &str) -> Option<String> {
    [dir.join("src"), dir.to_path_buf()]
        .into_iter()
        .find_map(|d| {
            let mut headers: Vec<String> = fs::read_dir(d)
                .ok()?
                .flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|f| f.to_lowercase().ends_with(".h"))
                .collect();
            headers.sort();
            let named = headers
                .iter()
                .find(|h| h[..h.len() - 2].eq_ignore_ascii_case(name));
            match (named, headers.len()) {
                (Some(h), _) => Some(h.clone()),
                (None, 1) => headers.pop(),
                _ => None,
            }
        })
}

/// `includes` (comma-separated) with `header` added once.
fn with_include(includes: &str, header: &str) -> String {
    let mut all = list(includes);
    if !all.iter().any(|h| h == header) {
        all.push(header.to_owned());
    }
    all.join(", ")
}

/// `includes` (comma-separated) without `header`.
fn without_include(includes: &str, header: &str) -> String {
    let all: Vec<String> = list(includes).into_iter().filter(|h| h != header).collect();
    all.join(", ")
}

fn parse_bool(t: &str) -> Option<bool> {
    match t {
        "true" | "1" | "да" => Some(true),
        "false" | "0" | "нет" => Some(false),
        _ => None,
    }
}

fn parse_float(t: &str) -> Option<f64> {
    t.replace(',', ".").parse().ok()
}

/// Default of a port; empty means "no default".
fn parse_signal(ty: SignalType, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let value = match ty {
        SignalType::Bool => parse_bool(t).map(ParamValue::Bool),
        SignalType::Int => t.parse().ok().map(ParamValue::Int),
        SignalType::Float => parse_float(t).map(ParamValue::Float),
        SignalType::String => Some(ParamValue::Text(t.to_owned())),
        SignalType::Any => Some(SignalType::guess_value(t)),
        SignalType::Number => {
            Some(SignalType::guess_value(t)).filter(|v| SignalType::of_value(v).is_numeric())
        }
    };
    value
        .map(Some)
        .ok_or_else(|| format!("«{t}» не подходит к типу {}", type_name(ty)))
}

/// Default of a parameter; empty means "no default" (the parameter is required).
fn parse_param(kind: ParamKind, text: &str) -> Result<Option<ParamValue>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let value = match kind {
        ParamKind::Bool => parse_bool(t).map(ParamValue::Bool),
        ParamKind::Int | ParamKind::Pin => t.parse().ok().map(ParamValue::Int),
        ParamKind::Float => parse_float(t).map(ParamValue::Float),
        // Durations are checked by the definition itself.
        ParamKind::Text | ParamKind::Duration => Some(ParamValue::Text(t.to_owned())),
    };
    value
        .map(Some)
        .ok_or_else(|| format!("«{t}» не подходит к виду «{}»", kind_label(kind)))
}

fn parse_number(text: &str) -> Result<Option<f64>, String> {
    let t = text.trim();
    if t.is_empty() {
        return Ok(None);
    }
    parse_float(t)
        .map(Some)
        .ok_or_else(|| format!("граница «{t}» — не число"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Ports,
    Params,
    Code,
    Docs,
    Files,
    Sketch,
}

/// Trial generation for one state of the form.
struct Generated {
    def: BlockDef,
    template: String,
    result: Result<String, String>,
}

pub struct BlockEditor {
    /// Type the form was loaded from; `None` for a block not saved yet.
    original: Option<String>,
    form: Form,
    /// The form as last loaded or saved.
    clean: Form,
    tab: Tab,
    generated: Option<Generated>,
    /// Closing with unsaved changes: asking what to do.
    confirm_close: bool,
    /// A built-in block: shown, but cannot be changed.
    read_only: bool,
    /// Sections, packs and folders a new block may be put into.
    folders: Vec<String>,
    /// Names of inputs, outputs and parameters as of the last frame, to follow renames.
    names: Vec<(&'static str, String)>,
}

impl BlockEditor {
    /// A new block in a section or folder of the user library (`""` — the root).
    pub fn new_block(app: &EditorApp, group: &str) -> Self {
        let mut form = Form::new_block(String::new(), group.to_owned());
        form.kind = free_kind(app, &ident_from_name(&form.name));
        Self {
            folders: user_folders(app),
            ..Self::with(None, form)
        }
    }

    /// Opens a library block: a user block for editing, a built-in one read-only.
    pub fn edit(app: &EditorApp, block: &LibraryBlock) -> Self {
        let kind = block.def.info.kind.clone();
        let read_only = !app.lib.is_user(&kind);
        Self {
            read_only,
            ..Self::with(Some(kind), Form::from_block(block))
        }
    }

    /// A new block based on an existing one.
    /// A copy of a user block lands next to it, of a built-in one in the root.
    pub fn copy_of(app: &EditorApp, block: &LibraryBlock) -> Self {
        let mut form = Form::from_block(block);
        if !app.lib.is_user(&block.def.info.kind) {
            form.group.clear();
        }
        form.attach = Attach::copied_from(&block.dir);
        form.name = format!("{} (копия)", form.name);
        form.kind = free_kind(app, &ident_from_name(&form.name));
        Self {
            folders: user_folders(app),
            ..Self::with(None, form)
        }
    }

    fn with(original: Option<String>, form: Form) -> Self {
        Self {
            original,
            clean: form.clone(),
            form,
            tab: Tab::Ports,
            generated: None,
            confirm_close: false,
            read_only: false,
            folders: Vec::new(),
            names: Vec::new(),
        }
    }

    /// When a port or parameter is renamed, its references in the template follow,
    /// so the code tab does not fail on the old name while the new one is typed.
    fn follow_renames(&mut self) {
        let now = self.form.row_names();
        if self.names.len() != now.len() {
            self.names = now;
            return;
        }
        for (old, new) in self.names.iter_mut().zip(now) {
            // A field cleared on the way to a new name keeps the old name until it is typed.
            if new.1.is_empty() || old.0 != new.0 || old.1 == new.1 {
                continue;
            }
            self.form.template = rename_in_template(&self.form.template, old.0, &old.1, &new.1);
            old.1 = new.1;
        }
    }

    pub fn dirty(&self) -> bool {
        !self.read_only && self.form != self.clean
    }

    /// Asks "save changes?" on the next frame, as when its window is closed.
    pub fn ask_to_close(&mut self) {
        self.confirm_close = true;
    }

    /// Regenerates the trial sketch when the definition or template changed.
    fn update_generated(&mut self, def: &BlockDef) {
        let fresh = self
            .generated
            .as_ref()
            .is_some_and(|g| g.def == *def && g.template == self.form.template);
        if !fresh {
            let result =
                flp_codegen::preview_block(def, &self.form.template).map_err(|e| e.to_string());
            self.generated = Some(Generated {
                def: def.clone(),
                template: self.form.template.clone(),
                result,
            });
        }
    }
}

/// A type not used by any block, made from `base`.
fn free_kind(app: &EditorApp, base: &str) -> String {
    let taken = |k: &str| app.lib.get(k).is_some() || k == VAR_GET || k == VAR_SET;
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|k| !taken(k))
        .expect("some name is free")
}

pub fn show(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(mut ed) = app.block_editor.take() else {
        return;
    };
    let title = match &ed.original {
        Some(kind) if ed.read_only => format!("Встроенный блок — {kind} (только просмотр)"),
        Some(kind) => format!("Редактор блока — {kind}"),
        None => "Редактор блока — новый блок".to_owned(),
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new(title)
        .id(Id::unique("block_editor"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(900.0)
        .default_height(640.0)
        .show(ctx, |ui| close = body(app, ui, &mut ed));
    if !open && ed.dirty() {
        ed.confirm_close = true;
    } else if !open {
        close = true;
    }
    if ed.confirm_close && !close {
        close = confirm_close(app, ctx, &mut ed);
    }
    if !close {
        app.block_editor = Some(ed);
    }
}

/// Modal question on closing with unsaved changes; returns true when the window
/// should close (saved or changes dropped).
fn confirm_close(app: &mut EditorApp, ctx: &egui::Context, ed: &mut BlockEditor) -> bool {
    let def = ed.form.to_def();
    let name = ed.form.name.trim().to_owned();
    let error = app.theme.palette.error;
    let mut answer = None;
    egui::Modal::new(Id::unique("block_editor_close")).show(ctx, |ui| {
        ui.label(RichText::new("Сохранить изменения?").strong());
        ui.label(format!("В блоке «{name}» есть несохранённые изменения."));
        if let Err(e) = &def {
            ui.colored_label(error, format!("Сохранить нельзя: {e}"));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(def.is_ok(), egui::Button::new("Сохранить"))
                .clicked()
            {
                answer = Some(1);
            }
            if ui.button("Выйти без сохранения").clicked() {
                answer = Some(2);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(0);
            }
        });
    });
    match answer {
        Some(1) => {
            let saved = save(app, ed, &def);
            ed.confirm_close = saved;
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

/// Window contents; returns true when the window should close.
fn body(app: &mut EditorApp, ui: &mut Ui, ed: &mut BlockEditor) -> bool {
    let pal = app.theme.palette;
    // Until the first save the type follows the name.
    if ed.original.is_none() {
        let kind = free_kind(app, &ident_from_name(&ed.form.name));
        ed.clean.kind.clone_from(&kind);
        ed.form.kind = kind;
    }
    if !ed.read_only {
        ed.follow_renames();
    }
    let def = ed.form.to_def();
    if let Ok(def) = &def {
        ed.update_generated(def);
    }

    // Panels inside the window: the footer stays visible and the tab gets the rest.
    let mut close = false;
    let mut help = None;
    egui::Panel::bottom("block_editor_footer").show(ui, |ui| {
        status(ui, ed, &def, &pal);
        close = footer(app, ui, ed, &def);
    });
    egui::Panel::top("block_editor_header").show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Вид на плате").strong());
                match &def {
                    Ok(def) => crate::canvas::block_preview(ui, def, &pal),
                    Err(_) => {
                        ui.weak("(исправьте ошибки описания)");
                    }
                }
            });
            ui.add_space(16.0);
            let editable = !ed.read_only;
            ui.vertical(|ui| ui.add_enabled_ui(editable, |ui| header(ui, ed)));
        });
        ui.separator();
        ui.horizontal(|ui| {
            let tabs = [
                (Tab::Ports, "Входы и выходы"),
                (Tab::Params, "Параметры"),
                (Tab::Code, "Код ESP32"),
                (Tab::Docs, "Описание"),
                (Tab::Files, "Библиотека и пример"),
                (Tab::Sketch, "Пробная генерация"),
            ];
            for (tab, text) in tabs {
                ui.selectable_value(&mut ed.tab, tab, text);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("? Справка")
                    .on_hover_text("Справка по редактору блока: открывается на текущей вкладке")
                    .clicked()
                {
                    let anchor = match ed.tab {
                        Tab::Ports => "входы-и-выходы",
                        Tab::Params => "параметры",
                        Tab::Code => "код-esp32",
                        Tab::Docs => "описание",
                        Tab::Files => "библиотека-и-пример",
                        Tab::Sketch => "пробная-генерация",
                    };
                    help = Some(format!("{}#{anchor}", crate::help::BLOCK_EDITOR));
                }
            });
        });
    });
    let editable = !ed.read_only;
    egui::CentralPanel::default().show(ui, |ui| {
        ScrollArea::both()
            .id_salt("block_editor_tab")
            .auto_shrink([false, false])
            .show(ui, |ui| match ed.tab {
                Tab::Ports => {
                    ui.add_enabled_ui(editable, |ui| ports_tab(ui, &mut ed.form));
                }
                Tab::Params => {
                    ui.add_enabled_ui(editable, |ui| params_tab(ui, &mut ed.form));
                }
                Tab::Code => code_tab(ui, &mut ed.form, editable),
                Tab::Docs => docs_tab(ui, &mut ed.form, editable),
                Tab::Files => {
                    ui.add_enabled_ui(editable, |ui| files_tab(ui, &mut ed.form));
                }
                Tab::Sketch => sketch_tab(ui, ed, &pal),
            });
    });
    if let Some(target) = help {
        app.open_help(&target);
    }
    close
}

fn user_folders(app: &EditorApp) -> Vec<String> {
    app.lib.user_folders().map(str::to_owned).collect()
}

fn header(ui: &mut Ui, ed: &mut BlockEditor) {
    let read_only = ed.read_only;
    let f = &mut ed.form;
    egui::Grid::new("block_header")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label("Название");
            ui.add_sized([220.0, 20.0], TextEdit::singleline(&mut f.name))
                .on_hover_text(format!(
                    "Тип блока в схеме: {}. У нового блока он берётся из названия, \
                     после сохранения не меняется.",
                    f.kind
                ));
            ui.label("Обозначение");
            ui.add_sized(
                [160.0, 20.0],
                TextEdit::singleline(&mut f.symbol).hint_text("как название"),
            )
            .on_hover_text("Короткая надпись на блоке: TON, a + b, &");
            ui.end_row();

            ui.label("Где лежит");
            let place = if read_only {
                format!("Встроенные / {}", f.group)
            } else if f.group.is_empty() {
                "Блоки пользователей".to_owned()
            } else {
                format!("Блоки пользователей / {}", f.group.replace('/', " / "))
            };
            if ed.original.is_none() && !read_only && !ed.folders.is_empty() {
                // A new block: choose the pack (section or folder) it goes into.
                egui::ComboBox::from_id_salt("block_place")
                    .selected_text(place)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut f.group, String::new(), "Блоки пользователей");
                        for folder in &ed.folders {
                            ui.selectable_value(
                                &mut f.group,
                                folder.clone(),
                                folder.replace('/', " / "),
                            );
                        }
                    })
                    .response
                    .on_hover_text("Набор, раздел или папка, куда сохранить новый блок");
            } else {
                ui.label(place).on_hover_text(
                    "Раздел и папка в библиотеке. Перенести блок — правая кнопка на нём \
                     во вкладке «Пользовательские».",
                );
            }
            ui.end_row();

            ui.label("Описание");
            ui.add_sized([160.0, 20.0], TextEdit::singleline(&mut f.description));
            ui.label("");
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.memory, "Хранит состояние")
                    .on_hover_text(
                        "Блок помнит значения между циклами (таймеры, триггеры, счётчики); \
                         через такой блок допустимы обратные связи",
                    );
                ui.checkbox(&mut f.single, "Один на проект")
                    .on_hover_text("В проекте может быть только один такой блок (Wi-Fi, NTP)");
            });
            ui.end_row();

            ui.label("Библиотеки");
            ui.add_sized(
                [220.0, 20.0],
                TextEdit::singleline(&mut f.libraries).hint_text("DHT sensor library"),
            )
            .on_hover_text(
                "Библиотеки arduino-cli через запятую, можно с версией: OneWire@2.3.8. \
                 Сборка ставит недостающие сама.",
            );
            ui.end_row();
        });
}

/// Move, delete buttons of a table row; returns the action chosen.
enum RowOp {
    Up(usize),
    Down(usize),
    Remove(usize),
}

fn row_ops(ui: &mut Ui, i: usize, len: usize, op: &mut Option<RowOp>) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(i > 0, egui::Button::new("↑").small())
            .clicked()
        {
            *op = Some(RowOp::Up(i));
        }
        if ui
            .add_enabled(i + 1 < len, egui::Button::new("↓").small())
            .clicked()
        {
            *op = Some(RowOp::Down(i));
        }
        if ui.small_button("✕").on_hover_text("Удалить").clicked() {
            *op = Some(RowOp::Remove(i));
        }
    });
}

fn apply_op<T>(rows: &mut Vec<T>, op: Option<RowOp>) {
    match op {
        Some(RowOp::Up(i)) => rows.swap(i, i - 1),
        Some(RowOp::Down(i)) => rows.swap(i, i + 1),
        Some(RowOp::Remove(i)) => {
            rows.remove(i);
        }
        None => {}
    }
}

fn type_combo(ui: &mut Ui, id: (&str, usize), ty: &mut SignalType) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(type_name(*ty))
        .width(70.0)
        .show_ui(ui, |ui| {
            // `any` fits inputs only; the block check says so for an output.
            for t in SignalType::ALL
                .into_iter()
                .chain([SignalType::Any, SignalType::Number])
            {
                ui.selectable_value(ty, t, type_name(t));
            }
        });
}

/// Choice of the value of an unconnected `bool` input: `true`, `false` or required (empty).
fn bool_default_combo(ui: &mut Ui, id: (&str, usize), default: &mut String) {
    // Text left from another type or typed by hand shows as the nearest choice.
    *default = match parse_bool(default.trim()) {
        Some(true) => "true",
        Some(false) => "false",
        None => "",
    }
    .to_owned();
    let shown = if default.is_empty() {
        "обязателен"
    } else {
        default.as_str()
    };
    egui::ComboBox::from_id_salt(("bool_default", id))
        .selected_text(shown)
        .width(90.0)
        .show_ui(ui, |ui| {
            for (value, text) in [("", "обязателен"), ("true", "true"), ("false", "false")]
            {
                ui.selectable_value(default, value.to_owned(), text);
            }
        })
        .response
        .on_hover_text("Значение неподключённого входа; «обязателен» — вход надо подключить");
}

/// A name not used by any port or parameter of the form.
fn free_name(f: &Form, base: &str) -> String {
    let taken = |n: &str| {
        f.inputs.iter().chain(&f.outputs).any(|p| p.name == n)
            || f.params.iter().any(|p| p.name == n)
    };
    (1..)
        .map(|i| format!("{base}{i}"))
        .find(|n| !taken(n))
        .expect("some name is free")
}

fn ports_table(ui: &mut Ui, id: &str, rows: &mut Vec<PortRow>, inputs: bool) {
    if rows.is_empty() {
        ui.weak(if inputs {
            "Входов нет"
        } else {
            "Выходов нет"
        });
        return;
    }
    let mut op = None;
    let len = rows.len();
    egui::Grid::new(id)
        .num_columns(6)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Имя").strong());
            ui.label(RichText::new("Тип").strong());
            ui.label(
                RichText::new(if inputs {
                    "Если не подключён"
                } else {
                    ""
                })
                .strong(),
            );
            ui.label(RichText::new("Условие when").strong());
            ui.label(RichText::new("Описание").strong());
            ui.label("");
            ui.end_row();
            for (i, row) in rows.iter_mut().enumerate() {
                ui.add_sized([90.0, 20.0], TextEdit::singleline(&mut row.name));
                type_combo(ui, (id, i), &mut row.ty);
                if inputs && row.ty == SignalType::Bool {
                    bool_default_combo(ui, (id, i), &mut row.default);
                } else if inputs {
                    ui.add_sized(
                        [90.0, 20.0],
                        TextEdit::singleline(&mut row.default).hint_text("обязателен"),
                    )
                    .on_hover_text("Значение неподключённого входа; пусто — вход надо подключить");
                } else {
                    ui.label("");
                }
                ui.add_sized(
                    [130.0, 20.0],
                    TextEdit::singleline(&mut row.when).hint_text("всегда"),
                )
                .on_hover_text(
                    "Выражение по параметрам, например mode == \"input\": \
                     порт есть, только пока оно выполняется",
                );
                ui.add_sized([220.0, 20.0], TextEdit::singleline(&mut row.description));
                row_ops(ui, i, len, &mut op);
                ui.end_row();
            }
        });
    apply_op(rows, op);
}

fn ports_tab(ui: &mut Ui, f: &mut Form) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Входы").strong());
        if ui.small_button("+ Добавить вход").clicked() {
            let name = free_name(f, "in");
            f.inputs.push(PortRow::new(&name));
        }
    });
    ports_table(ui, "block_inputs", &mut f.inputs, true);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Выходы").strong());
        if ui.small_button("+ Добавить выход").clicked() {
            let name = free_name(f, "out");
            f.outputs.push(PortRow::new(&name));
        }
    });
    ports_table(ui, "block_outputs", &mut f.outputs, false);
    ui.add_space(8.0);
    ui.weak(
        "Имена — буквы (можно русские), цифры и «_», без пробелов: по ним блок связан в схеме \
         и в коде; Входы, выходы и параметры не могут называться \
         одинаково, имена id и type заняты. Неявно приводится только int → float.",
    );
}

fn params_tab(ui: &mut Ui, f: &mut Form) {
    if ui.small_button("+ Добавить параметр").clicked() {
        let name = free_name(f, "p");
        f.params.push(ParamRow::new(&name));
    }
    if f.params.is_empty() {
        ui.weak("Параметров нет");
        return;
    }
    let mut op = None;
    let len = f.params.len();
    egui::Grid::new("block_params")
        .num_columns(9)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for h in [
                "Имя",
                "Вид",
                "По умолчанию",
                "Мин.",
                "Макс.",
                "Условие when",
                "Под блоком",
                "Описание",
            ] {
                ui.label(RichText::new(h).strong());
            }
            ui.label("");
            ui.end_row();
            for (i, row) in f.params.iter_mut().enumerate() {
                ui.add_sized([90.0, 20.0], TextEdit::singleline(&mut row.name));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("param_kind", i))
                        .selected_text(kind_label(row.kind))
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for k in PARAM_KINDS {
                                ui.selectable_value(&mut row.kind, k, kind_label(k));
                            }
                        });
                    if row.kind == ParamKind::Pin {
                        pin_mode_edit(ui, i, row);
                    }
                });
                let hint = match row.kind {
                    ParamKind::Duration => "обязателен (1s, 500ms)",
                    _ => "обязателен",
                };
                ui.add_sized(
                    [110.0, 20.0],
                    TextEdit::singleline(&mut row.default).hint_text(hint),
                )
                .on_hover_text("Пусто — параметр надо задать в свойствах блока");
                let numeric = matches!(
                    row.kind,
                    ParamKind::Int | ParamKind::Float | ParamKind::Pin | ParamKind::Duration
                );
                for bound in [&mut row.min, &mut row.max] {
                    ui.add_enabled_ui(numeric, |ui| {
                        ui.add_sized([50.0, 20.0], TextEdit::singleline(bound))
                    })
                    .inner
                    .on_hover_text("У duration — в миллисекундах");
                }
                ui.add_sized(
                    [130.0, 20.0],
                    TextEdit::singleline(&mut row.when).hint_text("всегда"),
                );
                egui::ComboBox::from_id_salt(("param_show", i))
                    .selected_text(show_label(row.show))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for s in ParamShow::ALL {
                            ui.selectable_value(&mut row.show, s, show_label(s));
                        }
                    })
                    .response
                    .on_hover_text("Как значение подписывается под блоком на плате");
                ui.add_sized([200.0, 20.0], TextEdit::singleline(&mut row.description));
                row_ops(ui, i, len, &mut op);
                ui.end_row();
            }
        });
    apply_op(&mut f.params, op);
}

/// Direction of a pin parameter and whether blocks of the type may share the pin.
fn pin_mode_edit(ui: &mut Ui, i: usize, row: &mut ParamRow) {
    let label = |m: PinMode| match m {
        PinMode::Io => "вход-выход",
        PinMode::In => "вход",
        PinMode::Out => "выход",
        PinMode::Adc => "АЦП",
    };
    egui::ComboBox::from_id_salt(("pin_mode", i))
        .selected_text(label(row.mode))
        .width(90.0)
        .show_ui(ui, |ui| {
            for m in [PinMode::Io, PinMode::In, PinMode::Out, PinMode::Adc] {
                ui.selectable_value(&mut row.mode, m, label(m));
            }
        })
        .response
        .on_hover_text("Какие выводы платы подходят: выход не ставится на вывод «только вход», АЦП — только на выводы с АЦП");
    ui.checkbox(&mut row.shared, "общий")
        .on_hover_text("Несколько блоков этого типа могут сидеть на одном выводе (шина 1-Wire)");
}

/// Text inserted into the template from the substitution menu or buttons.
struct Snippet {
    label: String,
    text: String,
    hint: String,
}

impl Snippet {
    fn new(label: impl Into<String>, text: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            hint: hint.into(),
        }
    }
}

/// Name with the caption, for menus: `in (Вход)`.
fn row_title(name: &str, label: &str) -> String {
    match label.trim() {
        "" => name.to_owned(),
        l => format!("{name} ({l})"),
    }
}

/// Replaces `group.old` and `group["old"]` in a template by the new name.
fn rename_in_template(template: &str, group: &str, old: &str, new: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let dotted = format!("{group}.{old}");
    while let Some(at) = rest.find(group) {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let before_ok = !head.chars().next_back().is_some_and(is_word);
        if before_ok && tail.starts_with(&dotted) && !tail[dotted.len()..].starts_with(is_word) {
            out.push_str(&format!("{group}.{new}"));
            rest = &tail[dotted.len()..];
            continue;
        }
        let mut done = false;
        for q in ['"', '\''] {
            let quoted = format!("{group}[{q}{old}{q}]");
            if before_ok && tail.starts_with(&quoted) {
                out.push_str(&format!("{group}[{q}{new}{q}]"));
                rest = &tail[quoted.len()..];
                done = true;
                break;
            }
        }
        if !done {
            out.push_str(group);
            rest = &tail[group.len()..];
        }
    }
    out.push_str(rest);
    out
}

/// Port substitutions: `{{ input.x }}` or `{{ output.x }}`.
fn port_snippets(rows: &[PortRow], kind: &str) -> Vec<Snippet> {
    rows.iter()
        .map(|p| {
            let hint = if p.description.is_empty() {
                type_name(p.ty).to_owned()
            } else {
                format!("{}, {}", type_name(p.ty), p.description)
            };
            Snippet::new(
                row_title(&p.name, &p.label),
                format!("{{{{ {kind}.{} }}}}", p.name),
                hint,
            )
        })
        .collect()
}

/// Submenu of snippets; the chosen text goes to `insert`.
fn snippet_menu(ui: &mut Ui, title: &str, items: &[Snippet], insert: &mut Option<String>) {
    ui.add_enabled_ui(!items.is_empty(), |ui| {
        ui.menu_button(title, |ui| {
            for s in items {
                let text = RichText::new(format!("{}   {}", s.label, s.text.trim_end()));
                if ui.button(text).on_hover_text(&s.hint).clicked() {
                    *insert = Some(s.text.clone());
                    ui.close();
                }
            }
        });
    });
}

/// Inserts `text` at the cursor of the text edit `id` (replacing the selection,
/// or at the end without a cursor) and puts the cursor after it.
fn insert_at_cursor(ctx: &egui::Context, id: Id, template: &mut String, text: &str) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let chars = template.chars().count();
    let (start, end) = state
        .cursor
        .char_range()
        .map(|r| {
            let (a, b) = (r.primary.index.0, r.secondary.index.0);
            (a.min(b).min(chars), a.max(b).min(chars))
        })
        .unwrap_or((chars, chars));
    let byte = |i: usize| {
        template
            .char_indices()
            .nth(i)
            .map_or(template.len(), |(b, _)| b)
    };
    let (from, to) = (byte(start), byte(end));
    template.replace_range(from..to, text);
    let cursor = CCursor::new(start + text.chars().count());
    state.cursor.set_char_range(Some(CCursorRange::one(cursor)));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// A Markdown template of the docs toolbar.
#[derive(Debug, Clone, PartialEq)]
enum Fmt {
    Bold,
    Italic,
    Code,
    Heading(usize),
    Bullets,
    Numbered,
    Quote,
    CodeBlock,
    Table,
    Link,
    /// Picture `![описание](target)` on a line of its own.
    Image(String),
}

/// Applies `fmt` to the chars `start..end` of `text`; returns the new text and the new selection.
fn format_selection(text: &str, start: usize, end: usize, fmt: &Fmt) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = (
        start.min(end).min(chars.len()),
        start.max(end).min(chars.len()),
    );
    let sel: String = chars[start..end].iter().collect();
    let before: String = chars[..start].iter().collect();
    let after: String = chars[end..].iter().collect();
    let len = |s: &str| s.chars().count();
    // Text put in place of the selection, and the selection inside it.
    let (insert, from, to) = match fmt {
        Fmt::Bold | Fmt::Italic | Fmt::Code => {
            let mark = match fmt {
                Fmt::Bold => "**",
                Fmt::Italic => "*",
                _ => "`",
            };
            let inner = if sel.is_empty() { "текст" } else { &sel };
            let m = len(mark);
            (format!("{mark}{inner}{mark}"), m, m + len(inner))
        }
        Fmt::Link => {
            let url = "https://";
            if sel.is_empty() {
                (format!("[текст]({url})"), 1, 1 + len("текст"))
            } else {
                let at = len(&sel) + 3;
                (format!("[{sel}]({url})"), at, at + len(url))
            }
        }
        Fmt::Heading(_) | Fmt::Bullets | Fmt::Numbered | Fmt::Quote => {
            return prefix_lines(&chars, start, end, fmt);
        }
        Fmt::CodeBlock | Fmt::Table | Fmt::Image(_) => {
            // A block stands on lines of its own.
            let lead = if before.is_empty() || before.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            let tail = if after.starts_with('\n') { "" } else { "\n" };
            let inner = if sel.is_empty() { "код" } else { &sel };
            let (body, from, to) = match fmt {
                Fmt::CodeBlock => (format!("```\n{inner}\n```"), 4, 4 + len(inner)),
                Fmt::Image(target) => (format!("![описание]({target})"), 2, 2 + len("описание")),
                _ => {
                    let table = "| Колонка 1 | Колонка 2 |\n|---|---|\n| значение | значение |";
                    (table.to_owned(), len(table), len(table))
                }
            };
            let lead_len = len(lead);
            (
                format!("{lead}{body}{tail}"),
                lead_len + from,
                lead_len + to,
            )
        }
    };
    (format!("{before}{insert}{after}"), start + from, start + to)
}

/// Adds a line prefix to the lines the selection touches, or removes it when every line has it.
fn prefix_lines(chars: &[char], start: usize, end: usize, fmt: &Fmt) -> (String, usize, usize) {
    let line_start = chars[..start]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |i| i + 1);
    // A selection ending right after a line break does not touch the next line.
    let last = if end > start && chars[end - 1] == '\n' {
        end - 1
    } else {
        end
    };
    let line_end = chars[last..]
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |i| last + i);
    let block: String = chars[line_start..line_end].iter().collect();
    let lines: Vec<&str> = block.split('\n').collect();

    let prefix = |n: usize| match fmt {
        Fmt::Heading(level) => format!("{} ", "#".repeat(*level)),
        Fmt::Bullets => "- ".to_owned(),
        Fmt::Numbered => format!("{}. ", n + 1),
        _ => "> ".to_owned(),
    };
    // Bytes of the prefix of this kind that the line already has.
    let existing = |line: &str| -> usize {
        match fmt {
            Fmt::Heading(_) => {
                let hashes = line.bytes().take_while(|&b| b == b'#').count();
                if hashes > 0 && line[hashes..].starts_with(' ') {
                    hashes + 1
                } else {
                    0
                }
            }
            Fmt::Bullets if line.starts_with("- ") => 2,
            Fmt::Quote if line.starts_with("> ") => 2,
            Fmt::Numbered => {
                let digits = line.bytes().take_while(u8::is_ascii_digit).count();
                if digits > 0 && line[digits..].starts_with(". ") {
                    digits + 2
                } else {
                    0
                }
            }
            _ => 0,
        }
    };
    let remove = match fmt {
        // The same heading level again removes it; another level replaces it.
        Fmt::Heading(level) => lines.iter().all(|l| existing(l) == level + 1),
        _ => lines.iter().all(|l| existing(l) > 0),
    };
    let out: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(n, line)| {
            let rest = &line[existing(line)..];
            if remove {
                rest.to_owned()
            } else {
                format!("{}{rest}", prefix(n))
            }
        })
        .collect();
    let out = out.join("\n");
    let before: String = chars[..line_start].iter().collect();
    let after: String = chars[line_end..].iter().collect();
    let to = line_start + out.chars().count();
    (format!("{before}{out}{after}"), line_start, to)
}

/// Applies `fmt` to the selection of the text field `id` and keeps the result selected.
fn format_at_cursor(ctx: &egui::Context, id: Id, text: &mut String, fmt: &Fmt) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let chars = text.chars().count();
    let (start, end) = state
        .cursor
        .char_range()
        .map_or((chars, chars), |r| (r.primary.index.0, r.secondary.index.0));
    let (new, from, to) = format_selection(text, start, end, fmt);
    *text = new;
    let range = CCursorRange::two(CCursor::new(from), CCursor::new(to));
    state.cursor.set_char_range(Some(range));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Tab «Описание»: formatting buttons, the Markdown text and a live preview.
fn docs_tab(ui: &mut Ui, f: &mut Form, editable: bool) {
    let id = Id::unique("block_docs_text");
    ui.weak("docs.md: пояснение и пример. По нему ИИ-помощник подбирает блоки.");
    if editable {
        let mut apply = None;
        ui.horizontal_wrapped(|ui| {
            let buttons = [
                ("Ж", "Жирный: **текст**", Fmt::Bold),
                ("К", "Курсив: *текст*", Fmt::Italic),
                ("`код`", "Код в строке", Fmt::Code),
                ("H1", "Заголовок первого уровня", Fmt::Heading(1)),
                ("H2", "Заголовок второго уровня", Fmt::Heading(2)),
                ("H3", "Заголовок третьего уровня", Fmt::Heading(3)),
                (
                    "• Список",
                    "Маркированный список; повторное нажатие снимает",
                    Fmt::Bullets,
                ),
                (
                    "1. Список",
                    "Нумерованный список; повторное нажатие снимает",
                    Fmt::Numbered,
                ),
                ("Цитата", "Цитата: > текст", Fmt::Quote),
                ("Блок кода", "Блок кода в ```", Fmt::CodeBlock),
                ("Таблица", "Таблица 2×2", Fmt::Table),
                ("Ссылка", "[текст](адрес)", Fmt::Link),
            ];
            for (text, hint, fmt) in buttons {
                if ui.button(text).on_hover_text(hint).clicked() {
                    apply = Some(fmt);
                }
            }
            let hint = "Вставить картинку из файла (png, jpg, gif, bmp). \
                        При сохранении она копируется в папку блока";
            if ui.button("Картинка...").on_hover_text(hint).clicked() {
                let file = rfd::FileDialog::new()
                    .set_title("Картинка для описания")
                    .add_filter("Изображения", &["png", "jpg", "jpeg", "gif", "bmp"])
                    .pick_file();
                if let Some(file) = file {
                    apply = Some(Fmt::Image(f.attach.add_image(file)));
                }
            }
        });
        if let Some(fmt) = apply {
            format_at_cursor(ui.ctx(), id, &mut f.docs, &fmt);
        }
    }
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        cols[0].add(
            TextEdit::multiline(&mut f.docs)
                .id(id)
                .interactive(editable)
                .desired_width(f32::INFINITY)
                .desired_rows(20),
        );
        cols[1].label(RichText::new("Предпросмотр").weak());
        let attach = &f.attach;
        let find = |target: &str| attach.image_path(target);
        crate::markdown::show_with_images(&mut cols[1], &f.docs, None, Some(&find));
    });
}

fn code_tab(ui: &mut Ui, f: &mut Form, editable: bool) {
    ui.horizontal(|ui| {
        ui.label("Заголовки");
        ui.add_enabled(
            editable,
            TextEdit::singleline(&mut f.includes)
                .hint_text("DHT.h, Wire.h")
                .desired_width(260.0),
        )
        .on_hover_text("Файлы для #include через запятую");
    });
    ui.weak(SECTIONS_HINT);
    let inputs = port_snippets(&f.inputs, "input");
    let outputs = port_snippets(&f.outputs, "output");
    let literal: Vec<Snippet> = f
        .params
        .iter()
        .map(|p| {
            let hint = format!("{} — значение как литерал C++", kind_label(p.kind));
            Snippet::new(
                row_title(&p.name, &p.label),
                format!("{{{{ param.{} }}}}", p.name),
                hint,
            )
        })
        .collect();
    let conditions: Vec<Snippet> = f
        .params
        .iter()
        .map(|p| {
            let hint = "Код внутри попадёт в скетч, только если условие выполнено";
            let text = match p.kind {
                ParamKind::Bool => format!(
                    "{{% if value.{} %}}

{{% endif %}}
",
                    p.name
                ),
                ParamKind::Text => {
                    format!(
                        "{{% if value.{} == \"\" %}}

{{% endif %}}
",
                        p.name
                    )
                }
                _ => format!(
                    "{{% if value.{} > 0 %}}

{{% endif %}}
",
                    p.name
                ),
            };
            Snippet::new(row_title(&p.name, &p.label), text, hint)
        })
        .collect();
    let sections: Vec<Snippet> = [
        ("// @global", "Переменные блока, по одной на экземпляр"),
        ("// @setup", "Код в setup()"),
        ("// @loop", "Код цикла, в порядке схемы"),
        ("// @functions", "Функции блока, перед setup()"),
        ("// @global once", "Общие переменные, один раз на проект"),
        ("// @setup once", "Общий код в setup(), один раз"),
        ("// @functions once", "Общие функции, один раз на проект"),
    ]
    .into_iter()
    .map(|(text, hint)| {
        Snippet::new(
            "",
            format!(
                "{text}
"
            ),
            hint,
        )
    })
    .collect();

    if !editable {
        let mut text = f.template.as_str();
        let mut layouter = crate::cpphl::layouter(true);
        ui.add(
            TextEdit::multiline(&mut text)
                .code_editor()
                .layouter(&mut layouter)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
        return;
    }
    let id = Id::unique("block_template");
    let mut insert = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Вставить:");
        let buttons = inputs.iter().chain(&outputs).chain(&literal);
        for s in std::iter::once(&Snippet::new("", "{{ id }}", "")).chain(buttons) {
            if ui
                .small_button(RichText::new(&s.text).monospace())
                .clicked()
            {
                insert = Some(s.text.clone());
            }
        }
    });
    ui.weak("Правая кнопка в поле кода — меню подстановок; вставка идёт в место курсора.");
    let mut layouter = crate::cpphl::layouter(true);
    let output = TextEdit::multiline(&mut f.template)
        .id(id)
        .code_editor()
        .layouter(&mut layouter)
        .desired_width(f32::INFINITY)
        .desired_rows(24)
        .show(ui);
    output.response.context_menu(|ui| {
        if ui
            .button("{{ id }} — префикс переменных блока")
            .on_hover_text("Имена в разделах без once начинайте с него: {{ id }}_start")
            .clicked()
        {
            insert = Some("{{ id }}".into());
            ui.close();
        }
        ui.separator();
        snippet_menu(ui, "Вход", &inputs, &mut insert);
        snippet_menu(ui, "Выход", &outputs, &mut insert);
        snippet_menu(ui, "Параметр", &literal, &mut insert);
        snippet_menu(ui, "Условие по параметру", &conditions, &mut insert);
        ui.separator();
        snippet_menu(ui, "Раздел", &sections, &mut insert);
    });
    if let Some(text) = insert {
        insert_at_cursor(ui.ctx(), id, &mut f.template, &text);
    }
}

fn files_tab(ui: &mut Ui, f: &mut Form) {
    let attach = &mut f.attach;
    ui.weak(
        "Библиотека Arduino и пример лежат в папке блока и едут вместе с ним \
         в файле .flpblock и в проекте. Сборка подхватывает библиотеки сама, \
         а главный заголовок прикреплённой библиотеки попадает в поле «Заголовки» \
         на вкладке «Код ESP32».",
    );
    ui.add_space(6.0);
    ui.label(RichText::new("Библиотеки").strong());
    let mut remove = None;
    for (i, lib) in attach.libs.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(&lib.name);
            if lib.src.is_some() {
                ui.weak("(будет скопирована при сохранении)");
            }
            if ui.small_button("Убрать").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        let lib = attach.libs.remove(i);
        if let Some(header) = lib.header {
            f.includes = without_include(&f.includes, &header);
        }
    }
    if attach.libs.is_empty() {
        ui.weak("Нет прикреплённых библиотек");
    }
    if ui
        .button("Прикрепить библиотеку...")
        .on_hover_text("Папка библиотеки Arduino (с library.properties или src/)")
        .clicked()
    {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("Папка библиотеки Arduino")
            .pick_folder()
        {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !name.is_empty() && !attach.libs.iter().any(|l| l.name == name) {
                let header = library_header(&dir, &name);
                if let Some(header) = &header {
                    f.includes = with_include(&f.includes, header);
                }
                attach.libs.push(AttachedLib {
                    name,
                    src: Some(dir),
                    header,
                });
            }
        }
    }
    ui.add_space(10.0);
    ui.label(RichText::new("Пример использования").strong());
    match &attach.example {
        Example::None => {
            ui.weak("Пример не прикреплён");
        }
        Example::Saved => {
            ui.label("example.flpn");
        }
        Example::New(p) => {
            ui.label(format!("{} (будет скопирован при сохранении)", p.display()));
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Прикрепить пример...").clicked() {
            if let Some(file) = rfd::FileDialog::new()
                .set_title("Проект-пример")
                .add_filter("Проект FLP", &["flpn"])
                .pick_file()
            {
                attach.example = Example::New(file);
            }
        }
        if ui
            .add_enabled(attach.example != Example::None, egui::Button::new("Убрать"))
            .clicked()
        {
            attach.example = Example::None;
        }
    });
}

fn sketch_tab(ui: &mut Ui, ed: &BlockEditor, pal: &Palette) {
    match &ed.generated {
        None => {
            ui.weak("Исправьте ошибки описания блока — тогда здесь появится код.");
        }
        Some(g) => match &g.result {
            Ok(code) => {
                ui.weak(format!(
                    "Скетч с одним блоком «{}»: неподключённые входы — нули, обязательные \
                     параметры — пробные значения.",
                    flp_codegen::preview_id(&g.def)
                ));
                let mut text = code.as_str();
                let mut layouter = crate::cpphl::layouter(false);
                ui.add(
                    TextEdit::multiline(&mut text)
                        .code_editor()
                        .layouter(&mut layouter)
                        .desired_width(f32::INFINITY),
                );
            }
            Err(e) => {
                ui.colored_label(pal.error, e);
            }
        },
    }
}

fn status(ui: &mut Ui, ed: &BlockEditor, def: &Result<BlockDef, String>, pal: &Palette) {
    let (color, text): (Color32, String) = match (def, &ed.generated) {
        (Err(e), _) => (pal.error, format!("Описание: {e}")),
        (Ok(_), Some(Generated { result: Err(e), .. })) => {
            let first = e.lines().next().unwrap_or_default();
            (pal.error, format!("Код: {first}"))
        }
        _ => (pal.ok, "Описание и код в порядке".to_owned()),
    };
    ui.colored_label(color, text);
}

/// Save and close buttons; returns true when the window should close.
fn footer(
    app: &mut EditorApp,
    ui: &mut Ui,
    ed: &mut BlockEditor,
    def: &Result<BlockDef, String>,
) -> bool {
    let mut close = false;
    if ed.read_only {
        ui.horizontal(|ui| {
            ui.colored_label(
                app.theme.palette.warning,
                "Встроенный блок изменять нельзя — создайте на его основе свой.",
            );
            if ui.button("Создать копию").clicked() {
                let block = ed.original.as_ref().and_then(|k| app.lib.get(k)).cloned();
                if let Some(block) = block {
                    *ed = BlockEditor::copy_of(app, &block);
                }
            }
            close = ui.button("Закрыть").clicked();
        });
        return close;
    }
    ui.horizontal(|ui| {
        // A new block can be saved untouched; an opened one only after changes.
        let savable = ed.dirty() || ed.original.is_none();
        if ui
            .add_enabled(def.is_ok() && savable, egui::Button::new("Сохранить"))
            .clicked()
        {
            save(app, ed, def);
        }
        if ui
            .add_enabled(ed.form != ed.clean, egui::Button::new("Отменить правки"))
            .clicked()
        {
            ed.form = ed.clean.clone();
        }
        if ui.button("Закрыть").clicked() {
            if ed.dirty() {
                ed.confirm_close = true;
            } else {
                close = true;
            }
        }
        if let Some(dir) = target_dir(app, ed) {
            ui.weak(format!("Папка: {}", dir.join(&ed.form.kind).display()));
        }
    });
    close
}

/// Folder the block is saved into: where it already lies, or for a new block
/// its section and folders under `user_library/`.
fn target_dir(app: &EditorApp, ed: &BlockEditor) -> Option<PathBuf> {
    let existing = ed.original.as_ref().and_then(|k| app.lib.get(k));
    if let Some(parent) = existing.and_then(|b| b.dir.parent()) {
        return Some(parent.to_owned());
    }
    let user = app.user_lib_dir()?;
    Some(
        ed.form
            .group
            .split('/')
            .filter(|p| !p.is_empty())
            .fold(user, |d, p| d.join(p)),
    )
}

/// Writes the block into the library and reloads it; returns whether it worked.
fn save(app: &mut EditorApp, ed: &mut BlockEditor, def: &Result<BlockDef, String>) -> bool {
    let def = match def {
        Ok(def) => def,
        Err(e) => {
            app.info(LogLevel::Error, format!("Блок не сохранён: {e}"));
            return false;
        }
    };
    let Some(lib_dir) = target_dir(app, ed) else {
        app.info(LogLevel::Error, "Блок не сохранён: нет папки библиотеки");
        return false;
    };
    let kind = &def.info.kind;
    if ed.read_only || app.lib.is_builtin(kind) {
        app.info(
            LogLevel::Error,
            format!("Блок не сохранён: тип «{kind}» занят встроенным блоком, его менять нельзя"),
        );
        return false;
    }
    let new = ed.original.as_deref() != Some(kind.as_str());
    if new && (app.lib.get(kind).is_some() || lib_dir.join(kind).exists()) {
        app.info(
            LogLevel::Error,
            format!("Блок не сохранён: тип «{kind}» уже есть, выберите другое имя"),
        );
        return false;
    }
    match flp_core::save_block(&lib_dir, def, &ed.form.template, &ed.form.docs) {
        Ok(dir) => {
            app.info(
                LogLevel::Ok,
                format!("Блок «{}» сохранён в {}", def.info.name, dir.display()),
            );
            if let Err(e) = ed.form.attach.apply(&dir, &ed.form.docs) {
                app.info(
                    LogLevel::Error,
                    format!("Библиотека или пример не сохранены: {e}"),
                );
            }
            ed.original = Some(kind.clone());
            ed.clean = ed.form.clone();
            app.reload_library();
            true
        }
        Err(e) => {
            app.info(LogLevel::Error, format!("Блок не сохранён: {e}"));
            false
        }
    }
}

/// Asks before deleting a user block (`app.block_delete`) and deletes its folder.
pub fn delete_dialog(app: &mut EditorApp, ctx: &egui::Context) {
    let Some(kind) = app.block_delete.clone() else {
        return;
    };
    let (Some(block), Some(user_dir)) = (app.lib.get(&kind).cloned(), app.user_lib_dir()) else {
        app.block_delete = None;
        return;
    };
    if !app.lib.is_user(&kind) {
        app.block_delete = None;
        app.info(LogLevel::Error, "Встроенный блок удалить нельзя");
        return;
    }
    let used = app.doc.as_ref().map_or(0, |d| {
        d.project
            .logic
            .blocks
            .iter()
            .filter(|b| b.kind == kind)
            .count()
    });
    let warning = app.theme.palette.warning;
    let mut answer = None;
    egui::Modal::new(Id::unique("block_delete")).show(ctx, |ui| {
        ui.label(RichText::new("Удалить блок").strong());
        ui.label(format!(
            "Удалить пользовательский блок «{}» ({kind})? Папка {} будет удалена.",
            block.def.info.name,
            block.dir.display()
        ));
        if used > 0 {
            ui.colored_label(
                warning,
                format!(
                    "В открытом проекте таких блоков: {used}. Схема перестанет проходить проверку."
                ),
            );
        }
        ui.horizontal(|ui| {
            if ui.button("Удалить").clicked() {
                answer = Some(true);
            }
            if ui.button("Отмена").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
            {
                answer = Some(false);
            }
        });
    });
    match answer {
        Some(true) => {
            app.block_delete = None;
            // Only a folder inside user_library/ is ever removed.
            if !block.dir.starts_with(&user_dir) || block.dir == user_dir {
                app.info(
                    LogLevel::Error,
                    "Блок не удалён: он не в папке пользовательских блоков",
                );
                return;
            }
            match fs::remove_dir_all(&block.dir) {
                Ok(()) => {
                    app.info(LogLevel::Ok, format!("Блок «{kind}» удалён"));
                    if app
                        .block_editor
                        .as_ref()
                        .is_some_and(|e| e.original.as_deref() == Some(kind.as_str()))
                    {
                        app.block_editor = None;
                    }
                    app.reload_library();
                }
                Err(e) => app.info(LogLevel::Error, format!("Блок не удалён: {e}")),
            }
        }
        Some(false) => app.block_delete = None,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_round_trip() {
        let src = r#"
[block]
type = "x"
name = "X"
group = "Тест"
includes = ["A.h", "B.h"]

[[input]]
name = "k"
label = "Коэффициент"
type = "float"
default = 2.5

[[output]]
name = "q"
type = "bool"

[[param]]
name = "t"
type = "duration"
default = "1s"
min = 0
"#;
        let def: BlockDef = toml::from_str(src).unwrap();
        let block = LibraryBlock {
            def: def.clone(),
            dir: Default::default(),
            inline_templates: [(ESP32_CPP_TEMPLATE, "// @loop\n".to_owned())].into(),
        };
        let form = Form::from_block(&block);
        assert_eq!(form.includes, "A.h, B.h");
        assert_eq!(form.inputs[0].label, "Коэффициент");
        assert_eq!(form.to_def(), Ok(def));
        assert_eq!(form.template, "// @loop\n");
    }

    #[test]
    fn form_errors() {
        let mut form = Form::new_block("ok".into(), String::new());
        assert!(form.to_def().is_ok());
        form.inputs[0].default = "maybe".into();
        assert!(form.to_def().unwrap_err().contains("maybe"));
        form.inputs[0].default = "да".into();
        assert_eq!(
            form.to_def().unwrap().inputs[0].default,
            Some(ParamValue::Bool(true))
        );
        form.kind = VAR_GET.into();
        assert!(form.to_def().is_err());
        form.kind = "ok".into();
        form.outputs[0].name = "in".into();
        assert!(form.to_def().is_err());
    }

    #[test]
    fn insert_at_cursor_replaces_selection() {
        use egui::text::{CCursor, CCursorRange};
        let ctx = egui::Context::default();
        let id = Id::unique("test_template");
        let mut text = "аб".to_owned();
        insert_at_cursor(&ctx, id, &mut text, "{{ id }}");
        assert_eq!(text, "аб{{ id }}");

        let mut state = egui::text_edit::TextEditState::default();
        let range = CCursorRange::two(CCursor::new(1), CCursor::new(2));
        state.cursor.set_char_range(Some(range));
        state.store(&ctx, id);
        let mut text = "абв".to_owned();
        insert_at_cursor(&ctx, id, &mut text, "X");
        assert_eq!(text, "аXв");
        // The cursor moved after the inserted text.
        insert_at_cursor(&ctx, id, &mut text, "Y");
        assert_eq!(text, "аXYв");
    }

    #[test]
    fn docs_formatting_wraps_selection() {
        // "абв" with "б" selected.
        let (text, from, to) = format_selection("абв", 1, 2, &Fmt::Bold);
        assert_eq!((text.as_str(), from, to), ("а**б**в", 3, 4));
        // No selection: a placeholder is inserted and selected.
        let (text, from, to) = format_selection("", 0, 0, &Fmt::Code);
        assert_eq!((text.as_str(), from, to), ("`текст`", 1, 6));
        let (text, from, to) = format_selection("см. ", 4, 4, &Fmt::Link);
        assert_eq!(text, "см. [текст](https://)");
        assert_eq!((from, to), (5, 10));
        let (text, from, to) = format_selection("сайт", 0, 4, &Fmt::Link);
        assert_eq!(text, "[сайт](https://)");
        assert_eq!((from, to), (7, 15));
    }

    #[test]
    fn docs_formatting_prefixes_lines() {
        let (text, ..) = format_selection(
            "один
два
три",
            2,
            7,
            &Fmt::Bullets,
        );
        assert_eq!(
            text,
            "- один
- два
три"
        );
        // Again: the prefix goes away.
        let (text, ..) = format_selection(&text, 0, 10, &Fmt::Bullets);
        assert_eq!(
            text,
            "один
два
три"
        );
        let (text, ..) = format_selection(
            "а
б",
            0,
            3,
            &Fmt::Numbered,
        );
        assert_eq!(
            text,
            "1. а
2. б"
        );
        // A heading level replaces another one and toggles off on the same.
        let (text, ..) = format_selection("# Тема", 0, 0, &Fmt::Heading(2));
        assert_eq!(text, "## Тема");
        let (text, ..) = format_selection(&text, 0, 0, &Fmt::Heading(2));
        assert_eq!(text, "Тема");
    }

    #[test]
    fn docs_blocks_stand_on_own_lines() {
        let (text, ..) = format_selection("текст", 5, 5, &Fmt::Image("images/a.png".into()));
        assert_eq!(
            text,
            "текст
![описание](images/a.png)
"
        );
        let (text, from, to) = format_selection("", 0, 0, &Fmt::CodeBlock);
        assert_eq!(
            (text.as_str(), from, to),
            (
                "```
код
```
",
                4,
                7
            )
        );
    }

    #[test]
    fn docs_pictures_are_copied_and_pruned() {
        let dir = std::env::temp_dir().join(format!("flp_docs_img_{}", std::process::id()));
        let block = dir.join("block");
        fs::create_dir_all(&block).unwrap();
        let pic = dir.join("Схема 1.png");
        fs::write(&pic, "png").unwrap();
        fs::write(dir.join("old.png"), "png").unwrap();

        let mut attach = Attach::default();
        let target = attach.add_image(pic.clone());
        assert_eq!(target, "images/1.png");
        assert_eq!(attach.image_path(&target), Some(pic.clone()));
        // The same file again gets another name.
        assert_eq!(attach.add_image(pic), "images/1_2.png");

        let docs = format!(
            "![схема]({target})
"
        );
        attach.apply(&block, &docs).unwrap();
        let root = block.join(flp_core::blockfile::IMAGES_DIR);
        assert!(root.join("1.png").is_file());
        assert!(!root.join("1_2.png").exists());
        assert_eq!(attach.image_path(&target), Some(root.join("1.png")));

        // A picture the docs no longer use is removed with the folder.
        attach.apply(&block, "без картинок").unwrap();
        assert!(!root.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn attached_library_adds_its_header() {
        let dir = std::env::temp_dir().join(format!("flp_lib_header_{}", std::process::id()));
        let lib = dir.join("DHT sensor library");
        fs::create_dir_all(lib.join("src")).unwrap();
        fs::write(lib.join("src/DHT.h"), "").unwrap();
        fs::write(lib.join("src/DHT_U.h"), "").unwrap();
        assert_eq!(
            library_header(&lib, "dht"),
            Some("DHT.h".to_owned()),
            "header named like the library wins"
        );
        assert_eq!(library_header(&lib, "other"), None);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(with_include("Wire.h", "DHT.h"), "Wire.h, DHT.h");
        assert_eq!(with_include("DHT.h", "DHT.h"), "DHT.h");
        assert_eq!(without_include("Wire.h, DHT.h", "DHT.h"), "Wire.h");
    }

    #[test]
    fn rename_follows_in_template() {
        let t = "{{ output.out }} = {{ input.in }} + {{ input.in2 }}; {{ input[\"in\"] }} {{ param.in }}";
        assert_eq!(
            rename_in_template(t, "input", "in", "вход"),
            "{{ output.out }} = {{ input.вход }} + {{ input.in2 }}; {{ input[\"вход\"] }} {{ param.in }}"
        );
        let mut form = Form::new_block("demo".into(), String::new());
        form.inputs[0].name = "вход".into();
        form.template = rename_in_template(&form.template, "input", "in", "вход");
        let def = form.to_def().unwrap();
        assert!(flp_codegen::preview_block(&def, &form.template).is_ok());
    }

    #[test]
    fn new_block_generates() {
        let form = Form::new_block("demo".into(), String::new());
        let def = form.to_def().unwrap();
        let code = flp_codegen::preview_block(&def, &form.template).unwrap();
        assert!(
            code.contains("demo_1_out = false;") || code.contains("demo_1_out"),
            "{code}"
        );
    }
}
