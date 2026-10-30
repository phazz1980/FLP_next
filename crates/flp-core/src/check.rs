//! Schema check: block types, parameters, links, and execution order.
//!
//! The check never stops at the first problem: it collects all diagnostics so the
//! editor and the AI can show everything at once.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;

use crate::ident::is_valid_ident;
use crate::library::{BlockDef, ParamDef, ParamKind, PortDef, SignalType};
use crate::pins::{pin_uses, BoardLayout, PinUse};
use crate::value::parse_duration_ms;
use crate::{Layout, Library, Link, Logic, ParamValue, PortRef, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Coarse, stable class of a [`Diagnostic`]. The id is what the help page
/// "Частые ошибки" and the AI refer to; messages may change, ids must not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagKind {
    BadId,
    DuplicateId,
    UnknownType,
    UnknownParam,
    MissingParam,
    ParamValue,
    Singleton,
    Condition,
    ConstantType,
    InvertType,
    LinkEnds,
    LinkType,
    InputBusy,
    InputEmpty,
    PinBusy,
    PinUnavailable,
    PinWarning,
    VariableName,
    VariableType,
    VariableLink,
    VariableWriters,
    VariableUnused,
    LayoutOrphan,
    Loop,
    Uart,
    BoardBlock,
    Expander,
    ExpanderPin,
    Tag,
}

impl DiagKind {
    pub const ALL: &'static [DiagKind] = &[
        DiagKind::BadId,
        DiagKind::DuplicateId,
        DiagKind::UnknownType,
        DiagKind::UnknownParam,
        DiagKind::MissingParam,
        DiagKind::ParamValue,
        DiagKind::Singleton,
        DiagKind::Condition,
        DiagKind::ConstantType,
        DiagKind::InvertType,
        DiagKind::LinkEnds,
        DiagKind::LinkType,
        DiagKind::InputBusy,
        DiagKind::InputEmpty,
        DiagKind::PinBusy,
        DiagKind::PinUnavailable,
        DiagKind::PinWarning,
        DiagKind::VariableName,
        DiagKind::VariableType,
        DiagKind::VariableLink,
        DiagKind::VariableWriters,
        DiagKind::VariableUnused,
        DiagKind::LayoutOrphan,
        DiagKind::Loop,
        DiagKind::Uart,
        DiagKind::BoardBlock,
        DiagKind::Expander,
        DiagKind::ExpanderPin,
        DiagKind::Tag,
    ];

    /// Stable machine-readable id, kebab-case.
    pub fn id(self) -> &'static str {
        match self {
            DiagKind::BadId => "bad-id",
            DiagKind::DuplicateId => "duplicate-id",
            DiagKind::UnknownType => "unknown-type",
            DiagKind::UnknownParam => "unknown-param",
            DiagKind::MissingParam => "missing-param",
            DiagKind::ParamValue => "param-value",
            DiagKind::Singleton => "singleton",
            DiagKind::Condition => "condition",
            DiagKind::ConstantType => "constant-type",
            DiagKind::InvertType => "invert-type",
            DiagKind::LinkEnds => "link-ends",
            DiagKind::LinkType => "link-type",
            DiagKind::InputBusy => "input-busy",
            DiagKind::InputEmpty => "input-empty",
            DiagKind::PinBusy => "pin-busy",
            DiagKind::PinUnavailable => "pin-unavailable",
            DiagKind::PinWarning => "pin-warning",
            DiagKind::VariableName => "variable-name",
            DiagKind::VariableType => "variable-type",
            DiagKind::VariableLink => "variable-link",
            DiagKind::VariableWriters => "variable-writers",
            DiagKind::VariableUnused => "variable-unused",
            DiagKind::LayoutOrphan => "layout-orphan",
            DiagKind::Loop => "loop",
            DiagKind::Uart => "uart",
            DiagKind::BoardBlock => "board-block",
            DiagKind::Expander => "expander",
            DiagKind::ExpanderPin => "expander-pin",
            DiagKind::Tag => "tag",
        }
    }
}

/// One problem found in the schema. Messages are user-facing, in Russian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagKind,
    pub severity: Severity,
    /// Block the problem belongs to, for highlighting in the editor.
    pub block: Option<String>,
    /// Index of the link in [`Logic::links`].
    pub link: Option<usize>,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "ошибка",
            Severity::Warning => "предупреждение",
        };
        write!(f, "{level}: {}", self.message)
    }
}

/// Result of [`check`].
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// Execution order as indices into [`Logic::blocks`]. Complete only without errors.
    pub order: Vec<usize>,
    /// Links that close a feedback loop through a block with memory: their value
    /// comes from the previous scan. Indices into [`Logic::links`].
    pub feedback: Vec<usize>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    fn block_error(&mut self, kind: DiagKind, block: &str, message: String) {
        self.push(kind, Severity::Error, Some(block), None, message);
    }

    fn link_error(&mut self, kind: DiagKind, link: usize, message: String) {
        self.push(kind, Severity::Error, None, Some(link), message);
    }

    pub(crate) fn push(
        &mut self,
        kind: DiagKind,
        severity: Severity,
        block: Option<&str>,
        link: Option<usize>,
        message: String,
    ) {
        self.diagnostics.push(Diagnostic {
            kind,
            severity,
            block: block.map(str::to_owned),
            link,
            message,
        });
    }
}

/// Checks the schema against the library. `layout` is optional and only produces warnings.
pub fn check(logic: &Logic, lib: &Library, layout: Option<&Layout>) -> Report {
    let mut report = Report::default();
    let defs = check_blocks(logic, lib, &mut report);
    let mut edges = check_links(logic, &defs, &mut report);
    edges.extend(check_vars(logic, &defs, &mut report));
    edges.extend(check_tags(logic, &defs, &mut report));
    check_unconnected_inputs(logic, &defs, &mut report);
    if let Some(layout) = layout {
        check_layout(logic, layout, &mut report);
    }
    check_pin_conflicts(&pin_uses(logic, lib), &mut report);
    check_expanders(logic, lib, &mut report);
    order_blocks(logic, &defs, &edges, &mut report);
    if !report.has_errors() {
        check_number_flow(logic, &defs, &mut report);
    }
    report
}

/// Real type of every output once `number` outputs follow their inputs, by
/// (block index, port name). Blocks go in execution order; an input whose source
/// is not known yet (a feedback loop) counts as `float`.
pub fn resolved_outputs<'a>(
    logic: &'a Logic,
    defs: &[Option<&'a BlockDef>],
    order: &[usize],
) -> HashMap<(usize, &'a str), SignalType> {
    let mut types: HashMap<(usize, &str), SignalType> = HashMap::new();
    for &bi in order {
        let (block, Some(def)) = (&logic.blocks[bi], defs[bi]) else {
            continue;
        };
        let numbers: Vec<SignalType> = def
            .active_inputs(&block.params)
            .filter(|p| p.ty == SignalType::Number)
            .map(|p| input_type(logic, defs, &types, block, p))
            .collect();
        for out in def.active_outputs(&block.params) {
            let ty = if out.ty == SignalType::Number {
                SignalType::widest(numbers.iter().copied())
            } else {
                out.ty
            };
            types.insert((bi, out.name.as_str()), ty);
        }
    }
    types
}

/// Real type of the signal on an input port: of its link, variable or constant.
fn input_type(
    logic: &Logic,
    defs: &[Option<&BlockDef>],
    types: &HashMap<(usize, &str), SignalType>,
    block: &crate::Block,
    port: &crate::PortDef,
) -> SignalType {
    let from_value = |v: &crate::ParamValue| SignalType::of_value(v);
    if let Some(var) = block.bound.get(&port.name).and_then(|v| logic.var(v)) {
        return var.ty.signal();
    }
    if let Some(link) = logic
        .links
        .iter()
        .find(|l| l.to.block == block.id && l.to.port == port.name)
    {
        let src = logic.blocks.iter().position(|b| b.id == link.from.block);
        return src
            .and_then(|i| {
                types
                    .get(&(i, link.from.port.as_str()))
                    .copied()
                    .or_else(|| {
                        defs[i]?
                            .output(&link.from.port)
                            .map(|o| o.ty)
                            .filter(|t| *t != SignalType::Number)
                    })
            })
            .unwrap_or(SignalType::Float);
    }
    block
        .consts
        .get(&port.name)
        .or(port.default.as_ref())
        .map_or(SignalType::Float, from_value)
}

