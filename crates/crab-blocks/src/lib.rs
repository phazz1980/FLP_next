//! Built-in blocks without hardware, computed by Rust code. Every value is `f64`: bool is `0`/`1`,
//! int is a whole number; strings travel beside them (see `text`). Port and parameter indexes follow `block.toml` order.

mod bench;
mod cpp;
mod hardware;
mod more;
mod net;
mod netsim;
mod network;
mod num;
mod text;
mod uart;
mod web;

pub use bench::{Bench, Signal, NO_BENCH};
pub use cpp::{CppCtx, CppParts, Raw, UartCtx};
pub use hardware::ds18b20_channel;
pub use more::{next_random, RANDOM_SEED};
pub use netsim::OWN_SLAVE;
pub use num::{Types, V};
pub use web::device_page;

/// A built-in block: one Rust type that both simulates the block and writes its firmware code.
pub trait BuiltinBlock: Sync {
    /// Simulation: a fresh instance with its own state; `None` for a block with hardware.
    fn behavior(&self) -> Option<Box<dyn Behavior>>;
    /// Firmware: the C++ the block adds to the sketch.
    fn emit_cpp(&self, e: &CppCtx) -> CppParts;
}

struct Block {
    kind: &'static str,
    emit: cpp::Emit,
}

impl BuiltinBlock for Block {
    fn behavior(&self) -> Option<Box<dyn Behavior>> {
        behavior(self.kind)
    }

    fn emit_cpp(&self, e: &CppCtx) -> CppParts {
        (self.emit)(e)
    }
}

