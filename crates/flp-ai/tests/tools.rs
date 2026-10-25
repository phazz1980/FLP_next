//! The command set against an in-memory project.

use std::fs;
use std::path::PathBuf;

use flp_ai::{call, tools, Context};
use flp_core::{Library, Project};
use serde_json::{json, Value};

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

struct Mem {
    project: Project,
    lib: Library,
    lib_dir: PathBuf,
    edits: usize,
}

impl Context for Mem {
    fn project(&self) -> &Project {
        &self.project
    }

    fn library(&self) -> &Library {
        &self.lib
    }

    fn library_dir(&self) -> PathBuf {
        self.lib_dir.clone()
    }

    fn edit(
        &mut self,
        edit: &mut dyn FnMut(&mut Project) -> Result<(), String>,
    ) -> Result<(), String> {
        // All or nothing, as in the real contexts.
        let mut p = self.project.clone();
        edit(&mut p)?;
        self.project = p;
        self.edits += 1;
        Ok(())
    }

    fn build(&mut self, _: bool, _: Option<String>, _: bool) -> Result<String, String> {
        Err("сборка в тестах не нужна".into())
    }

    fn live_values(&mut self) -> Result<Vec<(String, String, bool)>, String> {
        Ok(vec![("button.out".into(), "1".into(), false)])
    }

    fn reload_library(&mut self) -> Result<(), String> {
        let mut lib = Library::load(repo_path("library")).unwrap().0;
        lib.add_user_dir(&self.lib_dir).map_err(|e| e.to_string())?;
        self.lib = lib;
        Ok(())
    }
}

fn ctx(name: &str) -> Mem {
    let project = Project::load(repo_path("examples/button")).unwrap();
    let lib_dir = repo_path("target/test-ai-lib").join(name);
    let _ = fs::remove_dir_all(&lib_dir);
    fs::create_dir_all(&lib_dir).unwrap();
    Mem {
        project,
        lib: Library::load(repo_path("library")).unwrap().0,
        lib_dir,
        edits: 0,
    }
}

fn run(c: &mut Mem, name: &str, args: Value) -> Result<Value, String> {
    call(c, name, &args)
}

#[test]
fn every_tool_has_an_object_schema() {
    for t in tools() {
        assert_eq!(t.input_schema["type"], "object", "{}", t.name);
        assert!(!t.description.is_empty());
    }
}

#[test]
fn builds_a_schema_step_by_step() {
    let mut c = ctx("steps");
    let r = run(
        &mut c,
        "add_block",
        json!({"type": "timer_on", "id": "delay", "params": {"time": "2s"}}),
    )
    .unwrap();
    assert_eq!(r["id"], "delay");
    // The new timer's input is not connected yet: the check says so.
    assert!(r["check"]["errors"].to_string().contains("delay"));

    run(
        &mut c,
        "connect",
        json!({"from": "button.out", "to": "delay.in"}),
    )
    .unwrap();
    let r = run(
        &mut c,
        "connect",
        json!({"from": "delay.out", "to": "led.in"}),
    )
    .unwrap();
    assert_eq!(r["check"]["errors"], json!([]));
    assert_eq!(r["check"]["order"], json!(["button", "delay", "led"]));

    let schema = run(&mut c, "get_schema", json!({})).unwrap();
    let blocks = schema["logic"]["block"].as_array().unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(schema["placement"]["delay"]["sheet"], 1);

    let r = run(&mut c, "generate", json!({})).unwrap();
    let code = r["sketch"].as_str().unwrap();
    assert!(code.contains("// delay: timer_on"));
    assert!(code.contains("2000UL"));
}

#[test]
fn failed_edits_change_nothing() {
    let mut c = ctx("failed");
    let before = c.project.logic.clone();
    let err = run(
        &mut c,
        "add_block",
        json!({"type": "timer_on", "params": {"time": "скоро"}}),
    )
    .unwrap_err();
    assert!(err.contains("time"), "{err}");
    assert!(run(
        &mut c,
        "connect",
        json!({"from": "led.in", "to": "button.out"})
    )
    .is_err());
    assert!(run(&mut c, "connect", json!({"from": "nope", "to": "led.in"})).is_err());
    assert_eq!(c.project.logic, before);
    assert_eq!(c.edits, 0);
}

