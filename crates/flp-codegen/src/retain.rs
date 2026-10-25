//! Variables kept in flash (`retain`): restored in `setup()`, saved from `loop()`.
//!
//! Flash wears out on writes, so a value is written only when it differs from
//! the last saved copy, and the check runs at most once per [`SAVE_PERIOD_MS`].

use std::fmt::Write;

use flp_core::vars::storage_name;
use flp_core::{SignalType, Var};

/// NVS namespace of the variables; blocks use their own ones.
const NAMESPACE: &str = "flp_vars";
const SAVE_PERIOD_MS: u32 = 1000;

/// NVS key of a variable: at most 15 characters, and changes with the type so
/// a value saved under another type is not read back.
fn key(var: &Var) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for b in format!("{}:{}", var.name, var.ty.cpp()).bytes() {
        hash = (hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    format!("v{hash:08x}")
}

fn saved_name(var: &Var) -> String {
    format!("{}_saved", storage_name(&var.name))
}

fn retained(vars: &[Var]) -> impl Iterator<Item = &Var> {
    vars.iter().filter(|v| v.retain)
}

/// `String` keeps its text on the heap, so it is stored as a string, not as bytes.
fn is_text(var: &Var) -> bool {
    var.ty.signal() == SignalType::String
}

/// Global declarations, empty without retained variables.
pub fn globals(vars: &[Var]) -> String {
    let mut out = String::new();
    if retained(vars).next().is_none() {
        return out;
    }
    out.push_str("\n// Сохранение переменных во флеш\n");
    out.push_str("Preferences flp_retain;\nuint32_t flp_retain_ms = 0;\n");
    for v in retained(vars) {
        let _ = writeln!(
            out,
            "{} {};  // последнее сохранённое «{}»",
            v.ty.cpp(),
            saved_name(v),
            v.name
        );
    }
    out
}

/// Lines of `setup()` restoring the values.
pub fn setup(vars: &[Var]) -> Vec<String> {
    if retained(vars).next().is_none() {
        return Vec::new();
    }
    let mut lines = vec![
        "// Восстановление переменных из флеша".to_owned(),
        format!("flp_retain.begin(\"{NAMESPACE}\", false);"),
    ];
    for v in retained(vars) {
        let (var, key) = (storage_name(&v.name), key(v));
        if is_text(v) {
            lines.push(format!(
                "if (flp_retain.isKey(\"{key}\")) {{  // {}",
                v.name
            ));
            lines.push(format!("  {var} = flp_retain.getString(\"{key}\");"));
            lines.push("}".to_owned());
            lines.push(format!("{} = {var};", saved_name(v)));
            continue;
        }
        lines.push(format!(
            "if (flp_retain.getBytesLength(\"{key}\") == sizeof({var})) {{  // {}",
            v.name
        ));
        lines.push(format!(
            "  flp_retain.getBytes(\"{key}\", &{var}, sizeof({var}));"
        ));
        lines.push("}".to_owned());
        lines.push(format!("{} = {var};", saved_name(v)));
    }
    lines
}

/// Lines of `loop()` saving changed values.
pub fn save(vars: &[Var]) -> Vec<String> {
    if retained(vars).next().is_none() {
        return Vec::new();
    }
    let mut lines = vec![
        "// Сохранение переменных во флеш: только изменившиеся значения".to_owned(),
        format!("if (millis() - flp_retain_ms >= {SAVE_PERIOD_MS}) {{"),
        "  flp_retain_ms = millis();".to_owned(),
    ];
    for v in retained(vars) {
        let (var, saved) = (storage_name(&v.name), saved_name(v));
        if is_text(v) {
            lines.push(format!("  if ({var} != {saved}) {{  // {}", v.name));
            lines.push(format!("    {saved} = {var};"));
            lines.push(format!("    flp_retain.putString(\"{}\", {var});", key(v)));
            lines.push("  }".to_owned());
            continue;
        }
        lines.push(format!(
            "  if (memcmp(&{var}, &{saved}, sizeof({var})) != 0) {{  // {}",
            v.name
        ));
        lines.push(format!("    {saved} = {var};"));
        lines.push(format!(
            "    flp_retain.putBytes(\"{}\", &{var}, sizeof({var}));",
            key(v)
        ));
        lines.push("  }".to_owned());
    }
    lines.push("}".to_owned());
    lines
}