/// Types that have Rust code for the firmware; the others still use `esp32.cpp.tmpl`.
static BLOCKS: &[Block] = &[
    Block {
        kind: "add",
        emit: cpp::add,
    },
    Block {
        kind: "sub",
        emit: cpp::sub,
    },
    Block {
        kind: "mul",
        emit: cpp::mul,
    },
    Block {
        kind: "div",
        emit: cpp::div,
    },
    Block {
        kind: "mod",
        emit: cpp::modulo,
    },
    Block {
        kind: "min",
        emit: cpp::min,
    },
    Block {
        kind: "max",
        emit: cpp::max,
    },
    Block {
        kind: "not",
        emit: cpp::not,
    },
    Block {
        kind: "and",
        emit: cpp::and,
    },
    Block {
        kind: "or",
        emit: cpp::or,
    },
    Block {
        kind: "xor",
        emit: cpp::xor,
    },
    Block {
        kind: "rs_trigger",
        emit: cpp::rs_trigger,
    },
    Block {
        kind: "sr_trigger",
        emit: cpp::sr_trigger,
    },
    Block {
        kind: "t_trigger",
        emit: cpp::t_trigger,
    },
    Block {
        kind: "rising_edge",
        emit: cpp::rising_edge,
    },
    Block {
        kind: "falling_edge",
        emit: cpp::falling_edge,
    },
    Block {
        kind: "timer_on",
        emit: cpp::timer_on,
    },
    Block {
        kind: "timer_off",
        emit: cpp::timer_off,
    },
    Block {
        kind: "pulse",
        emit: cpp::pulse,
    },
    Block {
        kind: "generator",
        emit: cpp::generator,
    },
    Block {
        kind: "debounce",
        emit: cpp::debounce,
    },
    Block {
        kind: "counter",
        emit: cpp::counter,
    },
    Block {
        kind: "hysteresis",
        emit: cpp::hysteresis,
    },
    Block {
        kind: "compare",
        emit: cpp::compare,
    },
    Block {
        kind: "greater",
        emit: cpp::greater,
    },
    Block {
        kind: "less",
        emit: cpp::less,
    },
    Block {
        kind: "limit",
        emit: cpp::limit,
    },
    Block {
        kind: "scale",
        emit: cpp::scale,
    },
    Block {
        kind: "switch",
        emit: cpp::switch,
    },
    Block {
        kind: "mux",
        emit: cpp::mux,
    },
    Block {
        kind: "constant",
        emit: cpp::constant,
    },
    Block {
        kind: "abs",
        emit: more::abs,
    },
    Block {
        kind: "sq",
        emit: more::sq,
    },
    Block {
        kind: "sqrt",
        emit: more::sqrt,
    },
    Block {
        kind: "pow",
        emit: more::pow,
    },
    Block {
        kind: "sin",
        emit: more::sin,
    },
    Block {
        kind: "cos",
        emit: more::cos,
    },
    Block {
        kind: "tan",
        emit: more::tan,
    },
    Block {
        kind: "float_to_int",
        emit: more::float_to_int,
    },
    Block {
        kind: "int_to_float",
        emit: more::int_to_float,
    },
    Block {
        kind: "random",
        emit: more::random,
    },
    Block {
        kind: "bit_and",
        emit: more::bit_and,
    },
    Block {
        kind: "bit_or",
        emit: more::bit_or,
    },
    Block {
        kind: "bit_xor",
        emit: more::bit_xor,
    },
    Block {
        kind: "bit_not",
        emit: more::bit_not,
    },
    Block {
        kind: "bit_read",
        emit: more::bit_read,
    },
    Block {
        kind: "bit_write",
        emit: more::bit_write,
    },
    Block {
        kind: "bit_change",
        emit: more::bit_change,
    },
    Block {
        kind: "bits_to_byte",
        emit: more::bits_to_byte,
    },
    Block {
        kind: "byte_to_bits",
        emit: more::byte_to_bits,
    },
    Block {
        kind: "bytes_to_word",
        emit: more::bytes_to_word,
    },
    Block {
        kind: "word_to_bytes",
        emit: more::word_to_bytes,
    },
    Block {
        kind: "bytes_to_dword",
        emit: more::bytes_to_dword,
    },
    Block {
        kind: "dword_to_bytes",
        emit: more::dword_to_bytes,
    },
    Block {
        kind: "number_change",
        emit: more::number_change,
    },
    Block {
        kind: "string_change",
        emit: more::string_change,
    },
    Block {
        kind: "pid",
        emit: more::pid,
    },
    Block {
        kind: "scale_table",
        emit: more::scale_table,
    },
    Block {
        kind: "text_const",
        emit: text::text_const,
    },
    Block {
        kind: "text_concat",
        emit: text::text_concat,
    },
    Block {
        kind: "text_length",
        emit: text::text_length,
    },
    Block {
        kind: "num_to_text",
        emit: text::num_to_text,
    },
    Block {
        kind: "text_compare",
        emit: text::text_compare,
    },
    Block {
        kind: "text_to_num",
        emit: text::text_to_num,
    },
    Block {
        kind: "text_substring",
        emit: text::text_substring,
    },
    Block {
        kind: "text_find",
        emit: text::text_find,
    },
    Block {
        kind: "text_char",
        emit: text::text_char,
    },
    Block {
        kind: "text_add_char",
        emit: text::text_add_char,
    },
    Block {
        kind: "ip_to_text",
        emit: text::ip_to_text,
    },
    Block {
        kind: "text_to_ip",
        emit: text::text_to_ip,
    },
    Block {
        kind: "digital_in",
        emit: hardware::digital_in,
    },
    Block {
        kind: "digital_out",
        emit: hardware::digital_out,
    },
    Block {
        kind: "analog_in",
        emit: hardware::analog_in,
    },
    Block {
        kind: "pwm_out",
        emit: hardware::pwm_out,
    },
    Block {
        kind: "dht22",
        emit: hardware::dht22,
    },
    Block {
        kind: "ds18b20",
        emit: hardware::ds18b20,
    },
    Block {
        kind: "flash_store",
        emit: hardware::flash_store,
    },
    Block {
        kind: "dms",
        emit: hardware::dms,
    },
    Block {
        kind: "cycle_report",
        emit: hardware::cycle_report,
    },
    Block {
        kind: "decoder",
        emit: hardware::decoder,
    },
    Block {
        kind: "encoder",
        emit: hardware::encoder,
    },
    Block {
        kind: "pcf8574_in",
        emit: hardware::pcf8574_in,
    },
    Block {
        kind: "pcf8574_out",
        emit: hardware::pcf8574_out,
    },
    Block {
        kind: "uart_config",
        emit: uart::uart_config,
    },
    Block {
        kind: "uart_read",
        emit: uart::uart_read,
    },
    Block {
        kind: "uart_send",
        emit: uart::uart_send,
    },
    Block {
        kind: "modbus_rtu_master",
        emit: uart::modbus_rtu_master,
    },
    Block {
        kind: "modbus_rtu_slave",
        emit: uart::modbus_rtu_slave,
    },
    Block {
        kind: "modbus_tcp_master",
        emit: uart::modbus_tcp_master,
    },
    Block {
        kind: "modbus_tcp_slave",
        emit: uart::modbus_tcp_slave,
    },
    Block {
        kind: "modbus_read",
        emit: uart::modbus_read,
    },
    Block {
        kind: "modbus_write",
        emit: uart::modbus_write,
    },
    Block {
        kind: "modbus_master_read",
        emit: uart::modbus_master_read,
    },
    Block {
        kind: "modbus_master_write",
        emit: uart::modbus_master_write,
    },
    Block {
        kind: "mqtt",
        emit: network::mqtt,
    },
    Block {
        kind: "mqtt_publish",
        emit: network::mqtt_publish,
    },
    Block {
        kind: "mqtt_subscribe",
        emit: network::mqtt_subscribe,
    },
    Block {
        kind: "ntp",
        emit: network::ntp,
    },
    Block {
        kind: "wifi",
        emit: net::wifi,
    },
    Block {
        kind: "ethernet",
        emit: net::ethernet,
    },
];

