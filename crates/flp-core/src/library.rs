//! Block library: folders `library/<type>/` with `block.toml`, code templates and docs.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::boards::{boards_dir, Boards, BOARDS_DIR};
use crate::ident::is_valid_ident;
use crate::toml_io::read_toml;
use crate::value::parse_duration_ms;
use crate::{Error, ParamValue, Result};

pub const BLOCK_FILE: &str = "block.toml";
pub const ESP32_CPP_TEMPLATE: &str = "esp32.cpp.tmpl";
pub const DOCS_FILE: &str = "docs.md";
/// Folder of user blocks, next to the built-in `library/`.
pub const USER_LIBRARY_DIR: &str = "user_library";

/// Folder of user blocks for a built-in library folder: `user_library/` next to it.
pub fn user_library_dir(lib_dir: &Path) -> PathBuf {
    let parent = lib_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent.join(USER_LIBRARY_DIR)
}

/// Type of a value carried by a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignalType {
    Bool,
    Int,
    Float,
    /// Text (Arduino `String`): IP addresses, messages, names.
    String,
    /// Inputs only: takes a signal of any type; the template sees the real one
    /// in `{{ input_type.<port> }}`. Not in [`SignalType::ALL`].
    Any,
    /// Math ports: an input takes `int` or `float` (never bool or string), the template
    /// sees the real type in `{{ input_type.<port> }}`. An output of this type is as wide
    /// as the widest `number` input (`int` only when all of them are `int`).
    /// Not in [`SignalType::ALL`].
    Number,
    /// Switch ports: an input takes any type, but all fed `same` inputs of a block share
    /// one (`int` with `float` gives `float`); an output of this type gets that type.
    /// Not in [`SignalType::ALL`].
    Same,
}

impl SignalType {
    pub const ALL: [SignalType; 4] = [
        SignalType::Bool,
        SignalType::Int,
        SignalType::Float,
        SignalType::String,
    ];

    /// Whether an output of type `self` may feed an input of type `to`.
    /// Only `int` → `float` widening is implicit; an `any` input takes everything.
    /// A `number` input takes `int` and `float`; a `number` output is checked once its
    /// real type is known (see [`SignalType::widest`]), here it may go to any numeric port.
    /// A `same` port takes and feeds everything here; its real type is checked later.
    pub fn can_feed(self, to: SignalType) -> bool {
        use SignalType::*;
        match (self, to) {
            (Any, _) => false,
            (_, Any | Same) => true,
            (Same, _) => true,
            (Int | Float | Number, Number) => true,
            (Number, Int | Float) => true,
            (Int, Float) => true,
            (a, b) => a == b,
        }
    }

    /// Whether the type is `int` or `float`.
    pub fn is_numeric(self) -> bool {
        matches!(self, SignalType::Int | SignalType::Float)
    }

    /// Type of a `number` output fed by inputs of these types: `float` if any is `float`.
    pub fn widest(types: impl IntoIterator<Item = SignalType>) -> SignalType {
        let mut out = SignalType::Int;
        let mut any = false;
        for t in types {
            any = true;
            if t != SignalType::Int {
                out = SignalType::Float;
            }
        }
        if any {
            out
        } else {
            SignalType::Float
        }
    }

    /// Type shared by the signals on `same` inputs: `None` for no signals, `int` with
    /// `float` widens to `float`; `Err` with the first two types that cannot meet.
    pub fn common(
        types: impl IntoIterator<Item = SignalType>,
    ) -> Result<Option<SignalType>, (SignalType, SignalType)> {
        let mut out: Option<SignalType> = None;
        for t in types {
            out = Some(match out {
                None => t,
                Some(a) if a == t => a,
                Some(a) if a.is_numeric() && t.is_numeric() => SignalType::Float,
                Some(a) => return Err((a, t)),
            });
        }
        Ok(out)
    }

    /// Constant typed for an `any` input: `true`/`false`, a whole number, a number
    /// with `.` or `,`, otherwise the text as it is.
    pub fn guess_value(text: &str) -> ParamValue {
        let t = text.trim();
        match t {
            "true" => return ParamValue::Bool(true),
            "false" => return ParamValue::Bool(false),
            _ => {}
        }
        if let Ok(v) = t.parse::<i64>() {
            return ParamValue::Int(v);
        }
        match t.replace(',', ".").parse::<f64>() {
            Ok(v) if v.is_finite() => ParamValue::Float(v),
            _ => ParamValue::Text(text.to_owned()),
        }
    }

    /// Type of a constant on an `any` input.
    pub fn of_value(value: &ParamValue) -> SignalType {
        match value {
            ParamValue::Bool(_) => SignalType::Bool,
            ParamValue::Int(_) => SignalType::Int,
            ParamValue::Float(_) => SignalType::Float,
            ParamValue::Text(_) => SignalType::String,
        }
    }

    /// Whether a constant may stand for a signal of this type.
    pub fn accepts(self, value: &ParamValue) -> bool {
        matches!(
            (self, value),
            (SignalType::Bool, ParamValue::Bool(_))
                | (SignalType::Int, ParamValue::Int(_))
                | (SignalType::Float, ParamValue::Float(_) | ParamValue::Int(_))
                | (SignalType::String, ParamValue::Text(_))
                | (
                    SignalType::Number,
                    ParamValue::Int(_) | ParamValue::Float(_)
                )
                | (SignalType::Any | SignalType::Same, _)
        )
    }

    /// Name as written in `block.toml` and the project file.
    pub fn name(self) -> &'static str {
        match self {
            SignalType::Bool => "bool",
            SignalType::Int => "int",
            SignalType::Float => "float",
            SignalType::String => "string",
            SignalType::Any => "any",
            SignalType::Number => "number",
            SignalType::Same => "same",
        }
    }
}

/// Kind of a block parameter; defines how its value is checked and rendered into code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    Bool,
    Int,
    Float,
    #[serde(rename = "string")]
    Text,
    /// Duration like `"5s"`; rendered into code as milliseconds.
    Duration,
    /// Board pin number.
    Pin,
}