/// A `number` output that turned out `float` cannot go to an `int` input: the
/// narrowing needs an explicit block.
fn check_number_flow(logic: &Logic, defs: &[Option<&BlockDef>], report: &mut Report) {
    let types = resolved_outputs(logic, defs, &report.order.clone());
    for (li, link) in logic.links.iter().enumerate() {
        let (Some(fi), Some(ti)) = (
            logic.blocks.iter().position(|b| b.id == link.from.block),
            logic.blocks.iter().position(|b| b.id == link.to.block),
        ) else {
            continue;
        };
        let (Some(fd), Some(td)) = (defs[fi], defs[ti]) else {
            continue;
        };
        let (Some(out), Some(inp)) = (fd.output(&link.from.port), td.input(&link.to.port)) else {
            continue;
        };
        if out.ty != SignalType::Number {
            continue;
        }
        let real = types
            .get(&(fi, link.from.port.as_str()))
            .copied()
            .unwrap_or(SignalType::Float);
        if !real.can_feed(inp.ty) {
            report.link_error(
                DiagKind::LinkType,
                li,
                format!(
                    "связь {} → {}: выход получился {}, а вход ждёт {}; нужен блок преобразования",
                    link.from,
                    link.to,
                    type_name(real),
                    type_name(inp.ty)
                ),
            );
        }
    }
}

/// [`check`] with the layout plus what depends on the board: pins the chip lacks
/// or that do not fit the block, and the UART settings.
pub fn check_project(project: &Project, lib: &Library) -> Report {
    let links = project.enabled_links();
    let project = &project.without_disabled();
    let mut report = check(&project.logic, lib, Some(&project.layout));
    // An unused tag waits in the project tree: its inputs are not fed yet.
    let unused = project.unused_tags(lib);
    report.diagnostics.retain(|d| {
        d.kind != DiagKind::InputEmpty || d.block.as_ref().is_none_or(|b| !unused.contains(b))
    });
    // Link indices refer to the project's own list, not to the trimmed one.
    for d in &mut report.diagnostics {
        d.link = d.link.and_then(|i| links.get(i).copied());
    }
    check_board_blocks(project, lib, &mut report);
    let layout = BoardLayout::of_board(&project.config.board, lib.boards());
    let pins = pin_uses(&project.logic, lib);
    let uarts = crate::uart::check_project_part(
        project,
        lib,
        &pins,
        layout.as_ref().map(BoardLayout::chip),
        &mut report,
    );
    if let Some(layout) = &layout {
        // The blocks of the board's own devices sit on the pins the board has fixed for
        // them, so the board's remarks about those pins ("taken by the bus") are not for them.
        let own: HashSet<&str> = project
            .logic
            .blocks
            .iter()
            .filter(|b| lib.get(&b.kind).is_some_and(|l| l.def.info.board.is_some()))
            .map(|b| b.id.as_str())
            .collect();
        check_board_pins(layout, &pins, &own, &mut report);
        let uart_pins: Vec<PinUse> = uarts.iter().flat_map(|u| u.pin_uses()).collect();
        check_board_pins(layout, &uart_pins, &own, &mut report);
        check_board_expanders(project, lib, layout, &mut report);
    }
    report
}

/// Blocks on pins of an expander that the board carries: the pin must be one the board
/// brings out and must do what the block wants.
fn check_board_expanders(
    project: &Project,
    lib: &Library,
    layout: &BoardLayout,
    report: &mut Report,
) {
    for u in crate::expander::expander_uses(&project.logic, lib) {
        let Some(e) = project.logic.expander(&u.expander) else {
            continue;
        };
        let Some(board) = layout.expander_for(e) else {
            continue;
        };
        let id = u.block.as_str();
        if u.pin < 0 || u.pin >= i64::from(e.chip.pins()) {
            continue;
        }
        let problem = if board.pin(u.pin).is_none() {
            "не выведен на плате"
        } else if board.fits(u.pin, u.mode) {
            continue;
        } else {
            match u.mode {
                crate::PinMode::In => "не работает как цифровой вход",
                crate::PinMode::Out => "не работает как цифровой выход",
                _ => "не работает как цифровой вход и выход",
            }
        };
        report.block_error(
            DiagKind::ExpanderPin,
            id,
            format!(
                "блок «{id}»: вывод P{} расширителя «{}» {problem} у {}",
                u.pin,
                u.expander,
                layout.name()
            ),
        );
    }
}

/// A block made for a user board (its relays) works only in a project of that board.
fn check_board_blocks(project: &Project, lib: &Library, report: &mut Report) {
    let model = project.config.board.model.as_deref();
    for block in &project.logic.blocks {
        let Some(owner) = lib
            .get(&block.kind)
            .and_then(|b| b.def.info.board.as_deref())
            .filter(|&owner| Some(owner) != model)
        else {
            continue;
        };
        let owner_name = lib
            .boards()
            .get(owner)
            .map_or(owner, |b| b.def.info.name.as_str());
        let id = block.id.as_str();
        report.block_error(
            DiagKind::BoardBlock,
            id,
            format!(
                "блок «{id}»: тип «{}» сделан для платы «{owner_name}», а в проекте выбрана другая; выберите эту плату в настройках проекта или замените блок",
                block.kind
            ),
        );
    }
}

/// Declarations of the expanders and the blocks standing on their pins.
fn check_expanders(logic: &Logic, lib: &Library, report: &mut Report) {
    let mut ids: HashSet<&str> = HashSet::new();
    for (i, e) in logic.expanders.iter().enumerate() {
        let message = if !ids.insert(e.id.as_str()) {
            Some(format!("расширитель «{}» объявлен дважды", e.id))
        } else if let Some(problem) = e.problem() {
            Some(problem)
        } else {
            logic.expanders[..i]
                .iter()
                .find(|o| o.address == e.address && o.sda == e.sda && o.scl == e.scl)
                .map(|o| {
                    format!(
                        "расширитель «{}»: адрес 0x{:02X} на этой шине уже занят расширителем «{}»",
                        e.id, e.address, o.id
                    )
                })
        };
        if let Some(message) = message {
            report.push(DiagKind::Expander, Severity::Error, None, None, message);
        }
    }
    for block in logic.blocks.iter().filter(|b| b.tag.is_none()) {
        let Some(name) = &block.expander else {
            continue;
        };
        let id = block.id.as_str();
        let supported = lib
            .resolve(logic, block)
            .is_none_or(|l| l.def.info.expander);
        if !supported {
            report.block_error(
                DiagKind::Expander,
                id,
                format!(
                    "блок «{id}»: тип «{}» не может стоять на выводе расширителя",
                    block.kind
                ),
            );
        } else if logic.expander(name).is_none() {
            report.block_error(
                DiagKind::Expander,
                id,
                format!("блок «{id}»: расширитель «{name}» не найден"),
            );
        }
    }
    let uses = crate::expander::expander_uses(logic, lib);
    for (i, u) in uses.iter().enumerate() {
        let Some(e) = logic.expander(&u.expander) else {
            continue;
        };
        let id = u.block.as_str();
        if u.pin < 0 || u.pin >= i64::from(e.chip.pins()) {
            report.block_error(
                DiagKind::ExpanderPin,
                id,
                format!(
                    "блок «{id}»: у {} расширителя «{}» нет вывода P{}, допустимы P0–P{}",
                    e.chip.name(),
                    e.id,
                    u.pin,
                    e.chip.pins() - 1
                ),
            );
        } else if let Some(first) = uses[..i]
            .iter()
            .find(|o| o.expander == u.expander && o.pin == u.pin)
        {
            report.block_error(
                DiagKind::ExpanderPin,
                id,
                format!(
                    "блок «{id}»: вывод P{} расширителя «{}» уже занят блоком «{}»",
                    u.pin, u.expander, first.block
                ),
            );
        }
    }
}

