//! Simulation module of the `lowpass` block in Rust: exponential smoothing.
//! ABI: see help/10-block-editor.md ("Модуль симуляции").

#![no_std]

#[link(wasm_import_module = "crab")]
extern "C" {
    fn input(index: i32) -> f64;
    fn param(index: i32) -> f64;
    fn output(index: i32, value: f64);
}

static mut Y: f64 = 0.0;

#[no_mangle]
pub extern "C" fn init() {
    unsafe { Y = 0.0 };
}

#[no_mangle]
pub extern "C" fn step(dt_ms: i32) {
    unsafe {
        let tau = param(0);
        let dt = f64::from(dt_ms);
        Y += dt / (tau + dt) * (input(0) - Y);
        output(0, Y);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
