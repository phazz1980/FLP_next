@echo off
rem Builds block.wasm from src/block.rs (needs rustc with the wasm32-unknown-unknown target).
rem Small stack and memory: the module is meant to run on a microcontroller too.
rustc --target wasm32-unknown-unknown --crate-type cdylib -C opt-level=s -C panic=abort -C lto ^
  -C link-arg=-zstack-size=1024 -C link-arg=--initial-memory=65536 -C link-arg=--max-memory=65536 ^
  src/block.rs -o block.wasm
