//! Snapshot tests: every example in `examples/` is generated and compared with
//! `tests/snapshots/<example>.ino`. A missing snapshot is created; to accept
//! changes run `UPDATE_SNAPSHOTS=1 cargo test` and review the diff in git.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use flp_codegen::{generate, ArduinoCpp, GenError};
use flp_core::{Library, ParamValue, Project};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn library() -> Library {
    let (lib, errors) = Library::load(repo_path("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    lib
}

fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(repo_path("examples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("project.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn assert_snapshot(path: &Path, actual: &str) {
    let expected = fs::read_to_string(path)
        .ok()
        .map(|s| s.replace("\r\n", "\n"));
    if expected.as_deref() == Some(actual) {
        return;
    }
    if expected.is_none() || std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::write(path, actual).unwrap();
        eprintln!("снимок записан: {}", path.display());
        return;
    }
    panic!(
        "сгенерированный код отличается от снимка {}\n--- получено ---\n{actual}",
        path.display()
    );
}

#[test]
fn examples_match_snapshots() {
    let lib = library();
    for dir in examples() {
        let project = Project::load(&dir).unwrap();
        let code = generate(&project, &lib, &ArduinoCpp::RELEASE)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let name = dir.file_name().unwrap().to_str().unwrap();
        let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(format!("{name}.ino"));
        assert_snapshot(&snapshot, &code);
    }
}

/// Every library block must be covered by an example, and thus by a snapshot.
#[test]
fn every_block_has_an_example() {
    let lib = library();
    let used: BTreeSet<String> = examples()
        .iter()
        .flat_map(|dir| Project::load(dir).unwrap().logic.blocks)
        .map(|b| b.kind)
        .collect();
    let missing: Vec<&str> = lib
        .iter()
        .map(|b| b.def.info.kind.as_str())
        .filter(|kind| !used.contains(*kind))
        .collect();
    assert!(
        missing.is_empty(),
        "блоки без примера в examples/: {missing:?}"
    );
}

#[test]
fn schema_errors_stop_generation() {
    let mut project = Project::load(repo_path("examples/pump")).unwrap();
    project.logic.links.pop();
    match generate(&project, &library(), &ArduinoCpp::RELEASE) {
        Err(GenError::Schema(diagnostics)) => {
            assert!(diagnostics.iter().any(|d| d.message.contains("pump")));
        }
        other => panic!("ожидалась ошибка схемы, получено {other:?}"),
    }
}

#[test]
fn defaults_are_applied() {
    // `less.b` and `digital_out.inverted` are omitted and take their defaults.
    let mut project = Project::load(repo_path("examples/pump")).unwrap();
    for b in &mut project.logic.blocks {
        b.params.remove("b");
    }
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("low_out = pressure_out < 0.0f;"), "{code}");
    assert!(
        code.contains("digitalWrite(26, (delay_out) ? HIGH : LOW);"),
        "{code}"
    );
}

/// The debug build of an example with block outputs and variables.
#[test]
fn debug_module_snapshot() {
    let project = Project::load(repo_path("examples/tags")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/tags.debug.ino");
    assert_snapshot(&snapshot, &code);
    assert!(code.contains("flp_dbg_setup();"));
    assert!(code.contains("flp_dbg_loop();"));
    assert!(code.contains("flp_st_begin();"));
    assert!(code.contains("flp_st_end();"));
}

/// Strings in the debug build: their own table and `TEXT` frames.
#[test]
fn debug_strings_snapshot() {
    let project = Project::load(repo_path("examples/text_commands")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    let snapshot =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/text_commands.debug.ino");
    assert_snapshot(&snapshot, &code);
    assert!(code.contains("String* const flp_dbg_text[FLP_DBG_TEXTS] = { &p_text_out,"));
    assert!(code.contains("    flp_dbg_texts();"));
    // Without strings the debug module has no string table.
    let tags = Project::load(repo_path("examples/tags")).unwrap();
    let code = generate(&tags, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("flp_dbg_text"));
}

/// Normal firmware measures cycle time only with `[diagnostics] enabled`.
#[test]
fn diagnostics_in_normal_build() {
    let mut project = Project::load(repo_path("examples/tags")).unwrap();
    let plain = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(!plain.contains("flp_st_"), "{plain}");
    project.config.diagnostics.enabled = true;
    project.config.diagnostics.interval_ms = 2000;
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    let snapshot =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/tags.diagnostics.ino");
    assert_snapshot(&snapshot, &code);
    assert!(code.contains("Serial.begin(115200);"), "{code}");
    assert!(code.contains("flp_st_sheet(0);"), "{code}");
    assert!(code.contains("flp_st_report();"), "{code}");
    assert!(code.contains("2000UL"), "{code}");
    assert!(!code.contains("flp_dbg_"), "{code}");
}

/// Blocks on shared UARTs (`tests/fixtures/library/uart_probe`): the ports are
/// started in `setup()` from `[[uart]]`, UART0 only when nothing else starts it.
#[test]
fn shared_uart_setup() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let (lib, errors) = Library::load(fixtures.join("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let mut project = Project::load(repo_path("examples/tags")).unwrap();
    project.logic = toml::from_str(
        r#"
[[block]]
id = "a"
type = "uart_probe"

[[block]]
id = "b"
type = "uart_probe"

[[block]]
id = "usb"
type = "uart_probe"
uart = 0
"#,
    )
    .unwrap();
    project.config.uart = vec![flp_core::UartConfig {
        number: 1,
        speed: 19200,
        format: "8e1".into(),
        rx: 25,
        tx: 26,
        de: 14,
        rx_buffer: 512,
    }];
    let code = generate(&project, &lib, &ArduinoCpp::RELEASE).unwrap();
    let setup = "  // UART0 (USB): 115200 бод
  Serial.begin(115200);
  // UART1: 19200 бод, 8E1, RX GPIO25, TX GPIO26
  Serial1.setRxBufferSize(512);
  Serial1.begin(19200, SERIAL_8E1, 25, 26);
  Serial1.setPins(-1, -1, -1, 14);  // DE/RE RS-485
  Serial1.setMode(UART_MODE_RS485_HALF_DUPLEX);
";
    assert!(code.contains(setup), "{code}");
    assert_eq!(code.matches("Serial1.begin").count(), 1, "{code}");
    assert!(code.contains("Serial1.println(7);"), "{code}");
    assert!(code.contains("  // UART1, 19200"), "{code}");
    // The debug module starts UART0 itself.
    let code = generate(&project, &lib, &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("// UART0 (USB)"), "{code}");

    // A UART without settings stops generation.
    project.config.uart.clear();
    match generate(&project, &lib, &ArduinoCpp::RELEASE) {
        Err(GenError::Schema(d)) => {
            assert!(
                d.iter().any(|d| d.message.contains("UART1 не настроен")),
                "{d:?}"
            )
        }
        other => panic!("ожидалась ошибка схемы, получено {other:?}"),
    }
}

/// Modes of `uart_send` and `uart_read` that the example does not use.
#[test]
fn uart_block_modes() {
    let mut project = Project::load(repo_path("examples/text_commands")).unwrap();
    let set = |project: &mut Project, id: &str, params: &[(&str, ParamValue)]| {
        let block = project
            .logic
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .unwrap();
        for (k, v) in params {
            block.params.insert((*k).to_owned(), v.clone());
        }
    };
    use ParamValue::{Bool, Int, Text};
    set(
        &mut project,
        "report",
        &[("edge", Bool(false)), ("ending", Int(1))],
    );
    set(&mut project, "p_send", &[("mode", Int(2))]);
    set(
        &mut project,
        "rx",
        &[
            ("end", Int(2)),
            ("end_char", Text(";".into())),
            ("timeout", Text("200ms".into())),
        ],
    );
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("bool fire = tick_out;"), "{code}");
    assert!(code.contains(r#"const char* ending = "\n";"#), "{code}");
    assert!(
        code.contains("int16_t data = (int16_t)constrain(lroundf(bar_out), -32768L, 32767L);"),
        "{code}"
    );
    assert!(code.contains(r#"bool end = c == ";"[0];"#), "{code}");
    assert!(code.contains("millis() - rx_last >= 200UL"), "{code}");

    // Byte by byte: other outputs; the line outputs are gone, so the links go too.
    set(&mut project, "rx", &[("mode", Int(1))]);
    set(&mut project, "p_send", &[("mode", Int(1))]);
    project.logic.links.retain(|l| l.from.block != "rx");
    project.logic.blocks.retain(|b| b.id != "command_w");
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("rx_new = Serial2.available() > 0;"), "{code}");
    assert!(code.contains("rx_byte = Serial2.read();"), "{code}");
    assert!(!code.contains("rx_buf"), "{code}");
    assert!(
        code.contains("uint8_t data = (uint8_t)constrain(lroundf(bar_out), 0L, 255L);"),
        "{code}"
    );

    // Debug firmware starts UART0 itself; the block only writes to it.
    let project = Project::load(repo_path("examples/text_commands")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert_eq!(code.matches("Serial.begin(").count(), 1, "{code}");
}

/// `uart_read` on UART0: the port itself in normal firmware, the console input
/// (`TEXT_IN`) of the debugging module in debug firmware.
#[test]
fn uart0_reader_takes_console_input() {
    let mut project = Project::load(repo_path("examples/text_commands")).unwrap();
    let rx = project
        .logic
        .blocks
        .iter_mut()
        .find(|b| b.id == "rx")
        .unwrap();
    rx.params.insert("uart".into(), ParamValue::Int(0));

    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("while (Serial.available() > 0) {"), "{code}");
    assert!(code.contains("  Serial.begin(115200);"), "{code}");
    assert!(!code.contains("flp_dbg_in"), "{code}");

    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert!(
        code.contains("while (flp_dbg_input.available() > 0) {"),
        "{code}"
    );
    assert!(
        code.contains("char c = (char)flp_dbg_input.read();"),
        "{code}"
    );
    assert!(code.contains("FlpDebugInput flp_dbg_input;"), "{code}");
    assert!(
        code.contains("  if (type == 0x15) {\n    flp_dbg_in_push(p, len);"),
        "{code}"
    );
    assert_eq!(code.matches("Serial.begin(").count(), 1, "{code}");

    // Without a UART0 reader the module has no console input.
    let project = Project::load(repo_path("examples/text_commands")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("flp_dbg_in"), "{code}");
}

/// Block format features on a test block (`tests/fixtures/library/probe`):
/// `once` sections go in once per distinct text, `@functions` before `setup()`,
/// ports and parameters switched off by `when` are absent from the code.
#[test]
fn shared_code_snapshot() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let (lib, errors) = Library::load(fixtures.join("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let project = Project::load(fixtures.join("shared_code")).unwrap();
    let code = generate(&project, &lib, &ArduinoCpp::RELEASE).unwrap();
    let snapshot =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/shared_code.ino");
    assert_snapshot(&snapshot, &code);

    // Two buses for three blocks; the helper function once.
    assert_eq!(code.matches("Bus bus_4(4);").count(), 1, "{code}");
    assert_eq!(code.matches("Bus bus_5(5);").count(), 1, "{code}");
    assert_eq!(
        code.matches("float probe_read(Bus &bus)").count(),
        1,
        "{code}"
    );
    assert!(
        code.find("float a_value()") < code.find("void setup()"),
        "{code}"
    );
    // `raw` exists only in mode "raw", `en` only with use_en.
    assert!(code.contains("int32_t b_raw"), "{code}");
    assert!(!code.contains("a_raw"), "{code}");
    assert!(code.contains("if (a_ok) b_last"), "{code}");
}

/// Block ids may be Cyrillic: the sketch keeps them in comments and uses
/// transliterated C++ names, so it stays plain ASCII code.
#[test]
fn cyrillic_ids_give_ascii_code() {
    let mut project = Project::load(repo_path("examples/thermostat")).unwrap();
    let ids: Vec<String> = project.logic.blocks.iter().map(|b| b.id.clone()).collect();
    for (i, id) in ids.iter().enumerate() {
        project
            .rename_block(id, &format!("блок_{i}_ё"))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    for line in code.lines() {
        let code_part = line.split("//").next().unwrap();
        assert!(code_part.is_ascii(), "не ASCII вне комментария: {line}");
    }
    assert!(code.contains("// блок_0_ё"), "{code}");
}

/// Templates can name Cyrillic ports and parameters as attributes.
#[test]
fn templates_accept_cyrillic_names() {
    let env = minijinja::Environment::new();
    let text = env
        .render_str(
            "{{ input.вход }}",
            minijinja::context! { input => minijinja::context! { вход => "x" } },
        )
        .unwrap();
    assert_eq!(text, "x");
}

/// The `string` signal type: `String` storage, a `String("...")` default for an
/// unconnected input, a retained variable saved as a string, and no string
/// entries in the debug table (values travel as 4 bytes).
#[test]
fn string_signals_snapshot() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let (lib, errors) = Library::load(fixtures.join("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    let project = Project::load(fixtures.join("strings")).unwrap();
    let code = generate(&project, &lib, &ArduinoCpp::RELEASE).unwrap();
    let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/strings.ino");
    assert_snapshot(&snapshot, &code);
    assert!(code.contains("String join_out = String();"), "{code}");
    assert!(code.contains(" + String(\" — ok\");"), "{code}");
    assert!(code.contains("flp_retain.getString("), "{code}");
    assert!(code.contains("flp_retain.putString("), "{code}");
    assert!(!code.contains("sizeof(var_status)"), "{code}");

    let debug = generate(&project, &lib, &ArduinoCpp::DEBUG).unwrap();
    let table = debug
        .lines()
        .find(|l| l.starts_with("void* const flp_dbg_ptr"))
        .unwrap();
    assert!(table.contains("&var_count"), "{table}");
    assert!(
        !table.contains("var_status") && !table.contains("join_out"),
        "{table}"
    );
}
