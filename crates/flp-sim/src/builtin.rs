//! Built-in blocks computed by Rust code. Every value is `f64`: bool is `0`/`1`,
//! int is a whole number; strings travel beside them (see `text`). Port and parameter indexes follow `block.toml` order.

/// What a block sees during one step.
pub struct Ctx<'a> {
    /// Simulated time since the start, already advanced by `dt_ms`.
    pub now_ms: u64,
    /// Values of the inputs in `block.toml` order.
    pub inputs: &'a [f64],
    /// Which inputs exist for the current parameters (`when` conditions).
    pub active: &'a [bool],
    pub params: &'a [f64],
    /// Texts of the inputs; empty for ports that are not strings.
    pub texts: &'a [String],
    /// Texts of the parameters; empty for those that are not strings.
    pub param_texts: &'a [String],
}

impl Ctx<'_> {
    fn flag(&self, i: usize) -> bool {
        self.inputs.get(i).is_some_and(|v| *v != 0.0)
    }

    fn input(&self, i: usize) -> f64 {
        self.inputs.get(i).copied().unwrap_or(0.0)
    }

    fn param(&self, i: usize) -> f64 {
        self.params.get(i).copied().unwrap_or(0.0)
    }

    fn text(&self, i: usize) -> &str {
        self.texts.get(i).map_or("", String::as_str)
    }

    fn param_text(&self, i: usize) -> &str {
        self.param_texts.get(i).map_or("", String::as_str)
    }

    fn live_flags(&self) -> impl Iterator<Item = bool> + '_ {
        self.inputs
            .iter()
            .zip(self.active)
            .filter(|(_, a)| **a)
            .map(|(v, _)| *v != 0.0)
    }
}

/// Behaviour of one block instance; keeps its own state between steps.
pub trait Behavior {
    /// `out` holds the previous outputs, so a block may keep a value by not writing it.
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]);

    /// Blocks with string ports: `out` is the same slice `step` got, `out_text` the
    /// texts of the outputs (only string ports are written).
    fn step_text(&mut self, _ctx: &Ctx, _out: &mut [f64], _out_text: &mut [String]) {}
}

fn b(v: bool) -> f64 {
    f64::from(u8::from(v))
}

/// A block without state.
struct Plain(fn(&Ctx, &mut [f64]));

impl Behavior for Plain {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        (self.0)(ctx, out);
    }
}

/// Nothing to compute: output blocks, whose inputs are only shown.
/// `sent` pulses for one scan when the value changed while `en` is on (a simplified
/// "on change" publication; the real block also has a period and a dead band).
#[derive(Default)]
struct MqttPublish {
    last: Option<f64>,
}

impl Behavior for MqttPublish {
    fn step(&mut self, c: &Ctx, o: &mut [f64]) {
        let value = c.input(1);
        o[0] = b(c.flag(0) && self.last != Some(value));
        if c.flag(0) {
            self.last = Some(value);
        }
    }
}

struct Sink;

impl Behavior for Sink {
    fn step(&mut self, _: &Ctx, _: &mut [f64]) {}
}