impl ParamKind {
    pub fn accepts(self, value: &ParamValue) -> bool {
        match (self, value) {
            (ParamKind::Bool, ParamValue::Bool(_)) => true,
            (ParamKind::Int | ParamKind::Pin, ParamValue::Int(_)) => true,
            (ParamKind::Float, ParamValue::Float(_) | ParamValue::Int(_)) => true,
            (ParamKind::Text, ParamValue::Text(_)) => true,
            (ParamKind::Duration, ParamValue::Text(s)) => parse_duration_ms(s).is_some(),
            _ => false,
        }
    }
}

/// Contents of `block.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDef {
    #[serde(rename = "block")]
    pub info: BlockInfo,
    #[serde(rename = "input", default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<PortDef>,
    #[serde(rename = "output", default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<PortDef>,
    #[serde(rename = "param", default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Type name; must match the folder name.
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name in Russian.
    pub name: String,
    /// Short label drawn on the block: `TON`, `a + b`, `&`. Defaults to the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Library folder in the editor ("Входы и выходы", "Время", ...).
    pub group: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The block keeps state between scans (timers, triggers, counters).
    #[serde(default, skip_serializing_if = "is_false")]
    pub memory: bool,
    /// Headers the generated code needs, e.g. `DallasTemperature.h`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<String>,
    /// Arduino libraries to install before building, as arduino-cli names them
    /// (`DallasTemperature`, optionally with a version: `OneWire@2.3.8`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<String>,
    /// Only one instance per project (Wi-Fi, NTP, deep sleep).
    #[serde(default, skip_serializing_if = "is_false")]
    pub single: bool,
    /// A service of the project (Wi-Fi, web interface): set up in the side panel,
    /// not drawn on the sheets and not offered in the library.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// A hidden block that can still be dragged from the side panel onto a sheet
    /// (MQTT publication and subscription): it is drawn once it has a position.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pullable: bool,
    /// The block works through a library object shared with other blocks of the same
    /// tag (`network`: Wi-Fi, MQTT, web page, Modbus); such blocks cannot run in
    /// different tasks.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shared: String,
    /// The block works through a hardware UART set up in the board settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uart: Option<crate::uart::UartUse>,
    /// Id of the user board this block belongs to (relays soldered on it): the block
    /// is offered and allowed only in projects of that board. See [`crate::devices`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    /// The `pin` parameter may be a pin of an expander chip of the project instead of
    /// a GPIO (digital input and output). See [`crate::expander`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub expander: bool,
}

impl BlockInfo {
    /// The block may be offered in a project whose board is `model` (`board.model`):
    /// blocks of a user board belong to that board only.
    pub fn fits_board(&self, model: Option<&str>) -> bool {
        self.board
            .as_deref()
            .is_none_or(|owner| Some(owner) == model)
    }
}

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortDef {
    pub name: String,
    /// Caption in Russian shown on the block and in the properties instead of the
    /// name; the name stays the identifier used in code and links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub ty: SignalType,
    /// Value of an unconnected input. Only inputs may have it; without it
    /// the input must be connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    /// Condition on parameter values (`mode == "input"`); the port exists only
    /// while it holds. See [`BlockDef::enabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    pub name: String,
    /// Caption in Russian shown on the block and in the properties instead of the
    /// name; the name stays the identifier used in code and links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "type")]
    pub kind: ParamKind,
    /// Without a default the parameter is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParamValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Condition on other parameters; the parameter is shown and required
    /// only while it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// How the value is shown under the block on the sheet.
    #[serde(default, skip_serializing_if = "ParamShow::is_default")]
    pub show: ParamShow,
    /// Name of another text parameter: the value is kept by the program on this
    /// computer for each value of that one (a Wi-Fi password for each SSID) and
    /// never goes into the project file. See [`crate::local`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    /// For `pin` parameters: what the block does with the pin; limits the pins
    /// that fit (an output cannot go on an input-only pin).
    #[serde(default, skip_serializing_if = "PinMode::is_default")]
    pub mode: PinMode,
    /// For `pin` parameters: blocks of the same type may use one pin together
    /// (several DS18B20 on one 1-Wire bus).
    #[serde(default, skip_serializing_if = "is_false")]
    pub shared: bool,
    /// For `pin` parameters: name of a bus (`i2c-sda`). Blocks of any type with the
    /// same bus name may use one pin together (chips on one I²C bus).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bus: String,
    /// The value is set by the board (`[[device]]` of `board.toml`): it always equals
    /// `default` and is not shown in the properties, but pins of such a parameter
    /// still count in the pin check.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fixed: bool,
    /// A `bool` parameter that may come from a variable: the template uses it as a
    /// run-time condition (`if (...)`), not to choose code while generating.
    #[serde(default, skip_serializing_if = "is_false")]
    pub runtime: bool,
    /// Allowed values with captions: the properties show a drop-down list
    /// instead of a field (`[[param.choice]]` in `block.toml`).
    #[serde(rename = "choice", default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
}

/// One value of a parameter with a list of values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub value: ParamValue,
    pub label: String,
}

/// What a block does with a pin (`mode` of a `pin` parameter in `block.toml`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PinMode {
    /// Both directions (a 1-Wire or DHT bus, a chip select).
    #[default]
    Io,
    /// Input only: any pin fits.
    In,
    /// Output only.
    Out,
    /// PWM output.
    Pwm,
    /// Analog input: only pins with an ADC.
    Adc,
    /// Analog output: only pins with a DAC.
    Dac,
}

impl PinMode {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// How a parameter value is shown under the block on the sheet (`show` in `block.toml`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamShow {
    /// `caption=value`.
    #[default]
    Named,
    /// The value alone.
    Value,
    /// The value alone, masked: only the first and last characters stay (passwords).
    Secret,
    /// Not shown.
    Hidden,
}

impl ParamShow {
    pub const ALL: [ParamShow; 4] = [
        ParamShow::Named,
        ParamShow::Value,
        ParamShow::Secret,
        ParamShow::Hidden,
    ];

    fn is_default(&self) -> bool {
        *self == ParamShow::Named
    }
}

/// `p*****1`: the first and last characters, a star for each other one.
/// Up to two characters are all stars, so nothing is given away.
pub fn mask_secret(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    match chars.as_slice() {
        [first, middle @ .., last] if !middle.is_empty() => {
            format!("{first}{}{last}", "*".repeat(middle.len()))
        }
        _ => "*".repeat(chars.len()),
    }
}

