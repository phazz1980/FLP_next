use std::path::PathBuf;

use flp_core::{check, Library, Logic, Project, Report};

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

fn check_src(src: &str) -> Report {
    let logic: Logic = toml::from_str(src).unwrap();
    check(&logic, &library(), None)
}

fn error_messages(report: &Report) -> Vec<String> {
    report.errors().map(|d| d.message.clone()).collect()
}

/// Asserts that exactly one error is reported and it contains `needle`.
fn assert_single_error(src: &str, needle: &str) {
    let report = check_src(src);
    let errors = error_messages(&report);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert!(errors[0].contains(needle), "{errors:#?}");
}

const INPUT: &str = r#"
[[block]]
id = "button"
type = "digital_in"
pin = 13
"#;

const LAMP: &str = r#"
[[block]]
id = "lamp"
type = "digital_out"
pin = 2
"#;

#[test]
fn pump_example_is_valid() {
    let p = Project::load(repo_path("examples/pump")).unwrap();
    let report = check(&p.logic, &library(), Some(&p.layout));
    assert!(report.diagnostics.is_empty(), "{:#?}", report.diagnostics);
    let order: Vec<&str> = report
        .order
        .iter()
        .map(|&i| p.logic.blocks[i].id.as_str())
        .collect();
    assert_eq!(order, ["sensor", "pressure", "low", "delay", "pump"]);
}

#[test]
fn order_follows_links_not_declaration() {
    let src = format!("{LAMP}{INPUT}\n[[link]]\nfrom = \"button.out\"\nto = \"lamp.in\"\n");
    let report = check_src(&src);
    assert!(!report.has_errors(), "{:#?}", report.diagnostics);
    assert_eq!(report.order, [1, 0]);
}

#[test]
fn unknown_block_type() {
    assert_single_error(
        "[[block]]\nid = \"x\"\ntype = \"no_such_block\"\n",
        "неизвестный тип «no_such_block»",
    );
}

#[test]
fn duplicate_and_invalid_ids() {
    let src = format!("{INPUT}{INPUT}");
    assert_single_error(&src, "встречается дважды");
    assert_single_error(
        "[[block]]\nid = \"1 st\"\ntype = \"digital_in\"\npin = 1\n",
        "недопустимый id",
    );
}

#[test]
fn param_errors() {
    assert_single_error(
        "[[block]]\nid = \"b\"\ntype = \"digital_in\"\n",
        "не задан параметр «pin»",
    );
    assert_single_error(
        "[[block]]\nid = \"b\"\ntype = \"digital_in\"\npin = \"13\"\n",
        "ожидается номер вывода",
    );
    assert_single_error(
        "[[block]]\nid = \"b\"\ntype = \"digital_in\"\npin = -1\n",
        "не может быть отрицательным",
    );
    assert_single_error(
        "[[block]]\nid = \"b\"\ntype = \"digital_in\"\npin = 1\nspeed = 5\n",
        "нет параметра «speed»",
    );
}

#[test]
fn bad_duration() {
    let src = format!(
        "{INPUT}\n[[block]]\nid = \"t\"\ntype = \"timer_on\"\ntime = \"5 лет\"\n\
         [[link]]\nfrom = \"button.out\"\nto = \"t.in\"\n"
    );
    assert_single_error(&src, "ожидается время");
}

#[test]
fn link_to_missing_block() {
    let src = format!("{LAMP}\n[[link]]\nfrom = \"ghost.out\"\nto = \"lamp.in\"\n");
    assert_single_error(&src, "нет блока «ghost»");
}

#[test]
fn link_to_missing_port() {
    let src = format!("{INPUT}{LAMP}\n[[link]]\nfrom = \"button.value\"\nto = \"lamp.in\"\n");
    assert_single_error(&src, "нет порта «value»");
}

#[test]
fn link_in_wrong_direction() {
    // Both ends are wrong, and lamp.in is left without a source.
    let src = format!("{INPUT}{LAMP}\n[[link]]\nfrom = \"lamp.in\"\nto = \"button.out\"\n");
    let errors = error_messages(&check_src(&src));
    assert!(errors.iter().any(|e| e.contains("это вход")), "{errors:#?}");
    assert!(
        errors.iter().any(|e| e.contains("это выход")),
        "{errors:#?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("ничего не подано")),
        "{errors:#?}"
    );
}