/// Two blocks (or two parameters of one block) on one pin, unless the pin is
/// shared on purpose (a 1-Wire bus).
fn check_pin_conflicts(uses: &[PinUse], report: &mut Report) {
    for (i, u) in uses.iter().enumerate() {
        let Some(first) = uses[..i]
            .iter()
            .find(|other| other.gpio == u.gpio && !other.can_share(u))
        else {
            continue;
        };
        let message = if first.block == u.block {
            format!(
                "блок «{}»: параметры «{}» и «{}» на одном выводе GPIO{}",
                u.block, first.param, u.param, u.gpio
            )
        } else {
            format!(
                "блок «{}»: вывод GPIO{} уже занят блоком «{}»",
                u.block, u.gpio, first.block
            )
        };
        report.block_error(DiagKind::PinBusy, &u.block, message);
    }
}

/// Pins a block uses against the board: missing ones and unfit ones are errors, the
/// pin's own remark a warning, except for the `quiet` blocks.
fn check_board_pins(
    layout: &BoardLayout,
    uses: &[PinUse],
    quiet: &HashSet<&str>,
    report: &mut Report,
) {
    for u in uses {
        let (id, gpio) = (u.block.as_str(), u.gpio);
        let Some(pin) = layout.pin(gpio) else {
            report.block_error(
                DiagKind::PinUnavailable,
                id,
                format!("блок «{id}»: у {} нет вывода GPIO{gpio}", layout.name()),
            );
            continue;
        };
        let problem = if pin.flash {
            Some("занят флеш-памятью")
        } else if !pin.fits(u.mode) {
            Some(match u.mode {
                crate::PinMode::Adc => "без АЦП",
                crate::PinMode::Dac => "без ЦАП",
                crate::PinMode::Pwm => "не умеет ШИМ",
                crate::PinMode::In => "не работает как цифровой вход",
                _ if pin.input_only => "работает только на вход",
                crate::PinMode::Out => "не работает как цифровой выход",
                crate::PinMode::Io => "не работает как цифровой вход и выход",
            })
        } else {
            None
        };
        if let Some(problem) = problem {
            report.block_error(
                DiagKind::PinUnavailable,
                id,
                format!(
                    "блок «{id}»: вывод GPIO{gpio} {problem} у {}",
                    layout.name()
                ),
            );
        } else if let Some(warning) = pin.warning.as_ref().filter(|_| !quiet.contains(id)) {
            report.push(
                DiagKind::PinWarning,
                Severity::Warning,
                Some(id),
                None,
                format!("блок «{id}»: вывод GPIO{gpio} — {warning}"),
            );
        }
    }
}

/// Checks ids, types and parameters. Returns the definition of each block
/// (`None` for unknown types and duplicates).
fn check_blocks<'a>(
    logic: &Logic,
    lib: &'a Library,
    report: &mut Report,
) -> Vec<Option<&'a BlockDef>> {
    let mut seen = HashMap::new();
    let mut singles: HashMap<&str, &str> = HashMap::new();
    let mut defs = Vec::with_capacity(logic.blocks.len());
    for (i, block) in logic.blocks.iter().enumerate() {
        let id = block.id.as_str();
        if !is_valid_ident(id) {
            report.block_error(DiagKind::BadId,
                id,
                format!(
                    "недопустимый id блока «{id}»: только буквы (можно русские), цифры, «_» и «-» (имя может состоять и из одних цифр), без пробелов и точек"
                ),
            );
        }
        if seen.insert(id, i).is_some() {
            report.block_error(
                DiagKind::DuplicateId,
                id,
                format!("id «{id}» встречается дважды"),
            );
            defs.push(None);
            continue;
        }
        let Some(lib_block) = lib.resolve(logic, block) else {
            report.block_error(
                DiagKind::UnknownType,
                id,
                format!("блок «{id}»: неизвестный тип «{}»", block.kind),
            );
            defs.push(None);
            continue;
        };
        let def = &lib_block.def;
        for (name, value) in &block.params {
            match def.param(name) {
                Some(p) => {
                    check_param(id, p, value, report);
                    if block.kind == "wifi" {
                        check_wifi_text(id, &p.name, value, report);
                    }
                }
                None => report.block_error(
                    DiagKind::UnknownParam,
                    id,
                    format!(
                        "блок «{id}»: у типа «{}» нет параметра «{name}»",
                        block.kind
                    ),
                ),
            }
        }
        check_bindings(logic, block, def, report);
        for (port, value) in &block.consts {
            match def.input(port) {
                Some(p) if !p.ty.accepts(value) => report.block_error(
                    DiagKind::ConstantType,
                    id,
                    format!(
                        "блок «{id}»: константа на входе «{port}» не подходит к типу {}",
                        type_name(p.ty)
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    DiagKind::ConstantType,
                    id,
                    format!("блок «{id}»: константа на несуществующем входе «{port}»"),
                ),
            }
        }
        for port in &block.inverted {
            match def.output(port) {
                Some(p) if p.ty != SignalType::Bool => report.block_error(
                    DiagKind::InvertType,
                    id,
                    format!(
                        "блок «{id}»: инвертировать можно только логический выход, «{port}» — нет"
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    DiagKind::InvertType,
                    id,
                    format!("блок «{id}»: инверсия несуществующего выхода «{port}»"),
                ),
            }
        }
        for port in &block.inverted_inputs {
            match def.input(port) {
                Some(p) if p.ty != SignalType::Bool => report.block_error(
                    DiagKind::InvertType,
                    id,
                    format!(
                        "блок «{id}»: инвертировать можно только логический вход, «{port}» — нет"
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    DiagKind::InvertType,
                    id,
                    format!("блок «{id}»: инверсия несуществующего входа «{port}»"),
                ),
            }
        }
        // Conditions that fail to evaluate (e.g. comparing a string with a number).
        let conditions = def.inputs.iter().chain(&def.outputs).map(|p| &p.when);
        for when in conditions
            .chain(def.params.iter().map(|p| &p.when))
            .filter(|_| block.tag.is_none())
        {
            if let Err(e) = def.enabled(when.as_deref(), &block.params) {
                report.block_error(DiagKind::Condition, id, format!("блок «{id}»: {e}"));
            }
        }
        // A copy's parameters are its tag's: they are checked on the tag.
        for p in def.params.iter().filter(|_| block.tag.is_none()) {
            if p.default.is_none()
                && !block.params.contains_key(&p.name)
                && !block.bound.contains_key(&p.name)
                && def
                    .enabled(p.when.as_deref(), &block.params)
                    .unwrap_or(true)
            {
                report.block_error(
                    DiagKind::MissingParam,
                    id,
                    format!("блок «{id}»: не задан параметр «{}»", p.name),
                );
            }
        }
        if def.info.single && block.tag.is_none() {
            if let Some(first) = singles.insert(&block.kind, id) {
                singles.insert(&block.kind, first);
                report.block_error(
                    DiagKind::Singleton,
                    id,
                    format!(
                        "блок «{id}»: «{}» может быть в проекте только один, уже есть «{first}»",
                        def.info.name
                    ),
                );
            }
        }
        defs.push(Some(def));
    }
    defs
}

/// Wi-Fi limits: SSID is 1..=32 bytes; a password is empty (open network), 8..=63 printable
/// ASCII characters, or exactly 64 hex digits (a raw PSK).
pub fn wifi_text_error(name: &str, text: &str) -> Option<String> {
    match name {
        "ssid" | "ap_ssid" => {
            if text.len() > 32 {
                return Some("имя сети — не более 32 байт".to_owned());
            }
            if text.chars().any(|c| c.is_control()) {
                return Some("имя сети содержит управляющие символы".to_owned());
            }
            None
        }
        "password" | "ap_password" => {
            if text.is_empty() {
                return None;
            }
            if let Some(c) = text.chars().find(|c| !(' '..='~').contains(c)) {
                return Some(format!(
                    "в пароле недопустимый символ «{c}» (разрешены латиница, цифры и знаки ASCII)"
                ));
            }
            let hex64 = text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit());
            if !(8..=63).contains(&text.len()) && !hex64 {
                return Some(format!(
                    "длина пароля {} — нужно 8–63 символа (или 64 шестнадцатеричных)",
                    text.len()
                ));
            }
            None
        }
        _ => None,
    }
}

fn check_wifi_text(id: &str, name: &str, value: &ParamValue, report: &mut Report) {
    if let ParamValue::Text(s) = value {
        if let Some(e) = wifi_text_error(name, s) {
            report.block_error(
                DiagKind::ParamValue,
                id,
                format!("блок «{id}»: параметр «{name}» — {e}"),
            );
        }
    }
}

pub(crate) fn check_param(id: &str, p: &ParamDef, value: &ParamValue, report: &mut Report) {
    if !p.kind.accepts(value) {
        let expected = match p.kind {
            ParamKind::Bool => "true или false",
            ParamKind::Int => "целое число",
            ParamKind::Float => "число",
            ParamKind::Text => "строка в кавычках",
            ParamKind::Duration => "время вида \"500ms\", \"5s\", \"2m\", \"1h\"",
            ParamKind::Pin => "номер вывода",
        };
        report.block_error(
            DiagKind::ParamValue,
            id,
            format!("блок «{id}»: параметр «{}» — ожидается {expected}", p.name),
        );
        return;
    }
    if !p.allows(value) {
        let list: Vec<String> = p
            .choices
            .iter()
            .map(|c| format!("{} ({})", c.value, c.label))
            .collect();
        report.block_error(
            DiagKind::ParamValue,
            id,
            format!(
                "блок «{id}»: параметр «{}» — допустимо только {}",
                p.name,
                list.join(", ")
            ),
        );
        return;
    }
    let number = match value {
        ParamValue::Int(v) => Some(*v as f64),
        ParamValue::Float(v) => Some(*v),
        ParamValue::Text(s) if p.kind == ParamKind::Duration => {
            parse_duration_ms(s).map(|ms| ms as f64)
        }
        _ => None,
    };
    let Some(number) = number else { return };
    // A pin parameter allowed below zero (`min = -1`) uses it for "not used".
    if p.kind == ParamKind::Pin && number < 0.0 && p.min.is_none_or(|min| min >= 0.0) {
        report.block_error(
            DiagKind::ParamValue,
            id,
            format!(
                "блок «{id}»: номер вывода «{}» не может быть отрицательным",
                p.name
            ),
        );
    }
    if let Some(min) = p.min.filter(|&min| number < min) {
        report.block_error(
            DiagKind::ParamValue,
            id,
            format!(
                "блок «{id}»: параметр «{}» меньше допустимого {min}",
                p.name
            ),
        );
    }
    if let Some(max) = p.max.filter(|&max| number > max) {
        report.block_error(
            DiagKind::ParamValue,
            id,
            format!(
                "блок «{id}»: параметр «{}» больше допустимого {max}",
                p.name
            ),
        );
    }
}

/// Valid link between two known blocks.
#[derive(Debug, Clone, Copy)]
struct Edge {
    /// Index in [`Logic::links`]; `None` for the implicit order of variable writes
    /// before reads.
    link: Option<usize>,
    from: usize,
    to: usize,
}

/// Resolves link ends to blocks and ports.
struct Resolver<'a, 'l> {
    logic: &'l Logic,
    defs: &'l [Option<&'a BlockDef>],
    /// Ids of blocks with a known definition.
    index: HashMap<&'l str, usize>,
}

