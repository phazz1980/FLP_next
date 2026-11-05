//! Embeds the block library and the demo examples as source texts.

use std::{env, fs, path::Path};

const EXAMPLES: &[&str] = &[
    "pump",
    "blink",
    "logic_gates",
    "thermostat",
    "start_stop",
    "toggle",
];

fn lit(path: &Path) -> String {
    // A project may have no layout file.
    if !path.is_file() {
        return "\"\"".to_owned();
    }
    format!(
        "include_str!({:?})",
        path.to_str().unwrap().replace('\\', "/")
    )
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = String::from("pub const BLOCKS: &[(&str, &str)] = &[\n");
    let mut dirs: Vec<_> = fs::read_dir(root.join("library"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("block.toml").is_file())
        .collect();
    dirs.sort();
    for d in &dirs {
        let name = d.file_name().unwrap().to_str().unwrap();
        out += &format!("    ({name:?}, {}),\n", lit(&d.join("block.toml")));
    }
    out += "];\n\npub const EXAMPLES: &[(&str, &str, &str, &str)] = &[\n";
    for name in EXAMPLES {
        let d = root.join("examples").join(name);
        out += &format!(
            "    ({name:?}, {}, {}, {}),\n",
            lit(&d.join("project.toml")),
            lit(&d.join("logic.toml")),
            lit(&d.join("layout.toml")),
        );
    }
    out += "];\n";
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("data.rs"),
        out,
    )
    .unwrap();
    println!("cargo:rerun-if-changed=../../library");
    println!("cargo:rerun-if-changed=../../examples");
}