/// Built-in block of this type, or `None` when it has no Rust code (yet).
pub fn get(kind: &str) -> Option<&'static dyn BuiltinBlock> {
    BLOCKS
        .iter()
        .find(|b| b.kind == kind)
        .map(|b| b as &'static dyn BuiltinBlock)
}

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
    /// The world outside the board: input pin levels, sensor signals.
    pub bench: &'a Bench,
}

impl Ctx<'_> {
    pub fn flag(&self, i: usize) -> bool {
        self.inputs.get(i).is_some_and(|v| *v != 0.0)
    }

    pub fn input(&self, i: usize) -> f64 {
        self.inputs.get(i).copied().unwrap_or(0.0)
    }

    pub fn param(&self, i: usize) -> f64 {
        self.params.get(i).copied().unwrap_or(0.0)
    }

    pub fn text(&self, i: usize) -> &str {
        self.texts.get(i).map_or("", String::as_str)
    }

    pub fn param_text(&self, i: usize) -> &str {
        self.param_texts.get(i).map_or("", String::as_str)
    }

    pub fn live_flags(&self) -> impl Iterator<Item = bool> + '_ {
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

    /// Real port types and parameter kinds of the block, given once after creation.
    fn set_types(&mut self, _: &Types) {}

    /// What an output block puts on its pin after the last step: `0`/`1` or a duty in percent.
    fn drive(&self) -> Option<f64> {
        None
    }
}

pub fn b(v: bool) -> f64 {
    f64::from(u8::from(v))
}

/// A block without state.
struct Plain(fn(&Ctx, &mut [f64]));

impl Behavior for Plain {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        (self.0)(ctx, out);
    }
}

/// A stateless block that computes in the numeric model and so needs the real types.
struct Typed {
    f: fn(&Ctx, &Types, &mut [f64]),
    types: Types,
}

impl Behavior for Typed {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        (self.f)(ctx, &self.types, out);
    }

    fn set_types(&mut self, t: &Types) {
        self.types = t.clone();
    }
}