impl<'a> Resolver<'a, '_> {
    /// Returns the block index and port of one link end, or reports why it is invalid.
    fn end(
        &self,
        r: &PortRef,
        is_input: bool,
        link: usize,
        what: &str,
        report: &mut Report,
    ) -> Option<(usize, &'a PortDef)> {
        let Some(&bi) = self.index.get(r.block.as_str()) else {
            // A known id with an unknown type was already reported with the block.
            if self.logic.block(&r.block).is_none() {
                report.link_error(
                    DiagKind::LinkEnds,
                    link,
                    format!("{what}: нет блока «{}»", r.block),
                );
            }
            return None;
        };
        let def = self.defs[bi]?;
        let (port, wrong) = if is_input {
            (def.input(&r.port), def.output(&r.port))
        } else {
            (def.output(&r.port), def.input(&r.port))
        };
        let params = &self.logic.blocks[bi].params;
        let msg = match (port, wrong) {
            (Some(p), _) if !def.enabled(p.when.as_deref(), params).unwrap_or(true) => format!(
                "{what}: порт «{r}» отключён параметрами блока ({})",
                p.when.as_deref().unwrap_or_default()
            ),
            (Some(p), _) => return Some((bi, p)),
            (None, Some(_)) if is_input => {
                format!("{what}: «{r}» — это выход, а связь должна вести во вход")
            }
            (None, Some(_)) => {
                format!("{what}: «{r}» — это вход, а связь должна начинаться с выхода")
            }
            (None, None) => format!(
                "{what}: у блока «{}» ({}) нет порта «{}»",
                r.block, def.info.kind, r.port
            ),
        };
        report.link_error(DiagKind::LinkEnds, link, msg);
        None
    }
}

/// Checks link ends, directions, types and duplicate sources. Returns valid edges.
fn check_links(logic: &Logic, defs: &[Option<&BlockDef>], report: &mut Report) -> Vec<Edge> {
    let index = logic
        .blocks
        .iter()
        .enumerate()
        .filter(|(i, _)| defs[*i].is_some())
        .map(|(i, b)| (b.id.as_str(), i))
        .collect();
    let resolver = Resolver { logic, defs, index };
    let mut sources: HashMap<(usize, &str), usize> = HashMap::new();
    let mut edges = Vec::new();

    for (li, link) in logic.links.iter().enumerate() {
        let Link { from, to } = link;
        let what = format!("связь {from} → {to}");
        let src = resolver.end(from, false, li, &what, report);
        let dst = resolver.end(to, true, li, &what, report);
        let (Some((fi, fp)), Some((ti, tp))) = (src, dst) else {
            continue;
        };
        if !fp.ty.can_feed(tp.ty) {
            report.link_error(
                DiagKind::LinkType,
                li,
                format!(
                    "{what}: тип выхода {} не подходит ко входу {}",
                    type_name(fp.ty),
                    type_name(tp.ty)
                ),
            );
            continue;
        }
        if let Some(prev) = sources.insert((ti, to.port.as_str()), li) {
            let prev_from = &logic.links[prev].from;
            report.link_error(
                DiagKind::InputBusy,
                li,
                format!("{what}: на вход «{to}» уже подан «{prev_from}»"),
            );
            continue;
        }
        edges.push(Edge {
            link: Some(li),
            from: fi,
            to: ti,
        });
    }
    edges
}

