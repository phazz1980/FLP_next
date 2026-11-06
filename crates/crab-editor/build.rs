// Embeds the program icon into the exe (Explorer, taskbar, icon of `.crab` files).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let _ = embed_resource::compile("assets/icon.rc", embed_resource::NONE);
    }
}