impl PortDef {
    /// What to show to the user: the caption, or the name without one.
    pub fn title(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

impl ParamDef {
    /// What to show to the user: the caption, or the name without one.
    pub fn title(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// Text under the block on the sheet; `None` when the parameter is not shown
    /// (hidden, or an empty value without a caption).
    pub fn caption(&self, value: &ParamValue) -> Option<String> {
        self.caption_with(value, &str::to_owned)
    }

    /// Like [`caption`](Self::caption), with `tr` applied to the caption and
    /// to the choice label (the editor passes its translator).
    pub fn caption_with(&self, value: &ParamValue, tr: &dyn Fn(&str) -> String) -> Option<String> {
        let text = match self
            .choices
            .iter()
            .find(|c| choice_matches(&c.value, value))
        {
            Some(c) => tr(&c.label),
            None => value.to_string(),
        };
        match self.show {
            ParamShow::Named => Some(format!("{}={text}", tr(self.title()))),
            ParamShow::Hidden => None,
            _ if text.is_empty() => None,
            ParamShow::Value => Some(text),
            ParamShow::Secret => Some(mask_secret(&text)),
        }
    }

    /// The value as the user sees it: the caption of its choice, if listed.
    pub fn value_label(&self, value: &ParamValue) -> String {
        self.choices
            .iter()
            .find(|c| choice_matches(&c.value, value))
            .map_or_else(|| value.to_string(), |c| c.label.clone())
    }

    /// Whether `value` is allowed by the list of values (any value without a list).
    pub fn allows(&self, value: &ParamValue) -> bool {
        self.choices.is_empty() || self.choices.iter().any(|c| choice_matches(&c.value, value))
    }
}

/// `1` and `1.0` are the same choice of a float parameter.
fn choice_matches(choice: &ParamValue, value: &ParamValue) -> bool {
    match (choice, value) {
        (ParamValue::Int(a), ParamValue::Float(b)) | (ParamValue::Float(b), ParamValue::Int(a)) => {
            *a as f64 == *b
        }
        _ => choice == value,
    }
}

impl BlockDef {
    pub fn input(&self, name: &str) -> Option<&PortDef> {
        self.inputs.iter().find(|p| p.name == name)
    }

    pub fn output(&self, name: &str) -> Option<&PortDef> {
        self.outputs.iter().find(|p| p.name == name)
    }

    pub fn param(&self, name: &str) -> Option<&ParamDef> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Evaluates a `when` condition against the parameter values of a block instance
    /// (defaults filled in). No condition means always enabled.
    ///
    /// The condition is a Jinja expression, the same as in `{% if %}` of templates;
    /// parameters are plain variables, durations are milliseconds:
    /// `mode == "input" and time > 1000`.
    pub fn enabled(
        &self,
        when: Option<&str>,
        params: &BTreeMap<String, ParamValue>,
    ) -> Result<bool, String> {
        let Some(when) = when else { return Ok(true) };
        let env = minijinja::Environment::new();
        let expr = env
            .compile_expression(when)
            .map_err(|e| format!("условие «{when}»: {e}"))?;
        let ctx: BTreeMap<&str, minijinja::Value> = self
            .params
            .iter()
            .filter_map(|p| {
                let value = params.get(&p.name).or(p.default.as_ref())?;
                Some((p.name.as_str(), condition_value(p.kind, value)))
            })
            .collect();
        expr.eval(ctx)
            .map(|v| v.is_true())
            .map_err(|e| format!("условие «{when}»: {e}"))
    }

    /// Inputs present on a block instance; a broken condition counts as enabled
    /// (the schema check reports it).
    pub fn active_inputs<'a>(
        &'a self,
        params: &'a BTreeMap<String, ParamValue>,
    ) -> impl Iterator<Item = &'a PortDef> {
        self.inputs
            .iter()
            .filter(|p| self.enabled(p.when.as_deref(), params).unwrap_or(true))
    }

    /// Outputs present on a block instance; see [`BlockDef::active_inputs`].
    pub fn active_outputs<'a>(
        &'a self,
        params: &'a BTreeMap<String, ParamValue>,
    ) -> impl Iterator<Item = &'a PortDef> {
        self.outputs
            .iter()
            .filter(|p| self.enabled(p.when.as_deref(), params).unwrap_or(true))
    }

    /// Parameters shown for a block instance; see [`BlockDef::active_inputs`].
    pub fn active_params<'a>(
        &'a self,
        params: &'a BTreeMap<String, ParamValue>,
    ) -> impl Iterator<Item = &'a ParamDef> {
        self.params
            .iter()
            .filter(|p| self.enabled(p.when.as_deref(), params).unwrap_or(true))
    }

    /// Checks a `when` condition: it must parse and use only parameters of the block.
    fn validate_when(&self, owner: &str, when: Option<&str>) -> Result<(), String> {
        let Some(when) = when else { return Ok(()) };
        let env = minijinja::Environment::new();
        let expr = env
            .compile_expression(when)
            .map_err(|e| format!("«{owner}»: условие «{when}» не разобрано: {e}"))?;
        let mut unknown: Vec<String> = expr
            .undeclared_variables(false)
            .into_iter()
            .filter(|v| self.param(v).is_none())
            .collect();
        unknown.sort();
        if let Some(v) = unknown.first() {
            return Err(format!(
                "«{owner}»: в условии «{when}» нет такого параметра «{v}»"
            ));
        }
        Ok(())
    }

    /// Checks internal consistency; `folder` is the name of the block folder.
    pub fn validate(&self, folder: &str) -> Result<(), String> {
        if self.info.kind != folder {
            return Err(format!(
                "тип «{}» не совпадает с именем папки «{folder}»",
                self.info.kind
            ));
        }
        if !is_valid_ident(&self.info.kind) {
            return Err(format!("недопустимое имя типа «{}»", self.info.kind));
        }
        // Ports share one namespace: a link `block.port` does not say the direction.
        let mut names = HashSet::new();
        let ports = self.inputs.iter().chain(&self.outputs).map(|p| &p.name);
        for name in ports.chain(self.params.iter().map(|p| &p.name)) {
            if !is_valid_ident(name) {
                return Err(format!("недопустимое имя «{name}»"));
            }
            if name == "id" || name == "type" {
                return Err(format!("имя «{name}» зарезервировано"));
            }
            if !names.insert(name.as_str()) {
                return Err(format!("имя «{name}» встречается дважды"));
            }
        }
        for p in &self.inputs {
            if p.default.as_ref().is_some_and(|d| !p.ty.accepts(d)) {
                return Err(format!(
                    "значение по умолчанию входа «{}» не подходит к его типу",
                    p.name
                ));
            }
        }
        if let Some(p) = self
            .outputs
            .iter()
            .find(|p| p.ty == SignalType::Number)
            .filter(|_| !self.inputs.iter().any(|i| i.ty == SignalType::Number))
        {
            return Err(format!(
                "выход «{}» типа number следует типу входов, а у блока нет входов number",
                p.name
            ));
        }
        if let Some(p) = self
            .outputs
            .iter()
            .find(|p| p.ty == SignalType::Same)
            .filter(|_| !self.inputs.iter().any(|i| i.ty == SignalType::Same))
        {
            return Err(format!(
                "выход «{}» типа same следует типу входов, а у блока нет входов same",
                p.name
            ));
        }
        if let Some(p) = self.outputs.iter().find(|p| p.ty == SignalType::Any) {
            return Err(format!(
                "тип any бывает только у входа, а «{}» — выход",
                p.name
            ));
        }
        if let Some(p) = self.outputs.iter().find(|p| p.default.is_some()) {
            return Err(format!(
                "у выхода «{}» не может быть значения по умолчанию",
                p.name
            ));
        }
        for p in &self.params {
            if let Some(default) = &p.default {
                if !p.kind.accepts(default) {
                    return Err(format!(
                        "значение по умолчанию параметра «{}» не подходит к его типу",
                        p.name
                    ));
                }
                if !p.allows(default) {
                    return Err(format!(
                        "значения по умолчанию параметра «{}» нет в его списке choice",
                        p.name
                    ));
                }
            }
            if let Some(c) = p.choices.iter().find(|c| !p.kind.accepts(&c.value)) {
                return Err(format!(
                    "вариант «{}» параметра «{}» не подходит к его типу",
                    c.label, p.name
                ));
            }
            if !p.bus.is_empty() && p.kind != ParamKind::Pin {
                return Err(format!(
                    "параметр «{}» с bus должен быть выводом (type = \"pin\")",
                    p.name
                ));
            }
        }
        for p in &self.params {
            let Some(key) = &p.local else { continue };
            let key_kind = self.param(key).map(|k| k.kind);
            if key == &p.name || key_kind != Some(ParamKind::Text) || p.kind != ParamKind::Text {
                return Err(format!(
                    "параметр «{}» с local = «{key}»: оба должны быть строковыми и разными",
                    p.name
                ));
            }
            if p.default.is_none() {
                return Err(format!(
                    "у параметра «{}» с local нужно значение по умолчанию:                      на другом компьютере его может не быть",
                    p.name
                ));
            }
        }
        let ports = self.inputs.iter().chain(&self.outputs);
        for (name, when) in ports
            .map(|p| (&p.name, &p.when))
            .chain(self.params.iter().map(|p| (&p.name, &p.when)))
        {
            self.validate_when(name, when.as_deref())?;
        }
        if let Some(p) = self
            .params
            .iter()
            .find(|p| p.when.as_deref().is_some_and(|w| mentions(w, &p.name)))
        {
            return Err(format!(
                "условие параметра «{}» не может зависеть от него самого",
                p.name
            ));
        }
        Ok(())
    }
}

