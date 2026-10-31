# Builds block.wasm into block/ from the Rust or the C++ source.
# Usage: .\build.ps1 rust   (needs: rustup target add wasm32-unknown-unknown)
#        .\build.ps1 cpp    (needs clang with the wasm32 target)
param([ValidateSet('rust', 'cpp')][string]$Lang = 'rust')
$here = $PSScriptRoot
if ($Lang -eq 'rust') {
    Push-Location "$here\rust"
    cargo build --release --target wasm32-unknown-unknown
    Pop-Location
    Copy-Item "$here\rust\target\wasm32-unknown-unknown\release\lowpass.wasm" "$here\block\block.wasm" -Force
} else {
    clang --target=wasm32 -O2 -nostdlib "-Wl,--no-entry" "-Wl,--allow-undefined" `
        -o "$here\block\block.wasm" "$here\cpp\lowpass.cpp"
}
