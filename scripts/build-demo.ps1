# Builds the browser demo into docs/demo (served by GitHub Pages).
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version 0.2.130
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
Push-Location $root
try {
    cargo build -p flp-demo --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    wasm-bindgen --target web --no-typescript --out-dir docs/demo --out-name flp_demo `
        target/wasm32-unknown-unknown/release/flp_demo.wasm
    if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }
    $size = (Get-Item docs/demo/flp_demo_bg.wasm).Length / 1MB
    "docs/demo ready, wasm {0:N1} MB" -f $size
} finally { Pop-Location }