#[test]
fn incompatible_types() {
    // analog_in.out is int, digital_out.in is bool.
    let src = format!(
        "[[block]]\nid = \"adc\"\ntype = \"analog_in\"\npin = 34\n{LAMP}\
         [[link]]\nfrom = \"adc.out\"\nto = \"lamp.in\"\n"
    );
    assert_single_error(&src, "не подходит ко входу bool");
}

#[test]
fn int_feeds_float() {
    let src = r#"
        [[block]]
        id = "adc"
        type = "analog_in"
        pin = 34

        [[block]]
        id = "low"
        type = "less"

        [[link]]
        from = "adc.out"
        to = "low.a"
    "#;
    let report = check_src(src);
    assert!(!report.has_errors(), "{:#?}", report.diagnostics);
}

#[test]
fn input_without_source() {
    assert_single_error(LAMP, "на вход «in» ничего не подано");
}

#[test]
fn two_sources_for_one_input() {
    let src = format!(
        "{INPUT}\n[[block]]\nid = \"button2\"\ntype = \"digital_in\"\npin = 14\n{LAMP}\
         [[link]]\nfrom = \"button.out\"\nto = \"lamp.in\"\n\
         [[link]]\nfrom = \"button2.out\"\nto = \"lamp.in\"\n"
    );
    assert_single_error(&src, "уже подан «button.out»");
}

// Loops need bool→bool blocks without memory, which the library does not have yet;
// they are tested with synthetic blocks in `check.rs`.

#[test]
fn layout_with_unknown_block_is_warning() {
    let mut p = Project::load(repo_path("examples/pump")).unwrap();
    p.layout.blocks.insert(
        "ghost".into(),
        flp_core::BlockPos {
            sheet: 1,
            x: 0.0,
            y: 0.0,
        },
    );
    let report = check(&p.logic, &library(), Some(&p.layout));
    assert!(!report.has_errors());
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.diagnostics[0].message.contains("«ghost»"));
}

#[test]
fn optional_inputs_may_stay_unconnected() {
    // generator.en defaults to true, counter.down and counter.reset to false.
    let src = r#"
        [[block]]
        id = "blink"
        type = "generator"

        [[block]]
        id = "n"
        type = "counter"

        [[link]]
        from = "blink.out"
        to = "n.up"
    "#;
    let report = check_src(src);
    assert!(!report.has_errors(), "{:#?}", report.diagnostics);
}

#[test]
fn duration_min_is_checked_in_ms() {
    let src = "[[block]]\nid = \"g\"\ntype = \"generator\"\ntime_on = \"0ms\"\n";
    assert_single_error(src, "меньше допустимого 1");
}

const VARS: &str = r#"
[[var]]
name = "t"
type = "float"

[[var]]
name = "on"
type = "bool"
init = true
"#;

#[test]
fn variables_order_writes_before_reads() {
    let src = format!(
        "{VARS}\n[[block]]\nid = \"r\"\ntype = \"var_get\"\nvar = \"t\"\n\
         [[block]]\nid = \"hot\"\ntype = \"greater\"\nb = 30.0\n\
         [[block]]\nid = \"on_r\"\ntype = \"var_get\"\nvar = \"on\"\n\
         [[block]]\nid = \"adc\"\ntype = \"analog_in\"\npin = 34\n\
         [[block]]\nid = \"w\"\ntype = \"var_set\"\nvar = \"t\"\n\
         [[link]]\nfrom = \"r.out\"\nto = \"hot.a\"\n\
         [[link]]\nfrom = \"adc.out\"\nto = \"w.in\"\n"
    );
    let logic: Logic = toml::from_str(&src).unwrap();
    let report = check(&logic, &library(), None);
    assert!(!report.has_errors(), "{:#?}", report.diagnostics);
    let pos = |id: &str| {
        let i = logic.blocks.iter().position(|b| b.id == id).unwrap();
        report.order.iter().position(|&o| o == i).unwrap()
    };
    assert!(pos("w") < pos("r"), "write must run before read");
    assert!(report.diagnostics.is_empty(), "{:#?}", report.diagnostics);
}