/// Whether expression `when` reads variable `name`.
fn mentions(when: &str, name: &str) -> bool {
    minijinja::Environment::new()
        .compile_expression(when)
        .is_ok_and(|e| e.undeclared_variables(false).contains(name))
}

/// Parameter value as seen by `when` conditions; durations are milliseconds.
fn condition_value(kind: ParamKind, value: &ParamValue) -> minijinja::Value {
    use minijinja::Value;
    match value {
        ParamValue::Bool(b) => Value::from(*b),
        ParamValue::Int(v) => Value::from(*v),
        ParamValue::Float(v) => Value::from(*v),
        ParamValue::Text(s) if kind == ParamKind::Duration => {
            Value::from(parse_duration_ms(s).unwrap_or(0))
        }
        ParamValue::Text(s) => Value::from(s.as_str()),
    }
}

/// Block loaded from a library folder.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryBlock {
    pub def: BlockDef,
    pub dir: PathBuf,
    /// Templates kept in memory: built-in blocks, which have no folder, and blocks
    /// being edited before they are saved. File name → source.
    pub inline_templates: BTreeMap<&'static str, String>,
}

impl LibraryBlock {
    /// Template source: inline for built-in blocks, otherwise read from the folder.
    pub fn template(&self, file: &str) -> Option<std::io::Result<String>> {
        if let Some(src) = self.inline_templates.get(file) {
            return Some(Ok(src.clone()));
        }
        self.template_path(file).map(fs::read_to_string)
    }

    /// Path to a template (e.g. [`ESP32_CPP_TEMPLATE`]) if the block has one.
    pub fn template_path(&self, file: &str) -> Option<PathBuf> {
        let path = self.dir.join(file);
        path.is_file().then_some(path)
    }

    pub fn docs_path(&self) -> Option<PathBuf> {
        self.template_path(DOCS_FILE)
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(BLOCK_FILE);
        let def: BlockDef = read_toml(&path)?;
        let folder = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        def.validate(folder)
            .map_err(|message| Error::InvalidBlockDef { path, message })?;
        Ok(Self {
            def,
            dir: dir.to_owned(),
            inline_templates: BTreeMap::new(),
        })
    }
}