/// Checks variable declarations and the blocks that access them. Returns implicit
/// edges "write before read" for the execution order.
fn check_vars(logic: &Logic, defs: &[Option<&BlockDef>], report: &mut Report) -> Vec<Edge> {
    let mut names = HashMap::new();
    let mut storages = HashMap::new();
    for var in &logic.vars {
        let name = var.name.as_str();
        if !crate::vars::is_valid_var_name(name) {
            report.push(
                DiagKind::VariableName,
                Severity::Error,
                None,
                None,
                format!("недопустимое имя переменной «{name}»"),
            );
        }
        if names.insert(name, ()).is_some() {
            report.push(
                DiagKind::VariableName,
                Severity::Error,
                None,
                None,
                format!("переменная «{name}» объявлена дважды"),
            );
        }
        let storage = crate::vars::storage_name(name);
        if let Some(other) = storages
            .insert(storage.clone(), name)
            .filter(|&o| o != name)
        {
            report.push(DiagKind::VariableName,
                Severity::Error,
                None,
                None,
                format!("у переменных «{other}» и «{name}» совпало имя в скетче {storage}: переименуйте одну"),
            );
        }
        if var.init.as_ref().is_some_and(|v| !var.ty.accepts(v)) {
            report.push(
                DiagKind::VariableType,
                Severity::Error,
                None,
                None,
                format!(
                    "начальное значение переменной «{name}» не подходит к типу {}",
                    var.ty.name()
                ),
            );
        }
    }

    let mut writers: HashMap<&str, usize> = HashMap::new();
    let mut readers: Vec<(&str, usize)> = Vec::new();
    for (i, block) in logic.blocks.iter().enumerate() {
        if defs[i].is_none() {
            continue;
        }
        let Some(name) = block.var_name() else {
            continue;
        };
        if logic.var(name).is_none() {
            report.block_error(
                DiagKind::VariableLink,
                &block.id,
                format!("блок «{}»: нет переменной «{name}»", block.id),
            );
            continue;
        }
        if block.kind == crate::vars::VAR_SET {
            if let Some(&prev) = writers.get(name) {
                report.block_error(
                    DiagKind::VariableWriters,
                    &block.id,
                    format!(
                        "переменная «{name}» уже записывается блоком «{}»",
                        logic.blocks[prev].id
                    ),
                );
            } else {
                writers.insert(name, i);
            }
        } else {
            readers.push((name, i));
        }
    }
    for var in &logic.vars {
        let used = logic.blocks.iter().any(|b| {
            b.var_name() == Some(var.name.as_str()) || b.bound.values().any(|v| *v == var.name)
        });
        if !used {
            report.push(
                DiagKind::VariableUnused,
                Severity::Warning,
                None,
                None,
                format!("переменная «{}» не используется", var.name),
            );
        }
    }
    readers
        .into_iter()
        .filter_map(|(name, to)| {
            writers.get(name).map(|&from| Edge {
                link: None,
                from,
                to,
            })
        })
        .collect()
}

/// Checks the ports and parameters a block takes from variables.
fn check_bindings(logic: &Logic, block: &crate::Block, def: &BlockDef, report: &mut Report) {
    let id = block.id.as_str();
    for (name, var) in &block.bound {
        let Some(v) = logic.var(var) else {
            report.block_error(
                DiagKind::VariableLink,
                id,
                format!("блок «{id}»: «{name}» связан с несуществующей переменной «{var}»"),
            );
            continue;
        };
        let want = if let Some(p) = def.input(name).or_else(|| def.output(name)) {
            Some(p.ty)
        } else {
            def.param(name)
                .and_then(|p| bindable_type(p.kind, !p.choices.is_empty(), p.runtime))
        };
        match want {
            None => report.block_error(
                DiagKind::VariableLink,
                id,
                format!("блок «{id}»: «{name}» нельзя связать с переменной"),
            ),
            Some(ty)
                if ty != SignalType::Any
                    && ty != v.ty.signal()
                    && !(ty == SignalType::Number && v.ty.signal().is_numeric()) =>
            {
                report.block_error(
                    DiagKind::VariableType,
                    id,
                    format!(
                        "блок «{id}»: «{name}» — {}, а переменная «{var}» — {}",
                        type_name(ty),
                        v.ty.name()
                    ),
                )
            }
            Some(_) => {}
        }
    }
}

/// Signal type a parameter takes from a variable; `None` if it cannot be bound
/// (the value is needed while generating: pins, durations, choices, flags).
pub fn bindable_type(kind: crate::ParamKind, choice: bool, runtime: bool) -> Option<SignalType> {
    use crate::ParamKind;
    if choice {
        return None;
    }
    match kind {
        ParamKind::Text => Some(SignalType::String),
        ParamKind::Int => Some(SignalType::Int),
        ParamKind::Float => Some(SignalType::Float),
        ParamKind::Bool if runtime => Some(SignalType::Bool),
        _ => None,
    }
}

fn type_name(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool (логический)",
        SignalType::Int => "int (целый)",
        SignalType::Float => "float (дробный)",
        SignalType::String => "string (строка)",
        SignalType::Any => "any (любой)",
        SignalType::Number => "число (int или float)",
    }
}

fn check_unconnected_inputs(logic: &Logic, defs: &[Option<&BlockDef>], report: &mut Report) {
    for (bi, block) in logic.blocks.iter().enumerate() {
        let Some(def) = defs[bi] else { continue };
        let required = def
            .active_inputs(&block.params)
            .filter(|i| i.default.is_none());
        for input in required {
            // Any link counts: an invalid one was already reported, don't pile up.
            let fed = logic
                .links
                .iter()
                .any(|l| l.to.block == block.id && l.to.port == input.name);
            if !fed
                && !block.consts.contains_key(&input.name)
                && !block.bound.contains_key(&input.name)
            {
                report.block_error(
                    DiagKind::InputEmpty,
                    &block.id,
                    format!(
                        "блок «{}»: на вход «{}» ничего не подано",
                        block.id, input.name
                    ),
                );
            }
        }
    }
}

fn check_layout(logic: &Logic, layout: &Layout, report: &mut Report) {
    for id in layout.blocks.keys() {
        if logic.block(id).is_none() {
            report.push(
                DiagKind::LayoutOrphan,
                Severity::Warning,
                None,
                None,
                format!("в разметке проекта есть координаты несуществующего блока «{id}»"),
            );
        }
    }
}

/// Copies of tags: the tag must exist, be no copy itself, have the same type and a
/// board pin. Returns edges "tag before the copy" for copies of inputs, so that the
/// copy gives the value read in the same scan.
fn check_tags(logic: &Logic, defs: &[Option<&BlockDef>], report: &mut Report) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (i, block) in logic.blocks.iter().enumerate() {
        let Some(tag) = &block.tag else { continue };
        let id = block.id.as_str();
        let target = logic.blocks.iter().position(|b| b.id == *tag);
        let problem = match target.map(|t| (t, &logic.blocks[t])) {
            None => Some(format!("копия «{id}»: нет тега «{tag}»")),
            Some((_, t)) if t.tag.is_some() => {
                Some(format!("копия «{id}»: «{tag}» сам копия, ссылаться надо на тег"))
            }
            Some((_, t)) if t.kind != block.kind => Some(format!(
                "копия «{id}»: тип «{}» не совпадает с типом тега «{tag}» («{}»)",
                block.kind, t.kind
            )),
            Some((t, _)) if !defs[t].is_some_and(|d| d.params.iter().any(|p| p.kind == ParamKind::Pin)) => {
                Some(format!("копия «{id}»: у «{tag}» нет вывода платы, копировать можно только входы и выходы"))
            }
            Some((t, _)) => {
                if defs[i].is_some_and(|d| !d.outputs.is_empty()) {
                    edges.push(Edge {
                        link: None,
                        from: t,
                        to: i,
                    });
                }
                None
            }
        };
        if let Some(message) = problem {
            report.block_error(DiagKind::Tag, id, message);
        }
    }
    edges
}

