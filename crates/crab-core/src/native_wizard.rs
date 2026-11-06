//! Wizard model for the native part of a block built on an Arduino library (`specs/native-wizard`).
//!
//! The user fills a few fields (library, class, constructor arguments, outputs, actions);
//! [`LibraryWizard::generate`] turns them into `native.cpp`. The fields travel inside the file in
//! one comment line (`// crab-wizard: {json}`), so the block opens in the wizard again until the
//! code is edited by hand.

use serde::{Deserialize, Serialize};

use crate::native::is_cpp_ident;

/// Prefix of the comment line that carries the wizard fields.
const MARKER: &str = "// crab-wizard: ";

/// Type of a parameter or an output the wizard creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WizType {
    Pin,
    Int,
    Float,
    Bool,
}

impl WizType {
    fn cpp(self) -> &'static str {
        match self {
            WizType::Bool => "bool",
            WizType::Pin | WizType::Int => "int32_t",
            WizType::Float => "float",
        }
    }
}

/// A parameter of the block made on the "object" step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WizParam {
    pub name: String,
    pub ty: WizType,
}

/// One argument of the constructor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WizArg {
    /// A parameter of the block, by name.
    Param(String),
    /// A constant or a constant expression (`HX_GAIN64_A`, `9600`).
    Const(String),
}

/// An output: `out.<name> = (<type>)obj-><expr>`, only when `when` holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WizOutput {
    pub name: String,
    pub ty: WizType,
    /// A call of the library object, e.g. `read()`.
    pub expr: String,
    /// Empty: every cycle. A call of the object (`available()`) or the name of an earlier
    /// `bool` output of this block.
    #[serde(default)]
    pub when: String,
}

/// An action on the rising edge of a `bool` input: `obj->call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WizAction {
    pub input: String,
    pub call: String,
}

/// Everything the wizard asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryWizard {
    /// Name in the Arduino library manager.
    pub lib: String,
    /// Header file, e.g. `GyverHX711.h`.
    pub header: String,
    /// C++ class of the object (`GyverHX711`, `ns::Class`).
    pub class: String,
    pub params: Vec<WizParam>,
    pub args: Vec<WizArg>,
    /// Calls made once after the object is created (`begin()`).
    pub init: Vec<String>,
    pub outputs: Vec<WizOutput>,
    pub actions: Vec<WizAction>,
}

/// A C++ name, possibly qualified with `::`.
fn is_class_name(name: &str) -> bool {
    !name.is_empty() && name.split("::").all(is_cpp_ident)
}

/// A one-line C++ expression the user typed: not empty, no `;`, no comments, no line breaks.
fn check_expr(what: &str, text: &str, errors: &mut Vec<String>) {
    let t = text.trim();
    if t.is_empty() {
        errors.push(format!("{what}: пусто"));
    } else if t.contains(['\n', '\r'])
        || t.trim_end_matches(';').contains(';')
        || t.contains("//")
        || t.contains("/*")
    {
        errors.push(format!(
            "{what}: одно выражение в одну строку, без комментариев"
        ));
    }
}

/// `read();` becomes `read()`: the user may type the semicolon out of habit.
fn call_text(text: &str) -> String {
    text.trim().trim_end_matches(';').trim_end().to_owned()
}

