//! Snapshot tests: every example in `examples/` is generated and compared with
//! `tests/snapshots/<example>.ino`. A missing snapshot is created; to accept
//! changes run `UPDATE_SNAPSHOTS=1 cargo test` and review the diff in git.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use flp_codegen::{generate, ArduinoCpp, GenError};
use flp_core::boards::Boards;
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

/// The library with the boards the example carries in its `user_boards/` folder.
fn library_for(example: &Path) -> Library {
    let mut lib = library();
    let (boards, errors) = Boards::load(&example.join("user_boards"));
    assert!(errors.is_empty(), "{errors:?}");
    if !boards.is_empty() {
        let errors = lib.set_boards(boards);
        assert!(errors.is_empty(), "{errors:?}");
    }
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
    for dir in examples() {
        let lib = library_for(&dir);
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
/// started in `setup()` from `[[uart]]`, USB only when nothing else starts it.
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
uart = -1
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
    let setup = "  // USB: 115200 бод
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
    assert!(code.contains("  // Serial1, 19200"), "{code}");
    // The debug module starts USB itself.
    let code = generate(&project, &lib, &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("// USB: "), "{code}");

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
    use ParamValue::{Int, Text};
    set(
        &mut project,
        "report",
        &[("trigger", Int(1)), ("ending", Int(1))],
    );
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
    assert!(code.contains(r#"bool end = c == ";"[0];"#), "{code}");
    assert!(code.contains("millis() - rx_last >= 200UL"), "{code}");

    // Byte by byte: other outputs; the line outputs are gone, so the links go too.
    set(&mut project, "rx", &[("mode", Int(1))]);
    project.logic.links.retain(|l| l.from.block != "rx");
    project.logic.blocks.retain(|b| b.id != "command_w");
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("rx_new = Serial2.available() > 0;"), "{code}");
    assert!(code.contains("rx_byte = Serial2.read();"), "{code}");
    assert!(!code.contains("rx_buf"), "{code}");

    // Debug firmware starts USB itself; the block only writes to it.
    let project = Project::load(repo_path("examples/text_commands")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert_eq!(code.matches("Serial.begin(").count(), 1, "{code}");
}

/// Source of `p_send.value`, parameters and the lines expected in the code.
type Case<'a> = (&'a str, &'a [(&'a str, ParamValue)], &'a [&'a str]);

/// `uart_send` takes the type of what feeds `value`: text or binary per type,
/// and "on change" compares values of that type.
#[test]
fn uart_send_value_types() {
    // (source of p_send.value, parameters) → expected lines.
    let cases: [Case; 6] = [
        (
            "length.out",
            &[
                ("send_as", ParamValue::Int(1)),
                ("int_size", ParamValue::Int(2)),
            ],
            &["int32_t data = length_out;"],
        ),
        (
            "length.out",
            &[
                ("send_as", ParamValue::Int(1)),
                ("int_size", ParamValue::Int(1)),
            ],
            &["int16_t data = (int16_t)constrain(length_out, -32768L, 32767L);"],
        ),
        (
            "length.out",
            &[
                ("send_as", ParamValue::Int(1)),
                ("int_size", ParamValue::Int(0)),
            ],
            &["uint8_t data = (uint8_t)constrain(length_out, 0L, 255L);"],
        ),
        (
            "bar.out",
            &[
                ("send_as", ParamValue::Int(0)),
                ("decimals", ParamValue::Int(3)),
            ],
            &[
                "String text = String(bar_out, 3);",
                "fabsf(bar_out - p_send_last) >= 0.05f",
            ],
        ),
        (
            "is_on.out",
            &[
                ("send_as", ParamValue::Int(0)),
                ("trigger", ParamValue::Int(2)),
            ],
            &[
                r#"String text = is_on_out ? "1" : "0";"#,
                "bool p_send_last;",
            ],
        ),
        (
            "message.out",
            &[("send_as", ParamValue::Int(1))],
            &[
                "const String& text = message_out;",
                r#"const char* ending = "";"#,
                "String p_send_last;",
                "!p_send_started || message_out != p_send_last;",
            ],
        ),
    ];
    for (source, params, expected) in cases {
        let mut project = Project::load(repo_path("examples/text_commands")).unwrap();
        let block = project
            .logic
            .blocks
            .iter_mut()
            .find(|b| b.id == "p_send")
            .unwrap();
        for (k, v) in params {
            block.params.insert((*k).to_owned(), v.clone());
        }
        let link = project
            .logic
            .links
            .iter_mut()
            .find(|l| l.to.block == "p_send" && l.to.port == "value")
            .unwrap();
        let (b, port) = source.split_once('.').unwrap();
        link.from = flp_core::PortRef::new(b, port);
        let code = generate(&project, &library(), &ArduinoCpp::RELEASE)
            .unwrap_or_else(|e| panic!("{source}: {e}"));
        for line in expected {
            assert!(code.contains(line), "{source}: нет «{line}»\n{code}");
        }
    }
}