#[test]
fn variable_errors() {
    assert_single_error(
        "[[block]]\nid = \"r\"\ntype = \"var_get\"\nvar = \"nope\"\n",
        "нет переменной «nope»",
    );
    let two_writers = format!(
        "{VARS}\n[[block]]\nid = \"a\"\ntype = \"digital_in\"\npin = 1\n\
         [[block]]\nid = \"w1\"\ntype = \"var_set\"\nvar = \"on\"\n\
         [[block]]\nid = \"w2\"\ntype = \"var_set\"\nvar = \"on\"\n\
         [[block]]\nid = \"r\"\ntype = \"var_get\"\nvar = \"t\"\n\
         [[link]]\nfrom = \"a.out\"\nto = \"w1.in\"\n\
         [[link]]\nfrom = \"a.out\"\nto = \"w2.in\"\n"
    );
    assert_single_error(&two_writers, "уже записывается блоком «w1»");
    // A bool variable cannot be written from an int output.
    let mismatch = format!(
        "{VARS}\n[[block]]\nid = \"adc\"\ntype = \"analog_in\"\npin = 34\n\
         [[block]]\nid = \"w\"\ntype = \"var_set\"\nvar = \"on\"\n\
         [[block]]\nid = \"r\"\ntype = \"var_get\"\nvar = \"t\"\n\
         [[link]]\nfrom = \"adc.out\"\nto = \"w.in\"\n"
    );
    assert_single_error(&mismatch, "не подходит ко входу bool");
}

#[test]
fn unused_variable_is_warning() {
    let report = check_src(VARS);
    assert!(!report.has_errors());
    assert_eq!(report.diagnostics.len(), 2, "{:#?}", report.diagnostics);
}

#[test]
fn loop_through_variable_is_allowed() {
    // n = n + 1 every scan: the read takes the previous value.
    let src = r#"
        [[var]]
        name = "n"
        type = "float"

        [[block]]
        id = "r"
        type = "var_get"
        var = "n"

        [[block]]
        id = "one"
        type = "constant"
        value = 1.0

        [[block]]
        id = "inc"
        type = "add"

        [[block]]
        id = "w"
        type = "var_set"
        var = "n"

        [[link]]
        from = "r.out"
        to = "inc.a"

        [[link]]
        from = "one.out"
        to = "inc.b"

        [[link]]
        from = "inc.out"
        to = "w.in"
    "#;
    let report = check_src(src);
    assert!(!report.has_errors(), "{:#?}", report.diagnostics);
}

#[test]
fn sheets_swap_and_delete() {
    let mut p = Project::load(repo_path("examples/pump")).unwrap();
    let id = p.logic.blocks[0].id.clone();
    let before = p.layout.blocks[&id].sheet;
    p.swap_sheets(before, 7);
    assert_eq!(p.layout.blocks[&id].sheet, 7);
    p.delete_sheet(7).unwrap();
    assert!(!p.layout.blocks.contains_key(&id));
    assert!(p.logic.block(&id).is_none());
    assert!(p
        .logic
        .links
        .iter()
        .all(|l| l.from.block != id && l.to.block != id));
}

#[test]
fn disabled_sheet_is_left_out_of_the_check() {
    let lib = library();
    let mut project = Project::load(repo_path("examples/pump")).unwrap();
    let id = project.logic.blocks[0].id.clone();
    project.logic.blocks[0].kind = "no_such_block".into();
    assert!(!flp_core::check_project(&project, &lib)
        .diagnostics
        .is_empty());
    let sheet = project.layout.blocks[&id].sheet;
    project.layout.sheets.push(flp_core::SheetInfo {
        number: sheet,
        disabled: true,
        ..Default::default()
    });
    let rest = project.without_disabled();
    assert!(!rest.logic.blocks.iter().any(|b| b.id == id));
    assert!(rest
        .logic
        .links
        .iter()
        .all(|l| l.from.block != id && l.to.block != id));
}

const EXPANDER: &str = r#"
[[expander]]
id = "relays"
chip = "pcf8574"
address = 0x20
"#;

fn with_expander(blocks: &str) -> String {
    format!("{EXPANDER}{blocks}")
}

#[test]
fn block_on_expander_pin_is_valid_and_takes_no_gpio() {
    // pin 4 on the chip and pin 4 on the board are different things
    let src = with_expander(
        r#"
[[block]]
id = "a"
type = "digital_out"
const = { in = true }
expander = "relays"
pin = 4

[[block]]
id = "b"
type = "digital_out"
const = { in = true }
pin = 4
"#,
    );
    let report = check_src(&src);
    assert!(report.diagnostics.is_empty(), "{:#?}", report.diagnostics);
}