/// Builds the execution order. Feedback loops are allowed only through a block with
/// memory: links into such blocks inside a loop take the previous scan's value.
fn order_blocks(logic: &Logic, defs: &[Option<&BlockDef>], edges: &[Edge], report: &mut Report) {
    let n = logic.blocks.len();
    let has_memory = |i: usize| defs[i].is_some_and(|d| d.info.memory);

    let mut succ = vec![Vec::new(); n];
    for e in edges {
        succ[e.from].push(e.to);
    }
    let scc = strongly_connected(&succ);
    let in_loop = |e: &Edge| scc[e.from] == scc[e.to];

    let mut indegree = vec![0usize; n];
    let mut active = vec![Vec::new(); n];
    for e in edges {
        if in_loop(e) && has_memory(e.to) {
            report.feedback.extend(e.link);
        } else {
            active[e.from].push(e.to);
            indegree[e.to] += 1;
        }
    }

    // Kahn's algorithm; ties are broken by declaration order for stable output.
    let mut ready: BinaryHeap<Reverse<usize>> =
        (0..n).filter(|&i| indegree[i] == 0).map(Reverse).collect();
    while let Some(Reverse(i)) = ready.pop() {
        report.order.push(i);
        for &j in &active[i] {
            indegree[j] -= 1;
            if indegree[j] == 0 {
                ready.push(Reverse(j));
            }
        }
    }

    if report.order.len() < n {
        // Report only the loops themselves; blocks downstream of a loop are stuck too,
        // but that is a consequence, not a separate problem.
        let mut stuck: Vec<usize> = (0..n)
            .filter(|&i| indegree[i] > 0 && succ[i].iter().any(|&j| scc[j] == scc[i]))
            .collect();
        stuck.sort_by_key(|&i| (scc[i], i));
        for group in stuck.chunk_by(|&a, &b| scc[a] == scc[b]) {
            let ids: Vec<&str> = group.iter().map(|&i| logic.blocks[i].id.as_str()).collect();
            report.block_error(
                DiagKind::Loop,
                ids[0],
                format!(
                    "цикл без блока с памятью: {} — разорвите его триггером или таймером",
                    ids.join(", ")
                ),
            );
        }
    }
}

/// Tarjan's algorithm. Returns the component number of each node.
fn strongly_connected(succ: &[Vec<usize>]) -> Vec<usize> {
    struct State<'a> {
        succ: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        comp: Vec<usize>,
        next_index: usize,
        next_comp: usize,
    }

    fn visit(s: &mut State, v: usize) {
        s.index[v] = Some(s.next_index);
        s.low[v] = s.next_index;
        s.next_index += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for k in 0..s.succ[v].len() {
            let w = s.succ[v][k];
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(wi) if s.on_stack[w] => s.low[v] = s.low[v].min(wi),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            while let Some(w) = s.stack.pop() {
                s.on_stack[w] = false;
                s.comp[w] = s.next_comp;
                if w == v {
                    break;
                }
            }
            s.next_comp += 1;
        }
    }

    let n = succ.len();
    let mut s = State {
        succ,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        comp: vec![0; n],
        next_index: 0,
        next_comp: 0,
    };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.comp
}

