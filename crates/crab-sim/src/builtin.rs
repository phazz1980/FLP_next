//! Blocks that need the editor or hardware, and the dispatch to `crab-blocks`. Every value is `f64`: bool is `0`/`1`,
//! int is a whole number; strings travel beside them (see `text`). Port and parameter indexes follow `block.toml` order.

use crab_blocks::b;
pub use crab_blocks::{Behavior, Ctx};

/// Nothing to compute: output blocks, whose inputs are only shown.
/// How long the pulse of an MQTT block stays on in simulated time, ms: longer than
/// a scan, or the eye would not catch it.
pub const PULSE_MS: u64 = 300;

/// `sent` pulses when the value changed while `en` is on (a simplified "on change"
/// publication; the real block also has a period and a dead band).
#[derive(Default)]
struct MqttPublish {
    last: Option<f64>,
    until_ms: u64,
}

impl Behavior for MqttPublish {
    fn step(&mut self, c: &Ctx, o: &mut [f64]) {
        // Nothing leaves without the network and the broker; the value waits for them.
        if c.bench.offline() || c.bench.broker_down() {
            o[0] = b(c.now_ms < self.until_ms);
            return;
        }
        let value = c.input(1);
        if c.flag(0) && self.last != Some(value) {
            self.until_ms = c.now_ms + PULSE_MS;
        }
        if c.flag(0) {
            self.last = Some(value);
        }
        o[0] = b(c.now_ms < self.until_ms);
    }
}

struct Sink;

impl Behavior for Sink {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}
}

/// Behaviour of a built-in block, or `None` for a type this module does not know
/// (user blocks): their outputs are set by hand or by `block.wasm`.
pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    match kind {
        // Web page widgets that only show a value: nothing to compute.
        "web_button" | "web_chart" | "web_clock" | "web_gauge" | "web_icon" | "web_image"
        | "web_label" | "web_led" | "web_shape" | "web_tank" | "web_text" | "web_value" => {
            Some(Box::new(Sink))
        }
        "mqtt_publish" => Some(Box::<MqttPublish>::default()),
        _ => crab_blocks::behavior(kind),
    }
}