/// UART0 is a port of its own on ESP32-C3 (`Serial0`), next to USB (`Serial`);
/// on ESP32 it is the USB bridge and not offered.
#[test]
fn uart0_of_esp32c3() {
    let mut project = Project::load(repo_path("examples/text_commands")).unwrap();
    project.config.board.fqbn = "esp32:esp32:esp32c3".into();
    for (id, key) in [("port2", "uart"), ("rx", "port"), ("p_send", "port")] {
        let block = project
            .logic
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .unwrap();
        block.params.insert(key.into(), ParamValue::Int(0));
        // GPIO16/17 of the example are not on ESP32-C3.
        block.params.remove("rx");
        block.params.remove("tx");
    }
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("  Serial.begin(115200);"), "{code}");
    assert!(code.contains("  Serial0.setRxBufferSize(256);"), "{code}");
    assert!(code.contains("while (Serial0.available() > 0) {"), "{code}");
    assert!(
        code.contains("Serial0.write((const uint8_t*)&data, sizeof(data));"),
        "{code}"
    );

    project.config.board.fqbn = "esp32:esp32:esp32".into();
    match generate(&project, &library(), &ArduinoCpp::RELEASE) {
        Err(GenError::Schema(d)) => assert!(
            d.iter()
                .any(|d| d.message.contains("у ESP32 UART0 — это USB-мост")),
            "{d:?}"
        ),
        other => panic!("ожидалась ошибка схемы, получено {other:?}"),
    }
}

/// `uart_read` on USB: the port itself in normal firmware, the console input
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
    rx.params.insert("port".into(), ParamValue::Int(-1));

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

    // Without a USB reader the module has no console input.
    let project = Project::load(repo_path("examples/text_commands")).unwrap();
    let code = generate(&project, &library(), &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("flp_dbg_in"), "{code}");
}