pub fn behavior(kind: &str) -> Option<Box<dyn Behavior>> {
    let typed = |f: fn(&Ctx, &Types, &mut [f64])| -> Option<Box<dyn Behavior>> {
        Some(Box::new(Typed {
            f,
            types: Types::default(),
        }))
    };
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
        "hysteresis" => typed(|c, t, o| {
            let x = t.input(0, c.input(0)).f();
            let (on, off) = (t.param(0, c.param(0)).f(), t.param(1, c.param(1)).f());
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
        "compare" => typed(|c, t, o| {
            let (x, y) = (t.input(0, c.input(0)), t.input(1, c.input(1)));
            o[0] = b(num::compare(x, y, c.param(0) as i64));
        }),
        "greater" => typed(|c, t, o| {
            o[0] = b(num::compare(
                t.input(0, c.input(0)),
                t.param(0, c.param(0)),
                4,
            ));
        }),
        "less" => typed(|c, t, o| {
            o[0] = b(num::compare(
                t.input(0, c.input(0)),
                t.param(0, c.param(0)),
                2,
            ));
        }),
        "add" => {
            typed(|c, t, o| o[0] = num::add(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "sub" => {
            typed(|c, t, o| o[0] = num::sub(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "mul" => {
            typed(|c, t, o| o[0] = num::mul(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "div" => {
            typed(|c, t, o| o[0] = num::div(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "mod" => {
            typed(|c, t, o| o[0] = num::rem(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "min" => {
            typed(|c, t, o| o[0] = num::min(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "max" => {
            typed(|c, t, o| o[0] = num::max(t.input(0, c.input(0)), t.input(1, c.input(1))).out())
        }
        "limit" => typed(|c, t, o| {
            let (lo, hi) = (t.param(0, c.param(0)), t.param(1, c.param(1)));
            o[0] = num::limit(t.input(0, c.input(0)), lo, hi).out();
        }),
        "scale" => typed(|c, t, o| {
            let p = |i| t.param(i, c.param(i)).f();
            o[0] = f64::from(num::scale(
                t.input(0, c.input(0)).f(),
                p(0),
                p(1),
                p(2),
                p(3),
                c.param(4) != 0.0,
            ));
        }),
        "constant" => plain(|c, o| o[0] = c.param(0)),
        "switch" => plain(|c, o| o[0] = if c.flag(0) { c.input(2) } else { c.input(1) }),
        "mux" => plain(|c, o| {
            // Inputs after `sel` that exist for the current port count.
            let count = c.active.iter().skip(1).filter(|a| **a).count().max(1);
            let idx = (c.input(0).max(0.0) as usize).min(count - 1);
            o[0] = c.input(1 + idx);
        }),
        _ => more::behavior(kind)
            .or_else(|| text::behavior(kind))
            .or_else(|| hardware::behavior(kind)),
    }
}

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
    start: u32,
}

impl Behavior for TimerOn {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            if !self.running {
                self.running = true;
                self.start = ctx.now_ms as u32;
            }
            out[0] = b(num::elapsed(ctx.now_ms, self.start) >= num::ms(ctx.param(0)));
        } else {
            self.running = false;
            out[0] = 0.0;
        }
    }
}

#[derive(Default)]
struct TimerOff {
    on: bool,
    off_at: u32,
}

impl Behavior for TimerOff {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            self.on = true;
            out[0] = 1.0;
        } else {
            if self.on {
                self.on = false;
                self.off_at = ctx.now_ms as u32;
            }
            out[0] =
                b(out[0] != 0.0 && num::elapsed(ctx.now_ms, self.off_at) < num::ms(ctx.param(0)));
        }
    }
}

#[derive(Default)]
struct Pulse {
    prev: bool,
    active: bool,
    start: u32,
}

impl Behavior for Pulse {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = ctx.flag(0);
        if now && !self.prev && !self.active {
            self.active = true;
            self.start = ctx.now_ms as u32;
        }
        self.prev = now;
        if self.active && num::elapsed(ctx.now_ms, self.start) >= num::ms(ctx.param(0)) {
            self.active = false;
        }
        out[0] = b(self.active);
    }
}

#[derive(Default)]
struct Generator {
    start: u32,
}

impl Behavior for Generator {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        if ctx.flag(0) {
            let on = num::ms(ctx.param(0));
            let period = on.wrapping_add(num::ms(ctx.param(1))).max(1);
            let phase = num::elapsed(ctx.now_ms, self.start) % period;
            out[0] = b(phase < on);
        } else {
            self.start = ctx.now_ms as u32;
            out[0] = 0.0;
        }
    }
}

#[derive(Default)]
struct Debounce {
    last: bool,
    changed: u32,
}

impl Behavior for Debounce {
    fn step(&mut self, ctx: &Ctx, out: &mut [f64]) {
        let now = ctx.flag(0);
        if now != self.last {
            self.last = now;
            self.changed = ctx.now_ms as u32;
        }
        if num::elapsed(ctx.now_ms, self.changed) >= num::ms(ctx.param(0)) {
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
            let mut n = out[0] as i32;
            if ctx.flag(0) && !self.up {
                n = n.wrapping_add(1);
            }
            if ctx.flag(1) && !self.down {
                n = n.wrapping_sub(1);
            }
            out[0] = f64::from(n);
        }
        self.up = ctx.flag(0);
        self.down = ctx.flag(1);
    }
}
