//! ABI v3: tags, bus, permissions and the version check, on WAT modules.

use flp_sim::mcu::{Manifest, McuModule, Word};

fn load(src: &str, allow: &[&str]) -> Result<McuModule, String> {
    let manifest = Manifest {
        allow: allow.iter().map(|s| s.to_string()).collect(),
        ..Manifest::parse("").unwrap()
    };
    McuModule::load(&wat::parse_str(src).unwrap(), &manifest, 1)
}

/// The blink module of the board prototype (`specs/wasm-on-mcu/proto/blink.wat`).
const BLINK: &str = include_str!("../../../specs/wasm-on-mcu/proto/blink.wat");

#[test]
fn blink_toggles_pin_every_500_ms() {
    let mut m = load(BLINK, &["gpio_write", "millis"]).unwrap();
    let mut seen = Vec::new();
    for _ in 0..30 {
        m.step(100).unwrap();
        seen.push(m.io().gpio_out.get(&2).copied());
    }
    // Steps at 100, 200, ..., 3000 ms: the pin flips at 500, 1000, ...
    assert_eq!(seen[3], None);
    assert_eq!(seen[4], Some(1));
    assert_eq!(seen[9], Some(0));
    assert_eq!(seen[14], Some(1));
}

#[test]
fn tags_are_shared_with_the_host() {
    let mut m = load(
        r#"(module
            (import "flp" "tag_get_f" (func $get (param i32) (result f32)))
            (import "flp" "tag_set_f" (func $set (param i32 f32)))
            (func (export "abi_version") (result i32) i32.const 3)
            (func (export "step") (param i32)
                (call $set (i32.const 2) (f32.mul (call $get (i32.const 1)) (f32.const 2)))))"#,
        &[],
    )
    .unwrap();
    m.set_tag(1, Word::float(21.0));
    m.step(10).unwrap();
    assert_eq!(m.tag(2), Some(Word::float(42.0)));
}

const BUS: &str = r#"(module
    (import "flp" "publish" (func $publish (param i32 i32 i32 i32)))
    (import "flp" "subscribe" (func $subscribe (param i32 i32)))
    (import "flp" "tag_set_i" (func $set (param i32 i32)))
    (memory (export "memory") 1)
    (data (i32.const 0) "ping")
    (data (i32.const 4) "pong")
    (global $heap (mut i32) (i32.const 1024))
    (func (export "abi_version") (result i32) i32.const 3)
    (func (export "init") (call $subscribe (i32.const 0) (i32.const 4)))
    (func (export "alloc") (param $n i32) (result i32)
        (global.get $heap)
        (global.set $heap (i32.add (global.get $heap) (local.get $n))))
    ;; reply "pong" with the first payload byte as the payload
    (func (export "on_message") (param i32 i32 i32 i32)
        (call $set (i32.const 0) (i32.load8_u (local.get 2)))
        (call $publish (i32.const 4) (i32.const 4) (local.get 2) (local.get 3)))
    (func (export "step") (param i32)))"#;

#[test]
fn bus_delivers_to_subscribers_only() {
    let mut m = load(BUS, &[]).unwrap();
    assert_eq!(m.subscriptions(), ["ping".to_owned()]);
    assert!(!m.deliver("other", b"x").unwrap());
    assert!(m.deliver("ping", &[7, 8]).unwrap());
    assert_eq!(m.tag(0), Some(Word::int(7)));
    assert_eq!(m.take_published(), vec![("pong".to_owned(), vec![7, 8])]);
    assert!(m.take_published().is_empty());
}

#[test]
fn hardware_import_needs_permission() {
    let err = load(BLINK, &["millis"]).err().unwrap();
    assert!(err.contains("gpio_write") && err.contains("allow"), "{err}");
    assert!(load(BLINK, &["gpio_write", "millis"]).is_ok());
}

#[test]
fn unknown_import_is_named() {
    let err = load(
        r#"(module (import "flp" "reboot" (func)) (func (export "abi_version") (result i32) i32.const 3)
            (func (export "step") (param i32)))"#,
        &[],
    )
    .err()
    .unwrap();
    assert!(err.contains("неизвестный импорт flp.reboot"), "{err}");
}

#[test]
fn wrong_or_missing_abi_version_is_rejected() {
    let err = load(
        r#"(module (func (export "abi_version") (result i32) i32.const 1)
            (func (export "step") (param i32)))"#,
        &[],
    )
    .err()
    .unwrap();
    assert!(err.contains("ABI 1"), "{err}");
    let err = load(r#"(module (func (export "step") (param i32)))"#, &[])
        .err()
        .unwrap();
    assert!(err.contains("abi_version"), "{err}");
}