impl LibraryWizard {
    /// Problems found, in Russian; empty when `generate` can be used.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.lib.trim().is_empty() {
            errors.push("Библиотека: имя в менеджере библиотек Arduino не задано".to_owned());
        } else if self.lib.contains(['"', '\n', '\r']) {
            errors.push("Библиотека: в имени нельзя кавычки и переносы строк".to_owned());
        }
        let h = self.header.trim();
        if h.is_empty() {
            errors.push("Библиотека: не задан заголовочный файл (например Имя.h)".to_owned());
        } else if h.contains(['<', '>', '"', '\n', '\r', ' ']) {
            errors.push("Библиотека: файл — просто имя вроде Имя.h, без <> и пробелов".to_owned());
        }
        if !is_class_name(self.class.trim()) {
            errors.push(format!(
                "Объект: класс «{}» — латинские буквы, цифры, _ и :: для пространства имён",
                self.class.trim()
            ));
        }
        // Ports and parameters of the block share one namespace (fields of C++ structs).
        let mut seen: Vec<String> = Vec::new();
        let mut name = |what: &str, n: &str, errors: &mut Vec<String>| {
            if n.is_empty() {
                errors.push(format!("{what}: имя не задано"));
            } else if !is_cpp_ident(n) {
                errors.push(format!(
                    "{what} «{n}»: имя только из латинских букв, цифр и _, не с цифры"
                ));
            } else if seen.iter().any(|s| s == n) {
                errors.push(format!("{what} «{n}»: такое имя уже есть в блоке"));
            }
            seen.push(n.to_owned());
        };
        for p in &self.params {
            name("Параметр", p.name.trim(), &mut errors);
        }
        for o in &self.outputs {
            name("Выход", o.name.trim(), &mut errors);
        }
        let mut inputs: Vec<&str> = Vec::new();
        for a in &self.actions {
            let n = a.input.trim();
            // One input can drive several calls: it is made once.
            if !inputs.contains(&n) {
                name("Вход", n, &mut errors);
                inputs.push(n);
            }
        }
        for (i, a) in self.args.iter().enumerate() {
            match a {
                WizArg::Param(n) if !self.params.iter().any(|p| p.name.trim() == n.trim()) => {
                    errors.push(format!(
                        "Аргумент {}: параметра «{}» нет, создайте его на шаге «Объект»",
                        i + 1,
                        n.trim()
                    ))
                }
                WizArg::Const(t) => check_expr(&format!("Аргумент {}", i + 1), t, &mut errors),
                _ => {}
            }
        }
        for (i, c) in self.init.iter().enumerate() {
            check_expr(&format!("Запуск, вызов {}", i + 1), c, &mut errors);
        }
        for (i, o) in self.outputs.iter().enumerate() {
            let what = format!("Выход «{}»", o.name.trim());
            check_expr(&format!("{what}, выражение"), &o.expr, &mut errors);
            let when = o.when.trim();
            if !when.is_empty() {
                check_expr(&format!("{what}, условие"), when, &mut errors);
                if self.outputs[..i]
                    .iter()
                    .any(|e| e.name.trim() == when && e.ty != WizType::Bool)
                {
                    errors.push(format!("{what}, условие: выход «{when}» не bool"));
                }
            }
            if o.ty == WizType::Pin {
                errors.push(format!("{what}: тип «пин» бывает только у параметров"));
            }
        }
        for a in &self.actions {
            check_expr(
                &format!("Действие входа «{}»", a.input.trim()),
                &a.call,
                &mut errors,
            );
        }
        errors
    }

    /// Text of `native.cpp`. Call [`validate`](Self::validate) first.
    pub fn generate(&self) -> String {
        let mut inputs: Vec<&str> = Vec::new();
        for a in &self.actions {
            if !inputs.contains(&a.input.trim()) {
                inputs.push(a.input.trim());
            }
        }
        let mut t = format!("// crab:native libs=\"{}\"\n", self.lib.trim());
        t.push_str("// crab:sim");
        for o in &self.outputs {
            let zero = if o.ty == WizType::Bool { "false" } else { "0" };
            t.push_str(&format!(" {}={zero}", o.name.trim()));
        }
        t.push('\n');
        t.push_str(MARKER);
        t.push_str(&serde_json::to_string(self).expect("the wizard serialises"));
        t.push_str(&format!("\n#include <{}>\n\n", self.header.trim()));
        t.push_str("// Per block: the library object and the previous values of edge inputs.\n");
        t.push_str(&format!("struct State {{\n  {}* obj;\n", self.class.trim()));
        for i in &inputs {
            t.push_str(&format!("  bool prev_{i};\n"));
        }
        t.push_str("};\n\n");
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| match a {
                WizArg::Param(n) => format!("p.{}", n.trim()),
                WizArg::Const(c) => c.trim().to_owned(),
            })
            .collect();
        t.push_str("// Runs once and again when a parameter changes.\n");
        t.push_str("CRAB_INIT(State& s, Params p) {\n  delete s.obj;\n");
        t.push_str(&format!(
            "  s.obj = new {}({});\n",
            self.class.trim(),
            args.join(", ")
        ));
        for i in &inputs {
            t.push_str(&format!("  s.prev_{i} = false;\n"));
        }
        for c in &self.init {
            t.push_str(&format!("  s.obj->{};\n", call_text(c)));
        }
        t.push_str("}\n\n// Every cycle.\nCRAB_STEP(State& s, In in, Out& out) {\n");
        for a in &self.actions {
            let i = a.input.trim();
            t.push_str(&format!(
                "  if (in.{i} && !s.prev_{i}) s.obj->{};\n",
                call_text(&a.call)
            ));
        }
        for i in &inputs {
            t.push_str(&format!("  s.prev_{i} = in.{i};\n"));
        }
        for (n, o) in self.outputs.iter().enumerate() {
            let name = o.name.trim();
            let set = format!(
                "out.{name} = ({})s.obj->{};",
                o.ty.cpp(),
                call_text(&o.expr)
            );
            let when = o.when.trim();
            if when.is_empty() {
                t.push_str(&format!("  {set}\n"));
            } else if self.outputs[..n]
                .iter()
                .any(|e| e.name.trim() == when && e.ty == WizType::Bool)
            {
                t.push_str(&format!("  if (out.{when}) {set}\n"));
            } else {
                t.push_str(&format!("  if (s.obj->{}) {set}\n", call_text(when)));
            }
        }
        t.push_str("}\n");
        t
    }

    /// The fields stored in a file made by [`generate`](Self::generate); `None` for any other
    /// text or when the code below the comment was changed by hand.
    pub fn parse(source: &str) -> Option<Self> {
        let json = source.lines().find_map(|l| l.strip_prefix(MARKER))?.trim();
        let wizard: Self = serde_json::from_str(json).ok()?;
        (wizard.validate().is_empty() && wizard.generate() == source).then_some(wizard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hx711() -> LibraryWizard {
        LibraryWizard {
            lib: "GyverHX711".into(),
            header: "GyverHX711.h".into(),
            class: "GyverHX711".into(),
            params: vec![
                WizParam {
                    name: "data".into(),
                    ty: WizType::Pin,
                },
                WizParam {
                    name: "clock".into(),
                    ty: WizType::Pin,
                },
            ],
            args: vec![
                WizArg::Param("data".into()),
                WizArg::Param("clock".into()),
                WizArg::Const("HX_GAIN128_A".into()),
            ],
            init: vec![],
            outputs: vec![
                WizOutput {
                    name: "ready".into(),
                    ty: WizType::Bool,
                    expr: "available()".into(),
                    when: String::new(),
                },
                WizOutput {
                    name: "value".into(),
                    ty: WizType::Int,
                    expr: "read();".into(),
                    when: "ready".into(),
                },
            ],
            actions: vec![WizAction {
                input: "tare".into(),
                call: "tare()".into(),
            }],
        }
    }

    #[test]
    fn hx711_generates_the_expected_code() {
        let w = hx711();
        assert_eq!(w.validate(), Vec::<String>::new());
        let code = w.generate();
        for line in [
            "// crab:native libs=\"GyverHX711\"",
            "// crab:sim ready=false value=0",
            "#include <GyverHX711.h>",
            "  GyverHX711* obj;",
            "  bool prev_tare;",
            "  s.obj = new GyverHX711(p.data, p.clock, HX_GAIN128_A);",
            "  if (in.tare && !s.prev_tare) s.obj->tare();",
            "  s.prev_tare = in.tare;",
            "  out.ready = (bool)s.obj->available();",
            "  if (out.ready) out.value = (int32_t)s.obj->read();",
        ] {
            assert!(code.contains(line), "{line}\n{code}");
        }
    }

    #[test]
    fn the_fields_come_back_from_the_file() {
        let w = hx711();
        assert_eq!(LibraryWizard::parse(&w.generate()), Some(w.clone()));
        // Hand-edited code is no longer the wizard's.
        let edited = w.generate().replace("tare()", "tare(); // mine");
        assert_eq!(LibraryWizard::parse(&edited), None);
        assert_eq!(
            LibraryWizard::parse("CRAB_STEP(State& s, In in, Out& out) {}"),
            None
        );
    }

    #[test]
    fn a_condition_can_be_a_call_of_the_object() {
        let mut w = hx711();
        w.outputs[1].when = "available()".into();
        assert!(w.generate().contains("if (s.obj->available()) out.value"));
    }

    #[test]
    fn mistakes_are_explained_in_russian() {
        let mut w = hx711();
        w.lib = String::new();
        w.header = "<a.h>".into();
        w.class = "1bad".into();
        w.params[1].name = "data".into();
        w.args[0] = WizArg::Param("nope".into());
        w.args[2] = WizArg::Const("a // b".into());
        w.outputs[1].name = "значение".into();
        w.outputs[1].expr = String::new();
        w.actions[0].call = "tare() // x".into();
        let text = w.validate().join("\n");
        for part in [
            "Библиотека: имя в менеджере",
            "файл — просто имя",
            "Объект: класс «1bad»",
            "Параметр «data»: такое имя уже есть",
            "параметра «nope» нет",
            "Аргумент 3: одно выражение",
            "Выход «значение»: имя только из латинских",
            "выражение: пусто",
            "Действие входа «tare»: одно выражение",
        ] {
            assert!(text.contains(part), "{part}\n{text}");
        }
    }

    #[test]
    fn the_result_is_a_valid_native_part() {
        let w = hx711();
        let def: crate::BlockDef = toml::from_str(
            "[block]\ntype = \"hx\"\nname = \"HX\"\ngroup = \"g\"\n\
             [[input]]\nname = \"tare\"\ntype = \"bool\"\n\
             [[output]]\nname = \"ready\"\ntype = \"bool\"\n\
             [[output]]\nname = \"value\"\ntype = \"int\"\n\
             [[param]]\nname = \"data\"\ntype = \"pin\"\n\
             [[param]]\nname = \"clock\"\ntype = \"pin\"\n",
        )
        .unwrap();
        let part = crate::native::parse_native(&w.generate(), &def)
            .unwrap_or_else(|e| panic!("{}", crate::native::join_errors(&e)));
        assert_eq!(part.libs, ["GyverHX711"]);
        assert!(part.has_init && part.has_state);
    }
}
