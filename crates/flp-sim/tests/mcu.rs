//! ABI v2: tags, bus, permissions and the version check, on WAT modules.

use flp_sim::mcu::{Manifest, McuModule};

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
            (import "flp" "tag_get" (func $get (param i32) (result f64)))
            (import "flp" "tag_set" (func $set (param i32 f64)))
            (func (export "abi_version") (result i32) i32.const 2)
            (func (export "step") (param i32)
                (call $set (i32.const 2) (f64.mul (call $get (i32.const 1)) (f64.const 2)))))"#,
        &[],
    )
    .unwrap();
    m.set_tag(1, 21.0);
    m.step(10).unwrap();
    assert_eq!(m.tag(2), Some(42.0));
}

const BUS: &str = r#"(module
    (import "flp" "publish" (func $publish (param i32 i32 i32 i32)))
    (import "flp" "subscribe" (func $subscribe (param i32 i32)))
    (import "flp" "tag_set" (func $set (param i32 f64)))
    (memory (export "memory") 1)
    (data (i32.const 0) "ping")
    (data (i32.const 4) "pong")
    (global $heap (mut i32) (i32.const 1024))
    (func (export "abi_version") (result i32) i32.const 2)
    (func (export "init") (call $subscribe (i32.const 0) (i32.const 4)))
    (func (export "alloc") (param $n i32) (result i32)
        (global.get $heap)
        (global.set $heap (i32.add (global.get $heap) (local.get $n))))
    ;; reply "pong" with the first payload byte as the payload
    (func (export "on_message") (param i32 i32 i32 i32)
        (call $set (i32.const 0) (f64.convert_i32_u (i32.load8_u (local.get 2))))
        (call $publish (i32.const 4) (i32.const 4) (local.get 2) (local.get 3)))
    (func (export "step") (param i32)))"#;

#[test]
fn bus_delivers_to_subscribers_only() {
    let mut m = load(BUS, &[]).unwrap();
    assert_eq!(m.subscriptions(), ["ping".to_owned()]);
    assert!(!m.deliver("other", b"x").unwrap());
    assert!(m.deliver("ping", &[7, 8]).unwrap());
    assert_eq!(m.tag(0), Some(7.0));
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
        r#"(module (import "flp" "reboot" (func)) (func (export "abi_version") (result i32) i32.const 2)
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
    assert!(Manifest::parse("abi = 3").unwrap_err().contains("ABI 3"));
}
