//! TON (on-delay timer) as a no_std WASM block: input 0 = in, param 0 = time (ms), output 0 = out.
#![no_std]

#[link(wasm_import_module = "crab")]
extern "C" {
    fn input(i: i32) -> f64;
    fn param(i: i32) -> f64;
    fn output(i: i32, v: f64);
}

static mut ELAPSED_MS: f64 = 0.0;

#[no_mangle]
pub extern "C" fn init() {
    unsafe { ELAPSED_MS = 0.0 };
}

#[no_mangle]
pub extern "C" fn step(dt_ms: i32) {
    unsafe {
        if input(0) != 0.0 {
            ELAPSED_MS += dt_ms as f64;
            output(0, if ELAPSED_MS >= param(0) { 1.0 } else { 0.0 });
        } else {
            ELAPSED_MS = 0.0;
            output(0, 0.0);
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
