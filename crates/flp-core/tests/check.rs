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
        "[[block]]\nid = \"1st\"\ntype = \"digital_in\"\npin = 1\n",
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
