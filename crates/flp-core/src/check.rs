//! Schema check: block types, parameters, links, and execution order.
//!
//! The check never stops at the first problem: it collects all diagnostics so the
//! editor and the AI can show everything at once.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;

use crate::ident::is_valid_ident;
use crate::library::{BlockDef, ParamDef, ParamKind, PortDef, SignalType};
use crate::pins::{pin_uses, Chip, PinUse};
use crate::value::parse_duration_ms;
use crate::{Layout, Library, Link, Logic, ParamValue, PortRef, Project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One problem found in the schema. Messages are user-facing, in Russian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
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

    fn block_error(&mut self, block: &str, message: String) {
        self.push(Severity::Error, Some(block), None, message);
    }

    fn link_error(&mut self, link: usize, message: String) {
        self.push(Severity::Error, None, Some(link), message);
    }

    pub(crate) fn push(
        &mut self,
        severity: Severity,
        block: Option<&str>,
        link: Option<usize>,
        message: String,
    ) {
        self.diagnostics.push(Diagnostic {
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
    check_unconnected_inputs(logic, &defs, &mut report);
    if let Some(layout) = layout {
        check_layout(logic, layout, &mut report);
    }
    check_pin_conflicts(&pin_uses(logic, lib), &mut report);
    order_blocks(logic, &defs, &edges, &mut report);
    report
}

/// [`check`] with the layout plus what depends on the board: pins the chip lacks
/// or that do not fit the block, and the UART settings.
pub fn check_project(project: &Project, lib: &Library) -> Report {
    let mut report = check(&project.logic, lib, Some(&project.layout));
    let chip = Chip::of_board(&project.config.board);
    let pins = pin_uses(&project.logic, lib);
    let uarts = crate::uart::check_project_part(project, lib, &pins, chip, &mut report);
    if let Some(chip) = chip {
        check_board_pins(chip, &pins, &mut report);
        let uart_pins: Vec<PinUse> = uarts.iter().flat_map(|u| u.pin_uses()).collect();
        check_board_pins(chip, &uart_pins, &mut report);
    }
    report
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
        report.block_error(&u.block, message);
    }
}

fn check_board_pins(chip: Chip, uses: &[PinUse], report: &mut Report) {
    for u in uses {
        let (id, gpio) = (u.block.as_str(), u.gpio);
        let Some(pin) = chip.pin(gpio) else {
            report.block_error(
                id,
                format!("блок «{id}»: у {} нет вывода GPIO{gpio}", chip.name()),
            );
            continue;
        };
        let problem = if pin.flash {
            Some("занят флеш-памятью")
        } else if !pin.fits(u.mode) {
            Some(match u.mode {
                crate::PinMode::Adc => "без АЦП",
                _ => "работает только на вход",
            })
        } else {
            None
        };
        if let Some(problem) = problem {
            report.block_error(
                id,
                format!("блок «{id}»: вывод GPIO{gpio} {problem} у {}", chip.name()),
            );
        } else if let Some(warning) = pin.warning {
            report.push(
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
            report.block_error(
                id,
                format!(
                    "недопустимый id блока «{id}»: только буквы (можно русские), цифры и «_», первой — буква, без пробелов и точек"
                ),
            );
        }
        if seen.insert(id, i).is_some() {
            report.block_error(id, format!("id «{id}» встречается дважды"));
            defs.push(None);
            continue;
        }
        let Some(lib_block) = lib.resolve(logic, block) else {
            report.block_error(id, format!("блок «{id}»: неизвестный тип «{}»", block.kind));
            defs.push(None);
            continue;
        };
        let def = &lib_block.def;
        for (name, value) in &block.params {
            match def.param(name) {
                Some(p) => check_param(id, p, value, report),
                None => report.block_error(
                    id,
                    format!(
                        "блок «{id}»: у типа «{}» нет параметра «{name}»",
                        block.kind
                    ),
                ),
            }
        }
        for (port, value) in &block.consts {
            match def.input(port) {
                Some(p) if !p.ty.accepts(value) => report.block_error(
                    id,
                    format!(
                        "блок «{id}»: константа на входе «{port}» не подходит к типу {}",
                        type_name(p.ty)
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    id,
                    format!("блок «{id}»: константа на несуществующем входе «{port}»"),
                ),
            }
        }
        for port in &block.inverted {
            match def.output(port) {
                Some(p) if p.ty != SignalType::Bool => report.block_error(
                    id,
                    format!(
                        "блок «{id}»: инвертировать можно только логический выход, «{port}» — нет"
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    id,
                    format!("блок «{id}»: инверсия несуществующего выхода «{port}»"),
                ),
            }
        }
        for port in &block.inverted_inputs {
            match def.input(port) {
                Some(p) if p.ty != SignalType::Bool => report.block_error(
                    id,
                    format!(
                        "блок «{id}»: инвертировать можно только логический вход, «{port}» — нет"
                    ),
                ),
                Some(_) => {}
                None => report.block_error(
                    id,
                    format!("блок «{id}»: инверсия несуществующего входа «{port}»"),
                ),
            }
        }
        // Conditions that fail to evaluate (e.g. comparing a string with a number).
        let conditions = def.inputs.iter().chain(&def.outputs).map(|p| &p.when);
        for when in conditions.chain(def.params.iter().map(|p| &p.when)) {
            if let Err(e) = def.enabled(when.as_deref(), &block.params) {
                report.block_error(id, format!("блок «{id}»: {e}"));
            }
        }
        for p in &def.params {
            if p.default.is_none()
                && !block.params.contains_key(&p.name)
                && def
                    .enabled(p.when.as_deref(), &block.params)
                    .unwrap_or(true)
            {
                report.block_error(id, format!("блок «{id}»: не задан параметр «{}»", p.name));
            }
        }
        if def.info.single {
            if let Some(first) = singles.insert(&block.kind, id) {
                singles.insert(&block.kind, first);
                report.block_error(
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

fn check_param(id: &str, p: &ParamDef, value: &ParamValue, report: &mut Report) {
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
            id,
            format!(
                "блок «{id}»: номер вывода «{}» не может быть отрицательным",
                p.name
            ),
        );
    }
    if let Some(min) = p.min.filter(|&min| number < min) {
        report.block_error(
            id,
            format!(
                "блок «{id}»: параметр «{}» меньше допустимого {min}",
                p.name
            ),
        );
    }
    if let Some(max) = p.max.filter(|&max| number > max) {
        report.block_error(
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
                report.link_error(link, format!("{what}: нет блока «{}»", r.block));
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
        report.link_error(link, msg);
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
                Severity::Error,
                None,
                None,
                format!("недопустимое имя переменной «{name}»"),
            );
        }
        if names.insert(name, ()).is_some() {
            report.push(
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
            report.push(
                Severity::Error,
                None,
                None,
                format!("у переменных «{other}» и «{name}» совпало имя в скетче {storage}: переименуйте одну"),
            );
        }
        if var.init.as_ref().is_some_and(|v| !var.ty.accepts(v)) {
            report.push(
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
                &block.id,
                format!("блок «{}»: нет переменной «{name}»", block.id),
            );
            continue;
        }
        if block.kind == crate::vars::VAR_SET {
            if let Some(&prev) = writers.get(name) {
                report.block_error(
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
        let used = logic
            .blocks
            .iter()
            .any(|b| b.var_name() == Some(var.name.as_str()));
        if !used {
            report.push(
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

fn type_name(ty: SignalType) -> &'static str {
    match ty {
        SignalType::Bool => "bool (логический)",
        SignalType::Int => "int (целый)",
        SignalType::Float => "float (дробный)",
        SignalType::String => "string (строка)",
        SignalType::Any => "any (любой)",
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
            if !fed && !block.consts.contains_key(&input.name) {
                report.block_error(
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
                Severity::Warning,
                None,
                None,
                format!("в разметке проекта есть координаты несуществующего блока «{id}»"),
            );
        }
    }
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
}