#[cfg(test)]
mod tests {
    #[test]
    fn diag_kind_ids_are_unique_kebab_case() {
        let mut seen = std::collections::HashSet::new();
        for k in DiagKind::ALL {
            let id = k.id();
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{id}"
            );
            assert!(seen.insert(id), "duplicate kind id {id}");
        }
    }

    use super::*;
    use crate::{BlockDef, LibraryBlock};

    /// Library with `src` (output only), `not` (bool → bool) and `latch`
    /// (bool → bool with memory).
    fn library() -> Library {
        let mut lib = Library::default();
        for (kind, inputs, memory) in [
            ("src", "", false),
            ("not", "in", false),
            ("latch", "in", true),
        ] {
            let input = if inputs.is_empty() {
                String::new()
            } else {
                format!("[[input]]\nname = \"{inputs}\"\ntype = \"bool\"\n")
            };
            let def: BlockDef = toml::from_str(&format!(
                "[block]\ntype = \"{kind}\"\nname = \"{kind}\"\ngroup = \"Тест\"\nmemory = {memory}\n\
                 {input}[[output]]\nname = \"out\"\ntype = \"bool\"\n"
            ))
            .unwrap();
            lib.insert(LibraryBlock {
                def,
                dir: Default::default(),
                inline_templates: Default::default(),
            });
        }
        lib
    }

    fn run(blocks: &[(&str, &str)], links: &[(&str, &str)]) -> (Logic, Report) {
        let mut src = String::new();
        for (id, kind) in blocks {
            src += &format!("[[block]]\nid = \"{id}\"\ntype = \"{kind}\"\n");
        }
        for (from, to) in links {
            src += &format!("[[link]]\nfrom = \"{from}\"\nto = \"{to}\"\n");
        }
        let logic: Logic = toml::from_str(&src).unwrap();
        let report = check(&logic, &library(), None);
        (logic, report)
    }

    fn ids(logic: &Logic, order: &[usize]) -> Vec<String> {
        order.iter().map(|&i| logic.blocks[i].id.clone()).collect()
    }

    fn bound(var_type: &str, port: &str) -> Report {
        let src = format!(
            "[[var]]
name = \"v\"
type = \"{var_type}\"
             [[block]]
id = \"n\"
type = \"not\"
[block.bind]
{port} = \"v\"
"
        );
        let logic: Logic = toml::from_str(&src).unwrap();
        check(&logic, &library(), None)
    }

    #[test]
    fn bound_input_counts_as_fed() {
        let r = bound("bool", "in");
        assert!(!r.has_errors(), "{:?}", r.diagnostics);
    }

    #[test]
    fn bound_type_mismatch_is_error() {
        assert!(bound("float", "in").has_errors());
    }

    #[test]
    fn bound_unknown_var_is_error() {
        let src = "[[block]]
id = \"n\"
type = \"not\"
[block.bind]
in = \"zz\"
";
        let logic: Logic = toml::from_str(src).unwrap();
        assert!(check(&logic, &library(), None).has_errors());
    }

    #[test]
    fn chain_order() {
        let (logic, r) = run(
            &[("c", "not"), ("b", "not"), ("a", "src")],
            &[("a.out", "b.in"), ("b.out", "c.in")],
        );
        assert!(!r.has_errors(), "{:#?}", r.diagnostics);
        assert_eq!(ids(&logic, &r.order), ["a", "b", "c"]);
        assert!(r.feedback.is_empty());
    }

    #[test]
    fn loop_without_memory_is_error() {
        let (_, r) = run(
            &[("x", "not"), ("y", "not")],
            &[("x.out", "y.in"), ("y.out", "x.in")],
        );
        let errors: Vec<_> = r.errors().map(|d| d.message.as_str()).collect();
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert!(
            errors[0].contains("цикл без блока с памятью: x, y"),
            "{errors:#?}"
        );
    }

    #[test]
    fn self_loop_without_memory_is_error() {
        let (_, r) = run(&[("x", "not")], &[("x.out", "x.in")]);
        assert_eq!(r.errors().count(), 1, "{:#?}", r.diagnostics);
    }

    #[test]
    fn downstream_of_loop_is_not_reported_separately() {
        let (_, r) = run(
            &[("x", "not"), ("y", "not"), ("z", "not")],
            &[("x.out", "y.in"), ("y.out", "x.in"), ("y.out", "z.in")],
        );
        assert_eq!(r.errors().count(), 1, "{:#?}", r.diagnostics);
    }

    #[test]
    fn loop_through_memory_is_allowed() {
        // x → m → x: the link into the memory block takes the previous scan's value.
        let (logic, r) = run(
            &[("x", "not"), ("m", "latch")],
            &[("x.out", "m.in"), ("m.out", "x.in")],
        );
        assert!(!r.has_errors(), "{:#?}", r.diagnostics);
        assert_eq!(r.feedback, [0]);
        assert_eq!(ids(&logic, &r.order), ["m", "x"]);
    }

    /// Library with `src` (bool output) and `timer`, whose input `val` exists only
    /// with `source = "input"` and whose required `time` only with `source = "const"`.
    /// `wifi` is a single-instance block.
    fn conditional_library() -> Library {
        let mut lib = library();
        for src in [
            "[block]\ntype = \"timer\"\nname = \"Таймер\"\ngroup = \"Тест\"\n\
             [[input]]\nname = \"val\"\ntype = \"bool\"\nwhen = 'source == \"input\"'\n\
             [[output]]\nname = \"out\"\ntype = \"bool\"\n\
             [[param]]\nname = \"source\"\ntype = \"string\"\ndefault = \"const\"\n\
             [[param]]\nname = \"time\"\ntype = \"duration\"\nwhen = 'source == \"const\"'\n",
            "[block]\ntype = \"wifi\"\nname = \"Wi-Fi\"\ngroup = \"Тест\"\nsingle = true\n",
        ] {
            let def: BlockDef = toml::from_str(src).unwrap();
            assert_eq!(def.validate(&def.info.kind), Ok(()));
            lib.insert(LibraryBlock {
                def,
                dir: Default::default(),
                inline_templates: Default::default(),
            });
        }
        lib
    }

    fn check_src(src: &str) -> Vec<String> {
        let logic: Logic = toml::from_str(src).unwrap();
        check(&logic, &conditional_library(), None)
            .errors()
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn disabled_port_and_param() {
        // Default source "const": `time` is required, `val` does not exist.
        let errors = check_src(
            "[[block]]\nid = \"a\"\ntype = \"src\"\n\
             [[block]]\nid = \"t\"\ntype = \"timer\"\n\
             [[link]]\nfrom = \"a.out\"\nto = \"t.val\"\n",
        );
        assert_eq!(errors.len(), 2, "{errors:#?}");
        assert!(errors
            .iter()
            .any(|e| e.contains("не задан параметр «time»")));
        assert!(errors.iter().any(|e| e.contains("отключён параметрами")));

        // Source "input": `val` must be connected, `time` is not needed.
        let errors = check_src("[[block]]\nid = \"t\"\ntype = \"timer\"\nsource = \"input\"\n");
        assert_eq!(errors.len(), 1, "{errors:#?}");
        assert!(errors[0].contains("на вход «val» ничего не подано"));
        let errors = check_src(
            "[[block]]\nid = \"a\"\ntype = \"src\"\n\
             [[block]]\nid = \"t\"\ntype = \"timer\"\nsource = \"input\"\n\
             [[link]]\nfrom = \"a.out\"\nto = \"t.val\"\n",
        );
        assert!(errors.is_empty(), "{errors:#?}");
    }

    #[test]
    fn broken_condition_value_is_reported() {
        // Adding a number to a string fails at evaluation.
        let mut lib = conditional_library();
        let def: BlockDef = toml::from_str(
            "[block]\ntype = \"odd\"\nname = \"odd\"\ngroup = \"Тест\"\n\
             [[output]]\nname = \"q\"\ntype = \"bool\"\nwhen = \"m + 1 > 0\"\n\
             [[param]]\nname = \"m\"\ntype = \"string\"\ndefault = \"x\"\n",
        )
        .unwrap();
        lib.insert(LibraryBlock {
            def,
            dir: Default::default(),
            inline_templates: Default::default(),
        });
        let logic: Logic = toml::from_str("[[block]]\nid = \"o\"\ntype = \"odd\"\n").unwrap();
        let r = check(&logic, &lib, None);
        assert_eq!(r.errors().count(), 1, "{:#?}", r.diagnostics);
    }

    #[test]
    fn single_block() {
        let errors = check_src("[[block]]\nid = \"w\"\ntype = \"wifi\"\n");
        assert!(errors.is_empty(), "{errors:#?}");
        let errors = check_src(
            "[[block]]\nid = \"w1\"\ntype = \"wifi\"\n\
             [[block]]\nid = \"w2\"\ntype = \"wifi\"\n\
             [[block]]\nid = \"w3\"\ntype = \"wifi\"\n",
        );
        assert_eq!(errors.len(), 2, "{errors:#?}");
        assert!(errors[0].contains("«w2»") && errors[0].contains("уже есть «w1»"));
        assert!(errors[1].contains("«w3»") && errors[1].contains("уже есть «w1»"));
    }

    #[test]
    fn memory_block_outside_loop_is_not_feedback() {
        let (_, r) = run(&[("a", "src"), ("m", "latch")], &[("a.out", "m.in")]);
        assert!(!r.has_errors(), "{:#?}", r.diagnostics);
        assert!(r.feedback.is_empty());
    }

    /// Library with `out` (pin for output), `ain` (ADC pin), `bus` (shared pin)
    /// and `uart` (two pins, -1 means "not used").
    fn pin_library() -> Library {
        let mut lib = Library::default();
        for src in [
            "[block]
type = \"out\"
name = \"o\"
group = \"Тест\"
             [[param]]
name = \"pin\"
type = \"pin\"
mode = \"out\"
",
            "[block]
type = \"ain\"
name = \"a\"
group = \"Тест\"
             [[param]]
name = \"pin\"
type = \"pin\"
mode = \"adc\"
",
            "[block]
type = \"pwm\"
name = \"p\"
group = \"Тест\"
             [[param]]
name = \"pin\"
type = \"pin\"
mode = \"pwm\"
",
            "[block]
type = \"din\"
name = \"d\"
group = \"Тест\"
             [[param]]
name = \"pin\"
type = \"pin\"
mode = \"in\"
",
            "[block]
type = \"bus\"
name = \"b\"
group = \"Тест\"
             [[param]]
name = \"pin\"
type = \"pin\"
shared = true
",
            "[block]
type = \"uart\"
name = \"u\"
group = \"Тест\"
             [[param]]
name = \"rx\"
type = \"pin\"
min = -1
default = -1
             [[param]]
name = \"tx\"
type = \"pin\"
min = -1
default = -1
",
        ] {
            let def: BlockDef = toml::from_str(src).unwrap();
            lib.insert(LibraryBlock {
                def,
                dir: Default::default(),
                inline_templates: Default::default(),
            });
        }
        lib
    }

    fn pin_project(blocks: &[(&str, &str, &str)]) -> Project {
        let mut src = String::new();
        for (id, kind, params) in blocks {
            src += &format!(
                "[[block]]
id = \"{id}\"
type = \"{kind}\"
{params}
"
            );
        }
        Project {
            path: Default::default(),
            config: toml::from_str(
                "[project]
name = \"p\"
[board]
fqbn = \"esp32:esp32:esp32\"
",
            )
            .unwrap(),
            logic: toml::from_str(&src).unwrap(),
            layout: Default::default(),
        }
    }

    fn pin_messages(blocks: &[(&str, &str, &str)]) -> Vec<String> {
        check_project(&pin_project(blocks), &pin_library())
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn pin_used_twice() {
        let m = pin_messages(&[
            ("a", "out", "pin = 4"),
            ("b", "out", "pin = 4"),
            ("c", "out", "pin = 5"),
        ]);
        assert_eq!(m, ["блок «b»: вывод GPIO4 уже занят блоком «a»"]);
        let m = pin_messages(&[(
            "u",
            "uart",
            "rx = 16
tx = 16",
        )]);
        assert_eq!(
            m,
            ["блок «u»: параметры «rx» и «tx» на одном выводе GPIO16"]
        );
    }

    #[test]
    fn shared_pin_and_unused_pins() {
        let m = pin_messages(&[
            ("t1", "bus", "pin = 4"),
            ("t2", "bus", "pin = 4"),
            ("u1", "uart", ""),
            ("u2", "uart", ""),
        ]);
        assert!(m.is_empty(), "{m:#?}");
        let m = pin_messages(&[("t1", "bus", "pin = 4"), ("o", "out", "pin = 4")]);
        assert_eq!(m, ["блок «o»: вывод GPIO4 уже занят блоком «t1»"]);
    }

    #[test]
    fn pins_of_the_board() {
        let m = pin_messages(&[
            ("a", "out", "pin = 6"),
            ("b", "out", "pin = 34"),
            ("c", "ain", "pin = 5"),
            ("d", "out", "pin = 40"),
            ("e", "out", "pin = 1"),
            ("f", "ain", "pin = 36"),
        ]);
        assert_eq!(
            m,
            [
                "блок «a»: вывод GPIO6 занят флеш-памятью у ESP32",
                "блок «b»: вывод GPIO34 работает только на вход у ESP32",
                "блок «c»: вывод GPIO5 без АЦП у ESP32",
                "блок «d»: у ESP32 нет вывода GPIO40",
                "блок «e»: вывод GPIO1 — UART0: заливка и отладка по USB",
            ]
        );
    }

    #[test]
    fn pins_of_a_user_board() {
        use crate::boards::{BoardDef, BoardInfo, BoardPin, Boards, UserBoard};
        let pin = |gpio, name: &str, busy: Option<&str>| BoardPin {
            gpio,
            name: name.into(),
            note: String::new(),
            busy: busy.map(Into::into),
            functions: None,
        };
        let def = BoardDef {
            info: BoardInfo {
                id: "mine".into(),
                name: "Моя плата".into(),
                chip: "esp32".into(),
                fqbn: "esp32:esp32:esp32".into(),
                version: 1,
                options: Default::default(),
            },
            pins: vec![
                pin(4, "OUT1", None),
                pin(36, "A1", None),
                pin(15, "SCL", Some("шина I2C")),
            ],
            expander_pins: Vec::new(),
            devices: Vec::new(),
        };
        let mut boards = Boards::default();
        boards.insert(UserBoard {
            def,
            dir: Default::default(),
        });
        let mut lib = pin_library();
        lib.set_boards(boards);
        let mut project = pin_project(&[
            ("a", "out", "pin = 4"),
            // The chip has GPIO5, the board does not bring it out.
            ("b", "out", "pin = 5"),
            ("c", "ain", "pin = 36"),
            ("d", "out", "pin = 15"),
        ]);
        project.config.board.model = Some("mine".into());
        let m: Vec<String> = check_project(&project, &lib)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect();
        assert_eq!(
            m,
            [
                "блок «b»: у Моя плата нет вывода GPIO5",
                "блок «d»: вывод GPIO15 — шина I2C",
            ]
        );
        // Without the model the same schema is checked against the chip table.
        project.config.board.model = None;
        assert!(!check_project(&project, &lib)
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Моя плата")));
    }

    #[test]
    fn a_pin_without_the_needed_function_is_refused() {
        use crate::boards::{BoardDef, BoardInfo, BoardPin, Boards, UserBoard};
        use crate::PinFunction::*;
        let pin = |gpio, functions: Option<Vec<_>>| BoardPin {
            gpio,
            name: String::new(),
            note: String::new(),
            busy: None,
            functions,
        };
        let mut boards = Boards::default();
        boards.insert(UserBoard {
            def: BoardDef {
                info: BoardInfo {
                    id: "mine".into(),
                    name: "Моя плата".into(),
                    chip: "esp32".into(),
                    fqbn: "esp32:esp32:esp32".into(),
                    version: 1,
                    options: Default::default(),
                },
                pins: vec![
                    pin(4, Some(vec![Dout])),
                    pin(5, Some(vec![Din, Pwm])),
                    pin(36, Some(vec![Adc])),
                    pin(13, None),
                ],
                expander_pins: Vec::new(),
                devices: Vec::new(),
            },
            dir: Default::default(),
        });
        let mut lib = pin_library();
        lib.set_boards(boards);
        let mut project = pin_project(&[
            ("ok_out", "out", "pin = 4"),
            ("ok_pwm", "pwm", "pin = 5"),
            ("ok_adc", "ain", "pin = 36"),
            ("ok_any", "din", "pin = 13"),
            // GPIO4 has an ADC on the chip, but the board gives it only as an output
            ("no_adc", "ain", "pin = 4"),
            ("no_pwm", "pwm", "pin = 4"),
            ("no_in", "din", "pin = 4"),
            ("no_out", "out", "pin = 5"),
        ]);
        project.config.board.model = Some("mine".into());
        let m: Vec<String> = check_project(&project, &lib)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .filter(|m| m.contains("у Моя плата"))
            .collect();
        assert_eq!(
            m,
            [
                "блок «no_adc»: вывод GPIO4 без АЦП у Моя плата",
                "блок «no_pwm»: вывод GPIO4 не умеет ШИМ у Моя плата",
                "блок «no_in»: вывод GPIO4 не работает как цифровой вход у Моя плата",
                "блок «no_out»: вывод GPIO5 не работает как цифровой выход у Моя плата",
            ]
        );
    }

    #[test]
    fn a_block_of_another_board() {
        use crate::boards::{BoardDef, BoardInfo, Boards, UserBoard};
        let mut boards = Boards::default();
        boards.insert(UserBoard {
            def: BoardDef {
                info: BoardInfo {
                    id: "mine".into(),
                    name: "Моя плата".into(),
                    chip: "esp32".into(),
                    fqbn: "esp32:esp32:esp32".into(),
                    version: 1,
                    options: Default::default(),
                },
                pins: Vec::new(),
                expander_pins: Vec::new(),
                devices: Vec::new(),
            },
            dir: Default::default(),
        });
        let mut lib = pin_library();
        lib.set_boards(boards);
        let mut relay = lib.get("out").unwrap().clone();
        relay.def.info.kind = "mine_out".into();
        relay.def.info.board = Some("mine".into());
        lib.insert(relay);

        let mut project = pin_project(&[("r", "mine_out", "pin = 4")]);
        // A project of a built-in board, or of no board, cannot use it.
        for model in [None, Some("other")] {
            project.config.board.model = model.map(Into::into);
            let report = check_project(&project, &lib);
            let kinds: Vec<_> = report.errors().map(|d| d.kind).collect();
            assert_eq!(kinds, [DiagKind::BoardBlock], "{:?}", report.diagnostics);
            let message = &report.diagnostics[0].message;
            assert!(message.contains("для платы «Моя плата»"), "{message}");
        }
        project.config.board.model = Some("mine".into());
        let report = check_project(&project, &lib);
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }

    #[test]
    fn the_board_does_not_warn_its_own_devices_about_its_pins() {
        use crate::boards::{BoardDef, BoardInfo, BoardPin, Boards, UserBoard};
        let mut boards = Boards::default();
        boards.insert(UserBoard {
            def: BoardDef {
                info: BoardInfo {
                    id: "mine".into(),
                    name: "Моя плата".into(),
                    chip: "esp32".into(),
                    fqbn: "esp32:esp32:esp32".into(),
                    version: 1,
                    options: Default::default(),
                },
                pins: vec![BoardPin {
                    gpio: 4,
                    name: "SDA".into(),
                    note: String::new(),
                    busy: Some("шина I2C".into()),
                    functions: None,
                }],
                expander_pins: Vec::new(),
                devices: Vec::new(),
            },
            dir: Default::default(),
        });
        let mut lib = pin_library();
        lib.set_boards(boards);
        let mut relay = lib.get("out").unwrap().clone();
        relay.def.info.kind = "mine_out".into();
        relay.def.info.board = Some("mine".into());
        lib.insert(relay);

        let mut project = pin_project(&[("r", "mine_out", "pin = 4"), ("o", "out", "pin = 4")]);
        project.config.board.model = Some("mine".into());
        let m: Vec<String> = check_project(&project, &lib)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect();
        // Only the ordinary block hears about the bus (and about sharing the pin).
        assert!(
            m.iter()
                .any(|s| s.contains("«o»") && s.contains("шина I2C")),
            "{m:?}"
        );
        assert!(
            !m.iter()
                .any(|s| s.contains("«r»") && s.contains("шина I2C")),
            "{m:?}"
        );
    }
}

#[cfg(test)]
mod wifi_text_tests {
    use super::wifi_text_error;

    #[test]
    fn password_rules() {
        assert!(wifi_text_error("password", "").is_none());
        assert!(wifi_text_error("password", "12345678").is_none());
        assert!(wifi_text_error("password", "short").is_some());
        assert!(wifi_text_error("ap_password", &"a".repeat(64)).is_none());
        assert!(wifi_text_error("ap_password", &"g".repeat(64)).is_some());
        assert!(wifi_text_error("password", "парольпароль").is_some());
        assert!(wifi_text_error("ssid", &"a".repeat(33)).is_some());
        assert!(wifi_text_error("ssid", "Сеть").is_none());
    }
}