#[test]
fn expander_pin_taken_twice() {
    let src = with_expander(
        r#"
[[block]]
id = "a"
type = "digital_out"
const = { in = true }
expander = "relays"
pin = 3

[[block]]
id = "b"
type = "digital_in"
expander = "relays"
pin = 3
"#,
    );
    assert_single_error(&src, "уже занят блоком «a»");
}

#[test]
fn expander_pin_out_of_range() {
    let src = with_expander(
        r#"
[[block]]
id = "a"
type = "digital_out"
const = { in = true }
expander = "relays"
pin = 8
"#,
    );
    assert_single_error(&src, "нет вывода P8");
}

#[test]
fn unknown_expander_and_unsupported_block() {
    assert_single_error(
        r#"
[[block]]
id = "a"
type = "digital_out"
const = { in = true }
expander = "nope"
pin = 1
"#,
        "расширитель «nope» не найден",
    );
    assert_single_error(
        &with_expander(
            r#"
[[block]]
id = "a"
type = "analog_in"
expander = "relays"
pin = 1
"#,
        ),
        "не может стоять на выводе расширителя",
    );
}

#[test]
fn expander_address_clash() {
    let src = format!(
        "{EXPANDER}{}",
        r#"
[[expander]]
id = "second"
chip = "pcf8574"
address = 0x20
"#
    );
    assert_single_error(&src, "уже занят расширителем «relays»");
}

/// Kincony-like board: PCF8574 at 0x24 behind the device "relays"; the board brings out
/// P0 and P2 (outputs, K1 and K2) and P1 (an input, X1) only.
fn board_with_expander_pins() -> Library {
    use flp_core::boards::{BoardExpanderPinDef, BoardInfo, DeviceDef};
    use flp_core::{BoardDef, Boards, ParamValue, PinFunction, UserBoard};
    let device = DeviceDef {
        id: "relays".into(),
        kind: "pcf8574_out".into(),
        name: "Реле".into(),
        description: String::new(),
        channels: Vec::new(),
        params: [("address".to_owned(), ParamValue::Int(0x24))].into(),
    };
    let pin = |pin, name: &str, f| BoardExpanderPinDef {
        device: "relays".into(),
        pin,
        name: name.into(),
        note: String::new(),
        functions: Some(vec![f]),
    };
    let def = BoardDef {
        info: BoardInfo {
            id: "mine".into(),
            name: "Моя плата".into(),
            chip: "esp32".into(),
            fqbn: "esp32:esp32:esp32".into(),
            version: 1,
            options: Default::default(),
        },
        pins: Vec::new(),
        expander_pins: vec![
            pin(0, "K1", PinFunction::Dout),
            pin(1, "X1", PinFunction::Din),
            pin(2, "K2", PinFunction::Dout),
        ],
        devices: vec![device],
    };
    def.validate(None).unwrap();
    let mut boards = Boards::default();
    boards.insert(UserBoard {
        def,
        dir: Default::default(),
    });
    let mut lib = library();
    lib.set_boards(boards);
    lib
}

fn board_project(blocks: &str) -> Project {
    let src =
        format!("[[expander]]\nid = \"relays\"\nchip = \"pcf8574\"\naddress = 0x24\n{blocks}");
    Project {
        path: Default::default(),
        config: toml::from_str(
            "[project]\nname = \"p\"\n[board]\nmodel = \"mine\"\nfqbn = \"esp32:esp32:esp32\"\n",
        )
        .unwrap(),
        logic: toml::from_str(&src).unwrap(),
        layout: Default::default(),
    }
}

