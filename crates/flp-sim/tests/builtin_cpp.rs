//! The C++ a built-in block writes for the firmware must give the same outputs as its Rust
//! `Behavior` in the simulation. Needs a real compiler:
//! `FLP_ZIG=<path to zig> cargo test -p flp-sim --test builtin_cpp`.

use std::path::{Path, PathBuf};

use flp_core::value::parse_duration_ms;
use flp_core::{BlockDef, ParamKind, ParamValue, SignalType};
use flp_sim::builtin::Ctx;
use flp_sim::SimModule;

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

/// One run: the block with some parameters replaced; `live` limits the existing inputs
/// (variable-count blocks).
struct Scenario {
    kind: &'static str,
    params: Vec<(&'static str, ParamValue)>,
    live: Option<usize>,
}

fn scenario(kind: &'static str) -> Scenario {
    Scenario {
        kind,
        params: vec![],
        live: None,
    }
}

/// Steps of 10 ms; bool inputs flip now and then, numbers jump around a small range.
fn compare(zig: &Path, sc: &Scenario) {
    let kind = sc.kind;
    let dir = repo("library").join(kind);
    let mut def: BlockDef =
        toml::from_str(&std::fs::read_to_string(dir.join("block.toml")).unwrap()).unwrap();
    for (name, value) in &sc.params {
        let p = def.params.iter_mut().find(|p| p.name == *name).unwrap();
        p.default = Some(value.clone());
    }
    let block = flp_blocks::get(kind).unwrap_or_else(|| panic!("{kind}: no Rust code"));
    let source = flp_codegen::sim_source_builtin(&def, block).unwrap();

    let tag = format!("{kind}-{}", sc.params.len());
    let out = std::env::temp_dir().join(format!("flp-blocks-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    flp_build::wasm::build_module(zig, &out, &source).unwrap_or_else(|e| panic!("{kind}: {e}"));
    let wasm = std::fs::read(out.join("block.wasm")).unwrap();

    let mut module = SimModule::load(&wasm, def.outputs.len()).unwrap();
    let mut expected = block.behavior().expect("behaviour");
    let params: Vec<f64> = def
        .params
        .iter()
        .map(|p| p.default.as_ref().map_or(0.0, |v| number(p.kind, v)))
        .collect();
    // Variable-count blocks show only the ports the `count` parameter asks for.
    let live = sc.live.or_else(|| {
        let p = def.params.iter().find(|p| p.name == "count")?;
        let n = number(p.kind, p.default.as_ref()?) as usize;
        Some(if sc.kind == "mux" { n + 1 } else { n })
    });
    let active: Vec<bool> = (0..def.inputs.len())
        .map(|i| live.is_none_or(|n| i < n))
        .collect();
    let mut inputs = vec![0.0; def.inputs.len()];
    let mut outputs = vec![0.0; def.outputs.len()];
    let mut seed = 12345_u32;
    let mut now = 0_u64;
    for step in 0..3000 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        if !inputs.is_empty() && (seed >> 16).is_multiple_of(40) {
            let i = ((seed >> 8) as usize) % inputs.len();
            if !active[i] {
                continue;
            }
            inputs[i] = match def.inputs[i].ty {
                SignalType::Bool => 1.0 - inputs[i],
                // Eighths are exact in 32 bits, so rounding modes get real fractions.
                SignalType::Float => f64::from(((seed >> 4) % 1601) as i32 - 800) / 8.0,
                _ => f64::from(((seed >> 4) % 201) as i32 - 100),
            };
        }
        now += 10;
        expected.step(
            &Ctx {
                now_ms: now,
                inputs: &inputs,
                active: &active,
                params: &params,
                texts: &[],
                param_texts: &[],
                bench: &flp_blocks::NO_BENCH,
            },
            &mut outputs,
        );
        let got = module.step(10, &inputs, &params).unwrap();
        for (i, port) in def.outputs.iter().enumerate() {
            // The firmware computes floats in 32 bits, the simulation in 64.
            let same = if port.ty == SignalType::Float {
                (got[i].is_nan() && outputs[i].is_nan())
                    || got[i] == outputs[i]
                    || (got[i] - outputs[i]).abs() <= 1e-4 * outputs[i].abs().max(1.0)
            } else {
                got[i] == outputs[i]
            };
            assert!(
                same,
                "{kind}: step {step}, inputs {inputs:?}, {}: firmware {}, simulation {}",
                port.name, got[i], outputs[i]
            );
        }
    }
}

#[test]
fn builtin_cpp_matches_the_behaviour() {
    let Some(zig) = std::env::var_os("FLP_ZIG") else {
        eprintln!("FLP_ZIG is not set: skipped");
        return;
    };
    let zig = Path::new(&zig);
    let mut list: Vec<Scenario> = [
        "add",
        "sub",
        "mul",
        "div",
        "mod",
        "min",
        "max",
        "not",
        "and",
        "or",
        "xor",
        "rs_trigger",
        "sr_trigger",
        "t_trigger",
        "rising_edge",
        "falling_edge",
        "timer_on",
        "timer_off",
        "pulse",
        "generator",
        "debounce",
        "counter",
        "hysteresis",
        "compare",
        "greater",
        "less",
        "limit",
        "scale",
        "switch",
        "mux",
        "constant",
        "abs",
        "sq",
        "sqrt",
        "pow",
        "sin",
        "cos",
        "tan",
        "float_to_int",
        "int_to_float",
        "random",
        "bit_and",
        "bit_or",
        "bit_xor",
        "bit_not",
        "bit_read",
        "bit_write",
        "bit_change",
        "bits_to_byte",
        "byte_to_bits",
        "bytes_to_word",
        "word_to_bytes",
        "bytes_to_dword",
        "dword_to_bytes",
        "number_change",
        "pid",
    ]
    .into_iter()
    .map(scenario)
    .collect();
    let with = |kind, params: Vec<(&'static str, ParamValue)>| Scenario {
        params,
        ..scenario(kind)
    };
    let ms = |v: &str| ParamValue::Text(v.into());
    list.extend([
        with("timer_on", vec![("time", ms("250ms"))]),
        with("timer_off", vec![("time", ms("250ms"))]),
        with("pulse", vec![("time", ms("250ms"))]),
        with("debounce", vec![("time", ms("50ms"))]),
        with(
            "generator",
            vec![("time_on", ms("70ms")), ("time_off", ms("130ms"))],
        ),
        // Thresholds in the other order.
        with(
            "hysteresis",
            vec![
                ("on", ParamValue::Float(-10.0)),
                ("off", ParamValue::Float(10.0)),
            ],
        ),
        with("scale", vec![("clamp", ParamValue::Bool(false))]),
        with(
            "scale",
            vec![
                ("out_min", ParamValue::Float(100.0)),
                ("out_max", ParamValue::Float(-100.0)),
            ],
        ),
    ]);
    for kind in ["sin", "cos", "tan"] {
        list.push(with(kind, vec![("unit", ParamValue::Int(1))]));
    }
    for mode in 1..4 {
        list.push(with("float_to_int", vec![("mode", ParamValue::Int(mode))]));
    }
    for bit in [0, 3, 31] {
        for kind in ["bit_read", "bit_write", "bit_change"] {
            list.push(with(kind, vec![("bit", ParamValue::Int(bit))]));
        }
    }
    list.push(with(
        "random",
        vec![
            ("min", ParamValue::Float(-5.0)),
            ("max", ParamValue::Float(7.5)),
        ],
    ));
    list.push(with(
        "number_change",
        vec![("deadband", ParamValue::Float(2.5))],
    ));
    list.push(with(
        "pid",
        vec![
            ("kp", ParamValue::Float(2.0)),
            ("ki", ParamValue::Float(0.5)),
            ("kd", ParamValue::Float(0.1)),
            ("out_min", ParamValue::Float(-50.0)),
            ("out_max", ParamValue::Float(50.0)),
        ],
    ));
    for op in 0..6 {
        list.push(with("compare", vec![("op", ParamValue::Int(op))]));
    }
    for n in [2, 3, 16] {
        for kind in ["and", "or", "xor"] {
            list.push(Scenario {
                params: vec![("count", ParamValue::Int(n as i64))],
                live: Some(n),
                ..scenario(kind)
            });
        }
    }
    for n in [2, 3, 5] {
        list.push(Scenario {
            params: vec![("count", ParamValue::Int(n as i64))],
            live: Some(n + 1),
            ..scenario("mux")
        });
    }
    for n in [2, 5, 16] {
        for kind in ["dms", "decoder", "encoder"] {
            list.push(with(kind, vec![("count", ParamValue::Int(n))]));
        }
    }
    for sc in &list {
        compare(zig, sc);
    }
}