#[test]
fn manifest_is_validated() {
    let m = Manifest::parse("name = \"blink\"\nallow = [\"gpio_write\"]").unwrap();
    assert_eq!(m.allow, ["gpio_write"]);
    assert!(Manifest::parse("allow = [\"format_disk\"]")
        .unwrap_err()
        .contains("format_disk"));
    assert!(Manifest::parse("abi = 2").unwrap_err().contains("ABI 2"));
    assert_eq!(Manifest::parse("memory_pages = 2").unwrap().memory_pages, 2);
}

/// The module and script shared with the board comparison (`proto/compare/compare.ps1`).
const COUNTER: &str = include_str!("../../../specs/wasm-on-mcu/proto/counter.wat");
const SCENARIO: &str = include_str!("../../../specs/wasm-on-mcu/proto/compare/scenario.txt");
const GOLDEN_PATH: &str = "../../specs/wasm-on-mcu/proto/compare/golden.txt";

/// A scenario value: `i` is an `i32`, `f` an `f32`.
fn word(kind: &str, text: &str) -> Word {
    match kind {
        "i" => Word::int(text.parse().unwrap()),
        "f" => Word::float(text.parse().unwrap()),
        other => panic!("unknown value type in scenario: {other}"),
    }
}

/// Runs the scenario the way the master does: a step, then the messages it published go back
/// to the subscribers. Returns one `out …` line per `out` command.
fn run_scenario() -> Vec<String> {
    let manifest = Manifest::parse("").unwrap();
    let fresh = || McuModule::load(&wat::parse_str(COUNTER).unwrap(), &manifest, 4).unwrap();
    let mut m = fresh();
    let mut inputs = vec![Word::default(); 8];
    let mut params = vec![Word::default(); 8];
    let mut lines = Vec::new();
    for line in SCENARIO.lines().map(str::trim) {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [] | ["#", ..] => {}
            ["reset"] => {
                m = fresh();
                inputs = vec![Word::default(); 8];
                params = vec![Word::default(); 8];
            }
            ["in", i, t, v] => inputs[i.parse::<usize>().unwrap()] = word(t, v),
            ["param", i, t, v] => params[i.parse::<usize>().unwrap()] = word(t, v),
            ["tick", n, dt] => {
                for _ in 0..n.parse::<u32>().unwrap() {
                    m.set_inputs(&inputs);
                    m.set_params(&params);
                    m.step(dt.parse().unwrap()).unwrap();
                    for (topic, data) in m.take_published() {
                        m.deliver(&topic, &data).unwrap();
                    }
                }
            }
            ["out"] => {
                let bits: Vec<String> = m.outputs()[..4]
                    .iter()
                    .map(|v| format!("{:08x}", v.0))
                    .collect();
                lines.push(format!("out {}", bits.join(" ")));
            }
            other => panic!("unknown scenario line: {other:?}"),
        }
    }
    lines
}

#[test]
fn counter_matches_golden_outputs() {
    let got = run_scenario().join("\n") + "\n";
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(GOLDEN_PATH, &got).unwrap();
    }
    let want = std::fs::read_to_string(GOLDEN_PATH)
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(
        got, want,
        "run with UPDATE_GOLDEN=1 to refresh the golden file"
    );
}

#[test]
fn counter_scenario_is_sane() {
    let lines = run_scenario();
    let last: Vec<u32> = lines
        .last()
        .unwrap()
        .split(' ')
        .skip(1)
        .map(|w| u32::from_str_radix(w, 16).unwrap())
        .collect();
    // 53 steps added to in2 = i32::MAX wrap around, as the model says.
    assert_eq!(last[1] as i32, 53i32.wrapping_add(i32::MAX));
    assert_eq!(last[3], 26); // a message after every even step
}

/// `EXPORT_WASM=<dir> cargo test -p flp-sim --test mcu -- --ignored export_prototypes`
/// writes the prototype modules as `.wasm` for the board (`compare.ps1`, `test.ps1`).
#[test]
#[ignore = "writes files; run by hand"]
fn export_prototypes() {
    let dir = std::path::PathBuf::from(std::env::var("EXPORT_WASM").expect("set EXPORT_WASM"));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, src) in [
        ("counter", COUNTER),
        (
            "blink",
            include_str!("../../../specs/wasm-on-mcu/proto/blink.wat"),
        ),
        (
            "hang",
            include_str!("../../../specs/wasm-on-mcu/proto/hang.wat"),
        ),
    ] {
        std::fs::write(
            dir.join(format!("{name}.wasm")),
            wat::parse_str(src).unwrap(),
        )
        .unwrap();
    }
}