/// Behaviour of a built-in block, or `None` for a type this module does not know
/// (hardware blocks, user blocks): their outputs are set by hand or by `block.wasm`.
pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    let plain = |f: fn(&Ctx, &mut [f64])| -> Option<Box<dyn Behavior>> { Some(Box::new(Plain(f))) };
    match kind {
        "and" => plain(|c, o| o[0] = b(c.live_flags().all(|v| v))),
        "or" => plain(|c, o| o[0] = b(c.live_flags().any(|v| v))),
        "xor" => plain(|c, o| o[0] = b(c.live_flags().filter(|v| *v).count() % 2 == 1)),
        "not" => plain(|c, o| o[0] = b(!c.flag(0))),
        "rs_trigger" => plain(|c, o| {
            // Reset wins.
            if c.flag(1) {
                o[0] = 0.0;
            } else if c.flag(0) {
                o[0] = 1.0;
            }
        }),
        "sr_trigger" => plain(|c, o| {
            // Set wins.
            if c.flag(0) {
                o[0] = 1.0;
            } else if c.flag(1) {
                o[0] = 0.0;
            }
        }),
        "t_trigger" => Some(Box::<TTrigger>::default()),
        "rising_edge" => Some(Box::new(Edge {
            rising: true,
            prev: false,
        })),
        "falling_edge" => Some(Box::new(Edge {
            rising: false,
            prev: false,
        })),
        "timer_on" => Some(Box::<TimerOn>::default()),
        "timer_off" => Some(Box::<TimerOff>::default()),
        "pulse" => Some(Box::<Pulse>::default()),
        "generator" => Some(Box::<Generator>::default()),
        "debounce" => Some(Box::<Debounce>::default()),
        "counter" => Some(Box::<Counter>::default()),
        "hysteresis" => plain(|c, o| {
            let (x, on, off) = (c.input(0), c.param(0), c.param(1));
            if on >= off {
                if x >= on {
                    o[0] = 1.0;
                } else if x <= off {
                    o[0] = 0.0;
                }
            } else if x <= on {
                o[0] = 1.0;
            } else if x >= off {
                o[0] = 0.0;
            }
        }),
        "compare" => plain(|c, o| {
            let (x, y) = (c.input(0), c.input(1));
            o[0] = b(match c.param(0) as i64 {
                0 => x == y,
                1 => x != y,
                2 => x < y,
                3 => x <= y,
                4 => x > y,
                _ => x >= y,
            });
        }),
        "greater" => plain(|c, o| o[0] = b(c.input(0) > c.param(0))),
        "less" => plain(|c, o| o[0] = b(c.input(0) < c.param(0))),
        "add" => plain(|c, o| o[0] = c.input(0) + c.input(1)),
        "sub" => plain(|c, o| o[0] = c.input(0) - c.input(1)),
        "mul" => plain(|c, o| o[0] = c.input(0) * c.input(1)),
        "div" => plain(|c, o| {
            o[0] = if c.input(1) != 0.0 {
                c.input(0) / c.input(1)
            } else {
                0.0
            }
        }),
        "mod" => plain(|c, o| {
            o[0] = if c.input(1) != 0.0 {
                c.input(0) % c.input(1)
            } else {
                0.0
            }
        }),
        "min" => plain(|c, o| o[0] = c.input(0).min(c.input(1))),
        "max" => plain(|c, o| o[0] = c.input(0).max(c.input(1))),
        "limit" => plain(|c, o| {
            o[0] = c
                .input(0)
                .clamp(c.param(0).min(c.param(1)), c.param(1).max(c.param(0)))
        }),
        "scale" => plain(|c, o| {
            let (in_min, in_max, out_min, out_max) =
                (c.param(0), c.param(1), c.param(2), c.param(3));
            let span = in_max - in_min;
            let mut y = if span != 0.0 {
                out_min + (c.input(0) - in_min) * (out_max - out_min) / span
            } else {
                out_min
            };
            if c.param(4) != 0.0 {
                y = y.clamp(out_min.min(out_max), out_min.max(out_max));
            }
            o[0] = y;
        }),
        "constant" => plain(|c, o| o[0] = c.param(0)),
        "switch" => plain(|c, o| o[0] = if c.flag(0) { c.input(2) } else { c.input(1) }),
        "mux" => plain(|c, o| {
            // Inputs after `sel` that exist for the current port count.
            let count = c.active.iter().skip(1).filter(|a| **a).count().max(1);
            let idx = (c.input(0).max(0.0) as usize).min(count - 1);
            o[0] = c.input(1 + idx);
        }),
        "digital_out" | "pwm_out" => Some(Box::new(Sink)),
        // Web page widgets that only show a value: nothing to compute.
        "web_button" | "web_chart" | "web_clock" | "web_gauge" | "web_icon" | "web_image"
        | "web_label" | "web_led" | "web_shape" | "web_tank" | "web_text" | "web_value" => {
            Some(Box::new(Sink))
        }
        "mqtt_publish" => Some(Box::<MqttPublish>::default()),
        kind if text::is_text_block(kind) => Some(Box::new(text::TextBlock(kind.to_owned()))),
        _ => None,
    }
}

mod text;

#[derive(Default)]
struct TTrigger {
    prev: bool,
}

impl Behavior for TTrigger {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(1) {
            out[0] = 0.0;
        } else if ctx.flag(0) && !self.prev {
            out[0] = b(out[0] == 0.0);
        }
        self.prev = ctx.flag(0);
    }
}

struct Edge {
    rising: bool,
    prev: bool,
}

impl Behavior for Edge {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = ctx.flag(0);
        out[0] = b(if self.rising {
            now && !self.prev
        } else {
            !now && self.prev
        });
        self.prev = now;
    }
}

#[derive(Default)]
struct TimerOn {
    running: bool,
    start: u64,
}

impl Behavior for TimerOn {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            if !self.running {
                self.running = true;
                self.start = ctx.now_ms;
            }
            out[0] = b((ctx.now_ms - self.start) as f64 >= ctx.param(0));
        } else {
            self.running = false;
            out[0] = 0.0;
        }
    }
}

#[derive(Default)]
struct TimerOff {
    on: bool,
    off_at: u64,
}

impl Behavior for TimerOff {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            self.on = true;
            out[0] = 1.0;
        } else {
            if self.on {
                self.on = false;
                self.off_at = ctx.now_ms;
            }
            out[0] = b(out[0] != 0.0 && ((ctx.now_ms - self.off_at) as f64) < ctx.param(0));
        }
    }
}

#[derive(Default)]
struct Pulse {
    prev: bool,
    active: bool,
    start: u64,
}

impl Behavior for Pulse {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = ctx.flag(0);
        if now && !self.prev && !self.active {
            self.active = true;
            self.start = ctx.now_ms;
        }
        self.prev = now;
        if self.active && (ctx.now_ms - self.start) as f64 >= ctx.param(0) {
            self.active = false;
        }
        out[0] = b(self.active);
    }
}

#[derive(Default)]
struct Generator {
    start: u64,
}

impl Behavior for Generator {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            let period = (ctx.param(0) + ctx.param(1)).max(1.0);
            let phase = ((ctx.now_ms - self.start) as f64) % period;
            out[0] = b(phase < ctx.param(0));
        } else {
            self.start = ctx.now_ms;
            out[0] = 0.0;
        }
    }
}

#[derive(Default)]
struct Debounce {
    last: bool,
    changed: u64,
}

impl Behavior for Debounce {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = ctx.flag(0);
        if now != self.last {
            self.last = now;
            self.changed = ctx.now_ms;
        }
        if (ctx.now_ms - self.changed) as f64 >= ctx.param(0) {
            out[0] = b(self.last);
        }
    }
}

#[derive(Default)]
struct Counter {
    up: bool,
    down: bool,
}

impl Behavior for Counter {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(2) {
            out[0] = 0.0;
        } else {
            if ctx.flag(0) && !self.up {
                out[0] += 1.0;
            }
            if ctx.flag(1) && !self.down {
                out[0] -= 1.0;
            }
        }
        self.up = ctx.flag(0);
        self.down = ctx.flag(1);
    }
}