#[test]
fn constants_and_inversion() {
    let mut c = ctx("const");
    run(
        &mut c,
        "invert_port",
        json!({"port": "button.out", "output": true}),
    )
    .unwrap();
    let schema = run(&mut c, "get_schema", json!({})).unwrap();
    assert!(schema["logic"].to_string().contains("\"invert\":[\"out\"]"));
    let r = run(&mut c, "generate", json!({})).unwrap();
    assert!(r["sketch"].as_str().unwrap().contains("!button_out"));
    // Not an input of the button, and not a logical constant.
    assert!(run(
        &mut c,
        "invert_port",
        json!({"port": "button.out", "output": false})
    )
    .is_err());
    assert!(run(
        &mut c,
        "set_input_const",
        json!({"input": "led.in", "value": 1.5})
    )
    .is_err());

    // A constant replaces the link; removing it leaves the input unconnected.
    let r = run(
        &mut c,
        "set_input_const",
        json!({"input": "led.in", "value": true}),
    )
    .unwrap();
    assert_eq!(r["check"]["errors"], json!([]));
    assert!(c.project.logic.links.is_empty());
    let r = run(
        &mut c,
        "set_input_const",
        json!({"input": "led.in", "value": null}),
    )
    .unwrap();
    assert!(r["check"]["errors"].to_string().contains("led"));
}

#[test]
fn variables() {
    let mut c = ctx("vars");
    run(
        &mut c,
        "set_variable",
        json!({"name": "limit", "type": "float", "init": 22.5, "description": "уставка"}),
    )
    .unwrap();
    let r = run(
        &mut c,
        "add_variable_block",
        json!({"var": "limit", "write": false}),
    )
    .unwrap();
    assert_eq!(r["id"], "limit_r1");
    run(
        &mut c,
        "set_variable",
        json!({"name": "limit", "type": "float", "rename_to": "setpoint"}),
    )
    .unwrap();
    assert_eq!(
        c.project.logic.blocks.last().unwrap().var_name(),
        Some("setpoint")
    );
    run(&mut c, "remove_variable", json!({"name": "setpoint"})).unwrap();
    assert!(c.project.logic.vars.is_empty());
}

#[test]
fn library_queries() {
    let mut c = ctx("lib");
    let found = run(&mut c, "list_block_types", json!({"filter": "таймер"})).unwrap();
    let kinds: Vec<&str> = found
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["type"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"timer_on"), "{kinds:?}");
    let t = run(&mut c, "get_block_type", json!({"type": "timer_on"})).unwrap();
    assert!(t["template"].as_str().unwrap().contains("@loop"));
    assert!(t["docs"].as_str().unwrap().contains("TON"));
}

#[test]
fn creates_block_types_only_when_valid() {
    let mut c = ctx("create");
    let bad = run(
        &mut c,
        "create_block_type",
        json!({"type": "twice", "block_toml": "[block]\ntype = \"other\"\nname = \"x\"\ngroup = \"g\"\n", "template": ""}),
    );
    assert!(bad.is_err());
    assert!(!c.lib_dir.join("twice").exists());

    let toml = "[block]\ntype = \"twice\"\nname = \"Удвоение\"\ngroup = \"Числа\"\n\
                [[input]]\nname = \"in\"\ntype = \"float\"\n\
                [[output]]\nname = \"out\"\ntype = \"float\"\n";
    run(
        &mut c,
        "create_block_type",
        json!({"type": "twice", "block_toml": toml,
               "template": "// @loop\n{{ output.out }} = {{ input.in }} * 2.0f;\n",
               "docs": "# Удвоение\n"}),
    )
    .unwrap();
    assert!(c.lib.get("twice").is_some() && c.lib.is_user("twice"));
    let builtin = "[block]
type = \"and\"
name = \"И\"
group = \"Логика\"
";
    let taken = run(
        &mut c,
        "create_block_type",
        json!({"type": "and", "block_toml": builtin, "template": ""}),
    );
    assert!(taken.unwrap_err().contains("встроенным"));
    let r = run(&mut c, "add_block", json!({"type": "twice"})).unwrap();
    assert_eq!(r["id"], "twice_1");
}

#[test]
fn live_values_and_unknown_tool() {
    let mut c = ctx("live");
    let v = run(&mut c, "read_live_values", json!({})).unwrap();
    assert_eq!(v[0]["name"], "button.out");
    assert!(run(&mut c, "fly", json!({})).is_err());
}