/// Writes a block folder `<lib_dir>/<type>/`: `block.toml`, the ESP32 template and
/// `docs.md`. The definition is validated first, so a broken block never lands in
/// the library. Only files whose contents changed are rewritten (hand-written
/// comments in an unchanged `block.toml` survive); empty docs create no file.
pub fn save_block(lib_dir: &Path, def: &BlockDef, template: &str, docs: &str) -> Result<PathBuf> {
    let dir = lib_dir.join(&def.info.kind);
    def.validate(&def.info.kind)
        .map_err(|message| Error::InvalidBlockDef {
            path: dir.join(BLOCK_FILE),
            message,
        })?;
    let write_err = |path: &Path| {
        let path = path.to_owned();
        move |source| Error::Write { path, source }
    };
    fs::create_dir_all(&dir).map_err(write_err(&dir))?;

    let toml_path = dir.join(BLOCK_FILE);
    let old_def = read_toml::<BlockDef>(&toml_path).ok();
    let same_def = old_def.is_some_and(|old| old == *def);
    let mut changed = !same_def;
    if !same_def {
        let text = toml::to_string_pretty(def).map_err(|source| Error::Serialize {
            path: toml_path.clone(),
            source,
        })?;
        fs::write(&toml_path, text).map_err(write_err(&toml_path))?;
    }
    for (file, text) in [(ESP32_CPP_TEMPLATE, template), (DOCS_FILE, docs)] {
        let path = dir.join(file);
        let old = fs::read_to_string(&path).ok();
        if old.as_deref() == Some(text) || (old.is_none() && text.trim().is_empty()) {
            continue;
        }
        fs::write(&path, text).map_err(write_err(&path))?;
        changed = true;
    }
    if changed {
        // A block of a pack: the version belongs to the pack as a whole.
        crate::pack::bump(lib_dir);
    }
    Ok(dir)
}

/// All known block types: built-in ones from `library/` and user ones from
/// `user_library/`. Built-in blocks cannot be changed: a user block may not take
/// the type of a built-in one.
#[derive(Debug, Clone)]
pub struct Library {
    blocks: BTreeMap<String, LibraryBlock>,
    /// Types of user blocks.
    user: HashSet<String>,
    /// Sections and folders of the user library, as paths `Раздел/Папка`
    /// relative to `user_library/`; empty ones too.
    user_folders: BTreeSet<String>,
    /// Variable access blocks, one per access kind and signal type (`var_get_float`).
    builtins: BTreeMap<String, LibraryBlock>,
    /// Boards of `user_library/boards/`.
    boards: Boards,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            blocks: BTreeMap::new(),
            user: HashSet::new(),
            user_folders: BTreeSet::new(),
            builtins: crate::vars::builtin_blocks(),
            boards: Boards::default(),
        }
    }
}

impl Library {
    /// Loads blocks from `dir`. A broken block does not stop loading: it is skipped
    /// and its error is returned alongside the library.
    pub fn load(dir: impl AsRef<Path>) -> Result<(Self, Vec<Error>)> {
        let mut lib = Self::default();
        let errors = lib.add_dir(dir)?;
        Ok((lib, errors))
    }

    /// Loads built-in blocks from `dir` and user blocks from [`user_library_dir`]
    /// (if it exists). Problems of single blocks are returned alongside, as in
    /// [`Library::load`].
    pub fn load_with_user(dir: impl AsRef<Path>) -> Result<(Self, Vec<Error>)> {
        let dir = dir.as_ref();
        let (mut lib, mut errors) = Self::load(dir)?;
        let user = user_library_dir(dir);
        if user.is_dir() {
            errors.extend(lib.add_user_dir(&user)?);
            let (boards, board_errors) = Boards::load(&boards_dir(&user));
            errors.extend(board_errors);
            errors.extend(lib.set_boards(boards));
        }
        Ok((lib, errors))
    }

    /// User boards: pin tables and descriptions from `user_library/boards/`.
    pub fn boards(&self) -> &Boards {
        &self.boards
    }

    /// Replaces the user boards, e.g. with the ones packed into a project, together
    /// with the device blocks of the boards. A device that cannot be made is skipped
    /// and its error returned.
    pub fn set_boards(&mut self, boards: Boards) -> Vec<Error> {
        self.blocks.retain(|_, b| b.def.info.board.is_none());
        let mut errors = Vec::new();
        for board in boards.iter() {
            let (blocks, board_errors) = crate::devices::device_blocks(board, self);
            errors.extend(board_errors);
            for block in blocks {
                self.blocks.insert(block.def.info.kind.clone(), block);
            }
        }
        self.boards = boards;
        errors
    }

    /// Adds built-in blocks from one more folder; blocks with the same type are replaced.
    pub fn add_dir(&mut self, dir: impl AsRef<Path>) -> Result<Vec<Error>> {
        let (blocks, errors) = read_blocks(dir.as_ref())?;
        for block in blocks {
            self.blocks.insert(block.def.info.kind.clone(), block);
        }
        Ok(errors)
    }

    /// Adds user blocks from `dir`, which is a tree: any folder without `block.toml`
    /// is a section or folder of the library, and the group of a block is the path
    /// of its folder (`Раздел/Папка`, empty in the root). A block whose type is
    /// taken by a built-in block or by another user block is skipped with an error.
    pub fn add_user_dir(&mut self, dir: impl AsRef<Path>) -> Result<Vec<Error>> {
        let mut blocks = Vec::new();
        let mut errors = Vec::new();
        read_user_tree(
            dir.as_ref(),
            "",
            &mut blocks,
            &mut self.user_folders,
            &mut errors,
        )?;
        for (folder, mut block) in blocks {
            let kind = block.def.info.kind.clone();
            let taken = if self.is_builtin(&kind) {
                Some("встроенным блоком")
            } else if self.is_user(&kind) {
                Some("другим пользовательским блоком")
            } else {
                None
            };
            if let Some(by) = taken {
                errors.push(Error::InvalidBlockDef {
                    path: block.dir.join(BLOCK_FILE),
                    message: format!("тип «{kind}» занят {by}"),
                });
                continue;
            }
            block.def.info.group = folder;
            self.user.insert(kind.clone());
            self.blocks.insert(kind, block);
        }
        Ok(errors)
    }

    /// Whether `kind` is a user block (from `user_library/`).
    pub fn is_user(&self, kind: &str) -> bool {
        self.user.contains(kind)
    }

    /// Sections and folders of the user library (`Раздел`, `Раздел/Папка`), sorted.
    pub fn user_folders(&self) -> impl Iterator<Item = &str> {
        self.user_folders.iter().map(String::as_str)
    }