#[test]
fn expander_pins_of_the_board_follow_its_functions() {
    let lib = board_with_expander_pins();
    let project = board_project(
        r#"
[[block]]
id = "k1"
type = "digital_out"
const = { in = true }
expander = "relays"
pin = 0

[[block]]
id = "x1"
type = "digital_in"
expander = "relays"
pin = 1

[[block]]
id = "wrong"
type = "digital_in"
expander = "relays"
pin = 2

[[block]]
id = "missing"
type = "digital_out"
const = { in = true }
expander = "relays"
pin = 5
"#,
    );
    let messages: Vec<String> = flp_core::check_project(&project, &lib)
        .errors()
        .map(|d| d.message.clone())
        .collect();
    assert_eq!(
        messages,
        [
            "блок «wrong»: вывод P2 расширителя «relays» не работает как цифровой вход у Моя плата",
            "блок «missing»: вывод P5 расширителя «relays» не выведен на плате у Моя плата",
        ]
    );
    // Another chip address is not the board's expander: the board has no say there.
    let mut other = project.clone();
    other.logic.expanders[0].address = 0x25;
    assert!(flp_core::check_project(&other, &lib)
        .errors()
        .next()
        .is_none());
}

#[test]
fn tag_copy_errors() {
    let copy = |tag: &str, kind: &str| {
        format!("{INPUT}\n[[block]]\nid = \"copy\"\ntype = \"{kind}\"\ntag = \"{tag}\"\n")
    };
    assert_single_error(&copy("nobody", "digital_in"), "нет тега «nobody»");
    let errors = error_messages(&check_src(&copy("button", "digital_out")));
    assert!(
        errors
            .iter()
            .any(|e| e.contains("не совпадает с типом тега")),
        "{errors:#?}"
    );
    let chain = format!(
        "{}\n[[block]]\nid = \"copy2\"\ntype = \"digital_in\"\ntag = \"copy\"\n",
        copy("button", "digital_in")
    );
    assert_single_error(&chain, "сам копия");
    let not_tag = "[[block]]\nid = \"n\"\ntype = \"not\"\n\n[[block]]\nid = \"copy\"\ntype = \"not\"\ntag = \"n\"\n";
    let errors = error_messages(&check_src(not_tag));
    assert!(
        errors.iter().any(|e| e.contains("нет вывода платы")),
        "{errors:#?}"
    );
}

#[test]
fn input_copy_runs_after_tag() {
    let src = format!("[[block]]\nid = \"copy\"\ntype = \"digital_in\"\ntag = \"button\"\n{INPUT}");
    let logic: Logic = toml::from_str(&src).unwrap();
    let report = check(&logic, &library(), None);
    assert!(report.errors().next().is_none());
    let order: Vec<&str> = report
        .order
        .iter()
        .map(|&i| logic.blocks[i].id.as_str())
        .collect();
    assert_eq!(order, ["button", "copy"]);
}

#[test]
fn unused_tag_waits_in_tree() {
    let lib = library();
    let mut p = flp_core::Project::load(repo_path("examples/tag_copies")).unwrap();
    assert!(p.unused_tags(&lib).is_empty());
    // Taking every image of the lamp off the sheets leaves an unused output
    // without an "empty input" error.
    p.take_off_board("lamp_2").unwrap();
    p.take_off_board("lamp").unwrap();
    assert!(p.logic.block("lamp").is_some());
    assert_eq!(p.unused_tags(&lib), ["lamp"]);
    let report = flp_core::check_project(&p, &lib);
    let errors = error_messages(&report);
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn switch_inputs_share_one_type() {
    // A string and a number on the data inputs of a mux.
    let mixed = r#"
[[block]]
id = "pick"
type = "mux"
const = { sel = 1, a = "текст", b = 5 }
"#;
    assert_single_error(mixed, "разные типы");

    // `int` and `float` meet as `float`; a bool goes through a switch to a lamp.
    let fine = format!(
        r#"{INPUT}{LAMP}
[[block]]
id = "pick"
type = "mux"
const = {{ sel = 1, a = 2, b = 2.5 }}

[[block]]
id = "led_src"
type = "switch"
const = {{ a = false }}

[[link]]
from = "button.out"
to   = "led_src.sel"

[[link]]
from = "button.out"
to   = "led_src.b"

[[link]]
from = "led_src.out"
to   = "lamp.in"
"#
    );
    let report = check_src(&fine);
    assert!(!report.has_errors(), "{:#?}", error_messages(&report));
}

#[test]
fn same_output_keeps_its_real_type() {
    // A switch of strings cannot feed a lamp.
    let src = format!(
        r#"{LAMP}
[[block]]
id = "pick"
type = "switch"
const = {{ sel = true, a = "да", b = "нет" }}

[[link]]
from = "pick.out"
to   = "lamp.in"
"#
    );
    assert_single_error(&src, "нужен блок преобразования");
}