/// `uart_config` on a sheet: settings of its UART, the speed from an input, and
/// one place per UART.
#[test]
fn uart_config_block() {
    let mut project = Project::load(repo_path("examples/text_commands")).unwrap();
    let port2 = project
        .logic
        .blocks
        .iter_mut()
        .find(|b| b.id == "port2")
        .unwrap();
    port2
        .params
        .insert("runtime".into(), ParamValue::Bool(true));
    port2.params.insert("speed".into(), ParamValue::Int(19200));
    port2
        .params
        .insert("format".into(), ParamValue::Text("8E1".into()));
    project.logic.links.extend(
        toml::from_str::<flp_core::Logic>(
            r#"
[[link]]
from = "length.out"
to = "port2.new_speed"

[[link]]
from = "arg.out"
to = "port2.new_format"

[[link]]
from = "tick.out"
to = "port2.apply"
"#,
        )
        .unwrap()
        .links,
    );
    let code = generate(&project, &library(), &ArduinoCpp::RELEASE).unwrap();
    assert!(
        code.contains("  Serial2.begin(19200, SERIAL_8E1, 16, 17);"),
        "{code}"
    );
    assert!(code.contains("if (tick_out && !port2_prev) {"), "{code}");
    assert!(code.contains("String format = arg_out;"), "{code}");
    assert!(
        code.contains(r#"if (f == "7O2") return SERIAL_7O2;"#),
        "{code}"
    );
    // A new format restarts the port with its pins and buffer; a speed alone does not.
    assert!(
        code.contains("Serial2.setRxBufferSize(256);\n      Serial2.begin(speed, config, 16, 17);"),
        "{code}"
    );
    assert!(
        code.contains("} else if (length_out > 0) {\n      Serial2.updateBaudRate(speed);"),
        "{code}"
    );
    assert!(
        code.contains("port2_baud = (int32_t)Serial2.baudRate();"),
        "{code}"
    );

    let schema_error = |project: &Project| match generate(project, &library(), &ArduinoCpp::RELEASE)
    {
        Err(GenError::Schema(d)) => d.into_iter().map(|d| d.message).collect::<Vec<_>>(),
        other => panic!("ожидалась ошибка схемы, получено {other:?}"),
    };
    // The same UART in the board settings too.
    let mut both = Project::load(repo_path("examples/text_commands")).unwrap();
    both.config.uart.push(flp_core::UartConfig::new(2));
    let m = schema_error(&both);
    assert!(
        m.iter()
            .any(|m| m.contains("блок «port2»: UART2 уже настроен в настройках платы")),
        "{m:?}"
    );
    // Two settings blocks for one UART.
    let mut twice = Project::load(repo_path("examples/text_commands")).unwrap();
    let mut copy = twice.logic.block("port2").unwrap().clone();
    copy.id = "port2_again".into();
    twice.logic.blocks.push(copy);
    let m = schema_error(&twice);
    assert!(
        m.iter()
            .any(|m| m.contains("блок «port2_again»: UART2 уже настроен блоком «port2»")),
        "{m:?}"
    );
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

/// Sheets marked "background" run in their own task; blocks sharing a library
/// object with the other task are refused, and without the flag there is no task.
#[test]
fn background_sheet_rules() {
    let lib = library();
    let code = generate(
        &Project::load(repo_path("examples/background")).unwrap(),
        &lib,
        &ArduinoCpp::RELEASE,
    )
    .unwrap();
    assert!(
        code.contains("xTaskCreatePinnedToCore(flp_bg_task"),
        "{code}"
    );
    assert!(
        code.contains("press_out_in = press_out_pub || press_out_pend;"),
        "{code}"
    );

    let mut plain = Project::load(repo_path("examples/background")).unwrap();
    plain.layout.sheets.clear();
    let code = generate(&plain, &lib, &ArduinoCpp::RELEASE).unwrap();
    assert!(!code.contains("flp_bg_task"), "{code}");

    // Single-core chip: no task, the background sheets stay in loop() on periods,
    // one sheet per pass, phases spread over the period.
    let mut c3 = Project::load(repo_path("examples/background")).unwrap();
    c3.config.board.fqbn = "esp32:esp32:esp32c3".into();
    c3.layout.sheets.push(flp_core::SheetInfo {
        number: 3,
        background: true,
        period: 20,
        ..Default::default()
    });
    c3.layout.blocks.get_mut("blink_led").unwrap().sheet = 3;
    let code = generate(&c3, &lib, &ArduinoCpp::RELEASE).unwrap();
    let snapshot =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/background_c3.ino");
    assert_snapshot(&snapshot, &code);
    assert!(!code.contains("flp_bg_task"), "{code}");
    assert!(code.contains("flp_bg2_t = 0u - 10u;"), "{code}");
    assert!(code.contains("flp_bg3_t = 10u - 20u;"), "{code}");
    assert!(code.contains("if (flp_bg2_run) {"), "{code}");
    assert!(code.contains("!flp_bg_one"), "{code}");

    // The broker alone on a background sheet: the whole network family follows it into
    // the background task, so the publishers on the main sheet are not an error.
    let mut split = Project::load(repo_path("examples/mqtt")).unwrap();
    split.layout.sheets.push(flp_core::SheetInfo {
        number: 2,
        background: true,
        ..Default::default()
    });
    split.layout.blocks.insert(
        "broker".into(),
        flp_core::layout::BlockPos {
            sheet: 2,
            x: 0.0,
            y: 0.0,
        },
    );
    let code = generate(&split, &lib, &ArduinoCpp::RELEASE).unwrap();
    assert!(code.contains("flp_bg_task"), "{code}");

    // Even with no background sheet at all, the network goes to core 0, and a bool
    // crossing the boundary keeps a latch so a pulse is not lost.
    let mut net = Project::load(repo_path("examples/mqtt")).unwrap();
    // The publication is hidden in the editor, but a wire into it is still valid.
    net.logic.links.push(flp_core::Link {
        from: "button.out".parse().unwrap(),
        to: "button_pub.en".parse().unwrap(),
    });
    let code = generate(&net, &lib, &ArduinoCpp::RELEASE).unwrap();
    assert!(
        code.contains("xTaskCreatePinnedToCore(flp_bg_task"),
        "{code}"
    );
    assert!(code.contains("button_out_pend = false;"), "{code}");

    // Debug build: the pass time of core 0 is measured and sent after the sheets.
    let code = generate(&net, &lib, &ArduinoCpp::DEBUG).unwrap();
    assert!(code.contains("flp_st_bg_begin();"), "{code}");
    assert!(code.contains("flp_st_bg_end();"), "{code}");
    assert!(code.contains("&flp_st_bg_max, 4"), "{code}");
    let plain = Project::load(repo_path("examples/background")).unwrap();
    let mut plain = plain;
    plain.layout.sheets.clear();
    let code = generate(&plain, &lib, &ArduinoCpp::DEBUG).unwrap();
    assert!(!code.contains("flp_st_bg"), "{code}");

    // Single-core chip: the network stays in loop().
    let mut c3 = Project::load(repo_path("examples/mqtt")).unwrap();
    c3.config.board.fqbn = "esp32:esp32:esp32c3".into();
    let code = generate(&c3, &lib, &ArduinoCpp::RELEASE).unwrap();
    assert!(!code.contains("flp_bg_task"), "{code}");
}

/// An output taken off every sheet stays in the project tree, unfed: it is left
/// out of the sketch, which still builds.
#[test]
fn unused_tag_is_generated_idle() {
    let lib = library();
    let mut project = Project::load(repo_path("examples/tag_copies")).unwrap();
    project.take_off_board("lamp_2").unwrap();
    project.take_off_board("lamp").unwrap();
    let code = generate(&project, &lib, &ArduinoCpp::RELEASE).unwrap();
    assert!(!code.contains("pinMode(2, OUTPUT);"), "{code}");
    assert!(code.contains("button_out"), "{code}");
}

/// Russian phrases of the sketch comments that `comment_lang = "en"` must replace.
/// The comments inside block templates are not translated, so only these generated
/// ones are checked.
const RU_PHRASES: &[&str] = &[
    "Сгенерировано CrabLogic",
    "Порядок выполнения",
    "Выходы блоков",
    "Обмен сигналами",
    "Периоды фоновых плат",
    "Переменные\n",
    "Фоновая задача",
    "Фоновые платы",
    "Сохранение переменных",
    "Восстановление переменных",
    "Расширитель",
    "Читает выводы",
    "Записывает выводы",
    "Диагностика",
    "Онлайн-отладка",
    "Наблюдаемые значения",
    "цикл, мкс",
    "общий код",
    "бод",
];

fn generate_in(example: &str, lang: &str, diagnostics: bool, target: &ArduinoCpp) -> String {
    let dir = repo_path(&format!("examples/{example}"));
    let mut project = Project::load(&dir).unwrap();
    project.config.project.comment_lang = lang.to_owned();
    project.config.diagnostics.enabled = diagnostics;
    generate(&project, &library_for(&dir), target).unwrap()
}

#[test]
fn english_comments_in_the_sketch() {
    let cases = [
        ("background", false, ArduinoCpp::RELEASE),
        ("retain", false, ArduinoCpp::RELEASE),
        ("pcf8574", false, ArduinoCpp::RELEASE),
        ("tags", true, ArduinoCpp::RELEASE),
        ("tags", false, ArduinoCpp::DEBUG),
    ];
    for (example, diagnostics, target) in cases {
        let ru = generate_in(example, "", diagnostics, &target);
        let en = generate_in(example, "en", diagnostics, &target);
        assert_ne!(ru, en, "{example}");
        assert!(en.contains("// Generated by CrabLogic"), "{example}");
        assert!(en.contains("// Execution order:"), "{example}");
        for phrase in RU_PHRASES {
            assert!(!en.contains(phrase), "{example}: «{phrase}»\n{en}");
        }
        // Only the comments differ: the code itself is the same.
        let code = |s: &str| {
            s.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .count()
        };
        assert_eq!(code(&ru), code(&en), "{example}");
    }
    let plain = generate_in("tags", "ru", false, &ArduinoCpp::RELEASE);
    assert_eq!(plain, generate_in("tags", "", false, &ArduinoCpp::RELEASE));
}

#[test]
fn english_sketch_snapshot() {
    let code = generate_in("background", "en", false, &ArduinoCpp::RELEASE);
    let snapshot =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/background.en.ino");
    assert_snapshot(&snapshot, &code);
}