    /// Whether `kind` is a built-in block, including variable blocks.
    pub fn is_builtin(&self, kind: &str) -> bool {
        let var_block = kind == crate::vars::VAR_GET || kind == crate::vars::VAR_SET;
        var_block || (self.blocks.contains_key(kind) && !self.is_user(kind))
    }

    /// Adds or replaces a block (e.g. one created by AI before it is written to disk).
    pub fn insert(&mut self, block: LibraryBlock) {
        self.blocks.insert(block.def.info.kind.clone(), block);
    }

    /// Library block by type. Variable blocks (`var_get`, `var_set`) depend on the
    /// variable they access; use [`Library::resolve`] for blocks of a schema.
    pub fn get(&self, kind: &str) -> Option<&LibraryBlock> {
        self.blocks.get(kind)
    }

    /// Definition of a schema block, including variable blocks, whose port type comes
    /// from the variable (float while the variable is unknown).
    pub fn resolve(&self, logic: &crate::Logic, block: &crate::Block) -> Option<&LibraryBlock> {
        match crate::vars::builtin_kind(logic, block) {
            Some(kind) => self.builtins.get(&kind),
            None => self.get(&block.kind),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &LibraryBlock> {
        self.blocks.values()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Whether `name` may name a section or folder of the user library: not empty,
/// no spaces at the ends, no characters Windows forbids in file names.
pub fn is_valid_folder_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name != "."
        && name != ".."
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || r#"<>:"/\|?*"#.contains(c))
}

/// Walks the user library: block folders go to `blocks` with the path of the
/// folder they are in, other folders to `folders`.
fn read_user_tree(
    dir: &Path,
    prefix: &str,
    blocks: &mut Vec<(String, LibraryBlock)>,
    folders: &mut BTreeSet<String>,
    errors: &mut Vec<Error>,
) -> Result<()> {
    let read_err = |source| Error::Read {
        path: dir.to_owned(),
        source,
    };
    for entry in fs::read_dir(dir).map_err(read_err)? {
        let path = entry.map_err(read_err)?.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(BLOCK_FILE).is_file() {
            match LibraryBlock::load(&path) {
                Ok(block) => blocks.push((prefix.to_owned(), block)),
                Err(e) => errors.push(e),
            }
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // `boards/` holds boards, not blocks.
        if name.starts_with('.') || (prefix.is_empty() && name == BOARDS_DIR) {
            continue;
        }
        let folder = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        folders.insert(folder.clone());
        read_user_tree(&path, &folder, blocks, folders, errors)?;
    }
    Ok(())
}

/// Block folders of `dir`; broken blocks are returned as errors.
fn read_blocks(dir: &Path) -> Result<(Vec<LibraryBlock>, Vec<Error>)> {
    let read_err = |source| Error::Read {
        path: dir.to_owned(),
        source,
    };
    let mut blocks = Vec::new();
    let mut errors = Vec::new();
    for entry in fs::read_dir(dir).map_err(read_err)? {
        let path = entry.map_err(read_err)?.path();
        if !path.join(BLOCK_FILE).is_file() {
            continue;
        }
        match LibraryBlock::load(&path) {
            Ok(block) => blocks.push(block),
            Err(e) => errors.push(e),
        }
    }
    Ok((blocks, errors))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(src: &str) -> BlockDef {
        toml::from_str(src).unwrap()
    }

    const HEADER: &str = "[block]\ntype = \"x\"\nname = \"X\"\ngroup = \"Тест\"\n";

    #[test]
    fn number_ports_take_only_numbers() {
        use SignalType::*;
        assert!(Int.can_feed(Number) && Float.can_feed(Number));
        assert!(!Bool.can_feed(Number) && !String.can_feed(Number));
        assert_eq!(SignalType::widest([Int, Int]), Int);
        assert_eq!(SignalType::widest([Int, Float]), Float);
    }

    #[test]
    fn same_ports_share_one_type() {
        use SignalType::*;
        assert!(String.can_feed(Same) && Same.can_feed(Bool));
        assert_eq!(SignalType::common([]), Ok(None));
        assert_eq!(SignalType::common([String, String]), Ok(Some(String)));
        assert_eq!(SignalType::common([Int, Float, Int]), Ok(Some(Float)));
        assert_eq!(SignalType::common([Bool, Int]), Err((Bool, Int)));
        let only_out = format!("{HEADER}[[output]]\nname = \"out\"\ntype = \"same\"\n");
        let err = def(&only_out).validate("x").unwrap_err();
        assert!(err.contains("same"), "{err}");
    }

    #[test]
    fn type_widening() {
        assert!(SignalType::Int.can_feed(SignalType::Float));
        assert!(!SignalType::Float.can_feed(SignalType::Int));
        assert!(!SignalType::Bool.can_feed(SignalType::Int));
        assert!(!SignalType::Int.can_feed(SignalType::String));
        for ty in SignalType::ALL {
            assert!(ty.can_feed(SignalType::Any));
            assert!(!SignalType::Any.can_feed(ty));
        }
        assert!(SignalType::Any.accepts(&ParamValue::Text("x".into())));
        assert_eq!(SignalType::guess_value(" 12 "), ParamValue::Int(12));
        assert_eq!(SignalType::guess_value("2,5"), ParamValue::Float(2.5));
        assert_eq!(SignalType::guess_value("true"), ParamValue::Bool(true));
        assert_eq!(
            SignalType::guess_value("ON "),
            ParamValue::Text("ON ".into())
        );
        assert!(SignalType::String.accepts(&ParamValue::Text("192.168.1.1".into())));
        assert!(!SignalType::String.accepts(&ParamValue::Int(1)));
    }

    #[test]
    fn param_choices() {
        let src = format!(
            "{HEADER}[[param]]\nname = \"mode\"\ntype = \"int\"\ndefault = 0\n\
             [[param.choice]]\nvalue = 0\nlabel = \"Клиент\"\n\
             [[param.choice]]\nvalue = 1\nlabel = \"Точка доступа\"\n"
        );
        let d = def(&src);
        d.validate("x").unwrap();
        let p = d.param("mode").unwrap();
        assert_eq!(
            p.caption(&ParamValue::Int(1)).as_deref(),
            Some("mode=Точка доступа")
        );
        assert_eq!(p.value_label(&ParamValue::Int(7)), "7");
        assert!(p.allows(&ParamValue::Int(0)));
        assert!(!p.allows(&ParamValue::Int(2)));

        let bad_default = def(&src.replace("default = 0", "default = 5"));
        assert!(bad_default.validate("x").unwrap_err().contains("choice"));
        let bad_type = def(&src.replace("value = 1", "value = \"1\""));
        assert!(bad_type
            .validate("x")
            .unwrap_err()
            .contains("Точка доступа"));
    }

    #[test]
    fn param_captions() {
        assert_eq!(mask_secret("password1"), "p*******1");
        assert_eq!(mask_secret("пароль"), "п****ь");
        assert_eq!(mask_secret("ab"), "**");
        assert_eq!(mask_secret(""), "");

        let mut p: ParamDef = toml::from_str(
            "name = \"ssid\"
type = \"string\"",
        )
        .unwrap();
        let ssid = ParamValue::Text("Home".into());
        assert_eq!(p.caption(&ssid).as_deref(), Some("ssid=Home"));
        p.show = ParamShow::Value;
        assert_eq!(p.caption(&ssid).as_deref(), Some("Home"));
        assert_eq!(p.caption(&ParamValue::Text(String::new())), None);
        p.show = ParamShow::Secret;
        assert_eq!(p.caption(&ssid).as_deref(), Some("H**e"));
        p.show = ParamShow::Hidden;
        assert_eq!(p.caption(&ssid), None);
    }

    #[test]
    fn validate_ok() {
        let d = def(&format!(
            "{HEADER}[[input]]\nname = \"in\"\ntype = \"bool\"\n\
             [[param]]\nname = \"time\"\ntype = \"duration\"\ndefault = \"1s\"\n"
        ));
        assert_eq!(d.validate("x"), Ok(()));
        assert_eq!(d.param("time").unwrap().kind, ParamKind::Duration);
    }

    #[test]
    fn validate_errors() {
        assert!(def(HEADER).validate("y").is_err());
        let dup = def(&format!(
            "{HEADER}[[input]]\nname = \"a\"\ntype = \"bool\"\n\
             [[output]]\nname = \"a\"\ntype = \"bool\"\n"
        ));
        assert!(dup.validate("x").is_err());
        let bad_default = def(&format!(
            "{HEADER}[[param]]\nname = \"t\"\ntype = \"duration\"\ndefault = \"soon\"\n"
        ));
        assert!(bad_default.validate("x").is_err());
        let reserved = def(&format!(
            "{HEADER}[[param]]\nname = \"id\"\ntype = \"int\"\n"
        ));
        assert!(reserved.validate("x").is_err());
    }

    #[test]
    fn port_defaults() {
        let ok = def(&format!(
            "{HEADER}[[input]]\nname = \"en\"\ntype = \"bool\"\ndefault = true\n\
             [[input]]\nname = \"k\"\ntype = \"float\"\ndefault = 2\n"
        ));
        assert_eq!(ok.validate("x"), Ok(()));
        let wrong_type = def(&format!(
            "{HEADER}[[input]]\nname = \"en\"\ntype = \"bool\"\ndefault = 1\n"
        ));
        assert!(wrong_type.validate("x").is_err());
        let on_output = def(&format!(
            "{HEADER}[[output]]\nname = \"q\"\ntype = \"bool\"\ndefault = false\n"
        ));
        assert!(on_output.validate("x").is_err());
    }

    const CONDITIONAL: &str =
        "[[input]]\nname = \"val\"\ntype = \"int\"\nwhen = 'source == \"input\"'\n\
         [[output]]\nname = \"q\"\ntype = \"bool\"\n\
         [[param]]\nname = \"source\"\ntype = \"string\"\ndefault = \"const\"\n\
         [[param]]\nname = \"time\"\ntype = \"duration\"\nwhen = 'source == \"const\"'\n";

    #[test]
    fn when_conditions() {
        let d = def(&format!("{HEADER}{CONDITIONAL}"));
        assert_eq!(d.validate("x"), Ok(()));
        let mut params = BTreeMap::new();
        let names = |ps: Vec<&str>| ps.join(",");
        let inputs = |p: &BTreeMap<String, ParamValue>| {
            names(d.active_inputs(p).map(|i| i.name.as_str()).collect())
        };
        let shown = |p: &BTreeMap<String, ParamValue>| {
            names(d.active_params(p).map(|i| i.name.as_str()).collect())
        };
        assert_eq!(inputs(&params), "");
        assert_eq!(shown(&params), "source,time");
        params.insert("source".into(), ParamValue::Text("input".into()));
        assert_eq!(inputs(&params), "val");
        assert_eq!(shown(&params), "source");
        assert_eq!(d.active_outputs(&params).count(), 1);
    }

    #[test]
    fn when_sees_durations_in_ms() {
        let d = def(&format!(
            "{HEADER}[[output]]\nname = \"q\"\ntype = \"bool\"\nwhen = \"time > 1000\"\n\
             [[param]]\nname = \"time\"\ntype = \"duration\"\ndefault = \"1s\"\n"
        ));
        let mut params = BTreeMap::new();
        assert_eq!(d.active_outputs(&params).count(), 0);
        params.insert("time".into(), ParamValue::Text("2s".into()));
        assert_eq!(d.active_outputs(&params).count(), 1);
    }

    #[test]
    fn when_validation() {
        let unknown = def(&format!(
            "{HEADER}[[input]]\nname = \"a\"\ntype = \"bool\"\nwhen = \"nope\"\n"
        ));
        let err = unknown.validate("x").unwrap_err();
        assert!(err.contains("nope"), "{err}");
        let broken = def(&format!(
            "{HEADER}[[param]]\nname = \"m\"\ntype = \"int\"\ndefault = 0\n\
             [[input]]\nname = \"a\"\ntype = \"bool\"\nwhen = \"m ==\"\n"
        ));
        assert!(broken.validate("x").is_err());
        let itself = def(&format!(
            "{HEADER}[[param]]\nname = \"m\"\ntype = \"int\"\nwhen = \"m > 0\"\n"
        ));
        assert!(itself.validate("x").is_err());
    }

    #[test]
    fn save_block_writes_only_changes() {
        let lib = std::env::temp_dir().join(format!("flp_save_block_{}", std::process::id()));
        let _ = fs::remove_dir_all(&lib);
        let mut d = def(&format!(
            "{HEADER}[[input]]
name = \"in\"
type = \"bool\"
"
        ));
        let dir = save_block(
            &lib,
            &d,
            "// @loop
",
            "",
        )
        .unwrap();
        assert!(!dir.join(DOCS_FILE).exists());
        let loaded = LibraryBlock::load(&dir).unwrap();
        assert_eq!(loaded.def, d);

        // A hand-written comment survives a save with the same definition.
        let toml_path = dir.join(BLOCK_FILE);
        let commented = format!(
            "# hand-written
{}",
            fs::read_to_string(&toml_path).unwrap()
        );
        fs::write(&toml_path, &commented).unwrap();
        save_block(
            &lib,
            &d,
            "// @loop
",
            "Описание",
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&toml_path).unwrap(), commented);
        assert!(dir.join(DOCS_FILE).exists());

        d.info.name = "Другое".into();
        save_block(
            &lib,
            &d,
            "// @loop
",
            "",
        )
        .unwrap();
        assert_eq!(LibraryBlock::load(&dir).unwrap().def.info.name, "Другое");

        d.inputs[0].name = "id".into();
        assert!(save_block(&lib, &d, "", "").is_err());
        fs::remove_dir_all(&lib).unwrap();
    }

    #[test]
    fn labels() {
        let d = def(&format!(
            "{HEADER}[[input]]
name = \"en\"
label = \"Разрешение\"
type = \"bool\"
             [[param]]
name = \"t\"
type = \"int\"
default = 0
"
        ));
        assert_eq!(d.validate("x"), Ok(()));
        assert_eq!(d.inputs[0].title(), "Разрешение");
        assert_eq!(d.params[0].title(), "t");
        let back: BlockDef = toml::from_str(&toml::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn user_blocks() {
        let root = std::env::temp_dir().join(format!("flp_user_lib_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        let user = user_library_dir(&builtin);
        assert_eq!(user, root.join(USER_LIBRARY_DIR));
        let block = |kind: &str| {
            def(&format!(
                "[block]
type = \"{kind}\"
name = \"N\"
group = \"G\"
"
            ))
        };
        save_block(&builtin, &block("and"), "", "").unwrap();
        save_block(&user, &block("mine"), "", "").unwrap();
        save_block(&user, &block("and"), "", "").unwrap();

        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert_eq!(lib.len(), 2);
        assert!(lib.is_user("mine") && !lib.is_builtin("mine"));
        assert!(lib.is_builtin("and") && !lib.is_user("and"));
        assert!(lib.is_builtin(crate::vars::VAR_GET));
        // The user copy of a built-in type is refused, the built-in one stays.
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].to_string().contains("встроенным"));
        assert_eq!(lib.get("and").unwrap().dir, builtin.join("and"));

        // Without a user folder only built-in blocks are loaded.
        fs::remove_dir_all(&user).unwrap();
        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty() && lib.len() == 1);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn user_sections_and_folders() {
        let root = std::env::temp_dir().join(format!("flp_user_tree_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        let user = user_library_dir(&builtin);
        let block = |kind: &str| {
            def(&format!(
                "[block]\ntype = \"{kind}\"\nname = \"N\"\ngroup = \"старая\"\n"
            ))
        };
        save_block(&user, &block("top"), "", "").unwrap();
        save_block(
            &user.join("Насосы").join("Управление"),
            &block("pump"),
            "",
            "",
        )
        .unwrap();
        save_block(&user.join("Другой"), &block("pump"), "", "").unwrap();
        fs::create_dir_all(user.join("Пустой раздел")).unwrap();

        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        let folders: Vec<&str> = lib.user_folders().collect();
        assert_eq!(
            folders,
            ["Другой", "Насосы", "Насосы/Управление", "Пустой раздел"]
        );
        // The group comes from the place in the tree, not from block.toml.
        assert_eq!(lib.get("top").unwrap().def.info.group, "");
        let pump = &lib.get("pump").unwrap().def.info.group;
        assert!(pump == "Насосы/Управление" || pump == "Другой");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].to_string().contains("другим пользовательским"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn boards_folder_is_not_a_section() {
        let root = std::env::temp_dir().join(format!("flp_user_boards_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let builtin = root.join("library");
        fs::create_dir_all(&builtin).unwrap();
        let user = user_library_dir(&builtin);
        let block = def("[block]\ntype = \"top\"\nname = \"N\"\ngroup = \"\"\n");
        save_block(&user, &block, "", "").unwrap();
        let board = crate::boards::BoardDef {
            info: crate::boards::BoardInfo {
                id: "my_board".into(),
                name: "Моя плата".into(),
                chip: "esp32".into(),
                fqbn: "esp32:esp32:esp32".into(),
                version: 1,
                options: Default::default(),
            },
            pins: Vec::new(),
            expander_pins: Vec::new(),
            devices: Vec::new(),
        };
        crate::boards::save_board(&user, &board).unwrap();

        let (lib, errors) = Library::load_with_user(&builtin).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(lib.user_folders().count(), 0);
        assert!(lib.boards().get("my_board").is_some());
        assert_eq!(lib.len(), 1);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn folder_names() {
        assert!(is_valid_folder_name("Насосы"));
        assert!(is_valid_folder_name("K-Web 3.0"));
        for bad in ["", " a", "a ", "a/b", "a\\b", "a:b", "..", "a."] {
            assert!(!is_valid_folder_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn single_flag() {
        assert!(!def(HEADER).info.single);
        assert!(def(&format!("{HEADER}single = true\n")).info.single);
    }

    #[test]
    fn bus_belongs_to_pins() {
        let pin = format!(
            "{HEADER}[[param]]\nname = \"sda\"\ntype = \"pin\"\ndefault = -1\nmin = -1\nbus = \"i2c-sda\"\n"
        );
        let d = def(&pin);
        d.validate("x").unwrap();
        assert_eq!(d.param("sda").unwrap().bus, "i2c-sda");

        let bad = pin.replace("type = \"pin\"", "type = \"int\"");
        assert!(def(&bad).validate("x").unwrap_err().contains("bus"));
    }
}
