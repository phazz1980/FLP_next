//! A simulation module built from the C++ of a built-in text block must behave like the built-in
//! implementation of the same block (blocks written in Rust are checked in `builtin_cpp`). Needs a real compiler:
//! `CRAB_ZIG=<path to zig> cargo test -p crab-sim --test template`.

use std::path::{Path, PathBuf};

use crab_core::value::parse_duration_ms;
use crab_core::{BlockDef, ParamKind, ParamValue};
use crab_sim::builtin::{behavior, Ctx};
use crab_sim::SimModule;

fn repo(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn number(kind: ParamKind, value: &ParamValue) -> f64 {
    match value {
        ParamValue::Bool(b) => f64::from(u8::from(*b)),
        ParamValue::Int(v) => *v as f64,
        ParamValue::Float(v) => *v,
        ParamValue::Text(s) if kind == ParamKind::Duration => {
            parse_duration_ms(s).unwrap_or(0) as f64
        }
        ParamValue::Text(_) => 0.0,
    }
}

/// Text blocks: the module built from the C++ gives the same texts and numbers as
/// the built-in implementation, for a set of input texts.
fn compare_text(zig: &Path, kind: &str, samples: &[&[&str]]) {
    let dir = repo("library").join(kind);
    let def: BlockDef =
        toml::from_str(&std::fs::read_to_string(dir.join("block.toml")).unwrap()).unwrap();
    let block = crab_blocks::get(kind).unwrap_or_else(|| panic!("{kind}: no built-in block"));
    let source =
        crab_codegen::sim_source_builtin(&def, block).unwrap_or_else(|e| panic!("{kind}: {e}"));
    let out = std::env::temp_dir().join(format!("crab-sim-text-{kind}-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    crab_build::wasm::build_module(zig, &out, &source).unwrap_or_else(|e| panic!("{kind}: {e}"));
    let wasm = std::fs::read(out.join("block.wasm")).unwrap();

    let mut module = SimModule::load(&wasm, def.outputs.len()).unwrap();
    let mut expected = behavior(kind).expect("built-in behaviour");
    let params: Vec<f64> = def
        .params
        .iter()
        .map(|p| match p.default.as_ref() {
            Some(v) => number(p.kind, v),
            None => 0.0,
        })
        .collect();
    let param_texts: Vec<String> = def
        .params
        .iter()
        .map(|p| match p.default.as_ref() {
            Some(ParamValue::Text(s)) if p.kind == ParamKind::Text => s.clone(),
            _ => String::new(),
        })
        .collect();
    let active = vec![true; def.inputs.len()];
    let mut num = vec![0.0; def.outputs.len()];
    let mut txt = vec![String::new(); def.outputs.len()];
    for (n, sample) in samples.iter().enumerate() {
        // Each sample gives the texts of the string inputs and the numbers of the others.
        let mut inputs = vec![0.0; def.inputs.len()];
        let mut texts = vec![String::new(); def.inputs.len()];
        for (i, port) in def.inputs.iter().enumerate() {
            let raw = sample.get(i).copied().unwrap_or("");
            if port.ty == crab_core::SignalType::String {
                texts[i] = raw.to_owned();
            } else {
                inputs[i] = raw.parse().unwrap_or(0.0);
            }
        }
        let ctx = Ctx {
            now_ms: 10,
            inputs: &inputs,
            active: &active,
            params: &params,
            texts: &texts,
            param_texts: &param_texts,
            bench: &crab_blocks::NO_BENCH,
        };
        expected.step(&ctx, &mut num);
        expected.step_text(&ctx, &mut num, &mut txt);
        let (got, got_txt) = module
            .step_text(10, &inputs, &params, &texts, &param_texts)
            .unwrap();
        for (i, port) in def.outputs.iter().enumerate() {
            if port.ty == crab_core::SignalType::String {
                assert_eq!(
                    got_txt[i], txt[i],
                    "{kind}: sample {n} {sample:?}, {}",
                    port.name
                );
            } else {
                assert_eq!(
                    got[i], num[i],
                    "{kind}: sample {n} {sample:?}, {}",
                    port.name
                );
            }
        }
    }
}

#[test]
fn text_template_modules_match_the_built_in_blocks() {
    let Some(zig) = std::env::var_os("CRAB_ZIG") else {
        eprintln!("CRAB_ZIG is not set: skipped");
        return;
    };
    let zig = Path::new(&zig);
    compare_text(zig, "text_const", &[&[]]);
    compare_text(
        zig,
        "text_concat",
        &[&["a", "b"], &["", "x"], &["long text", ""]],
    );
    compare_text(
        zig,
        "text_length",
        &[&["hello"], &[""], &["\u{43f}\u{440}"]],
    );
    compare_text(
        zig,
        "num_to_text",
        &[&["21.456"], &["-3.5"], &["0"], &["100"]],
    );
    compare_text(zig, "text_compare", &[&["abc"], &[""], &["ON"]]);
    compare_text(
        zig,
        "text_to_num",
        &[&["3.5"], &[" 2,25 "], &["abc"], &[""], &["-7"]],
    );
    compare_text(zig, "text_substring", &[&["hello"], &[""], &["a"]]);
    compare_text(
        zig,
        "text_find",
        &[&["hello", "ll"], &["hello", "z"], &["", ""], &["abc", ""]],
    );
    compare_text(
        zig,
        "text_char",
        &[&["hello", "1"], &["hello", "9"], &["hello", "-1"]],
    );
    compare_text(zig, "text_add_char", &[&["ab", "99"], &["", "65"]]);
    compare_text(
        zig,
        "ip_to_text",
        &[&["192", "168", "1", "300"], &["0", "0", "0", "0"]],
    );
    compare_text(
        zig,
        "text_to_ip",
        &[
            &["10.0.0.7"],
            &["10.0.0"],
            &["1.2.3.256"],
            &["a.b.c.d"],
            &["1.2.3.4"],
        ],
    );
}
