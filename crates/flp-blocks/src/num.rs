//! The numeric model of the blocks (`specs/numeric-model`): `bool`/`int` are `i32` with wrapping
//! arithmetic, `float` is `f32`. Values travel as `f64` between blocks, which holds both exactly;
//! the arithmetic itself is done here in 32 bits, so the result is the same as on the board.

use flp_core::{ParamKind, SignalType};

/// A 32-bit number of the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum V {
    I(i32),
    F(f32),
}

impl V {
    /// As `float`: an `int` widens (rounding to the nearest `f32`).
    pub fn f(self) -> f32 {
        match self {
            V::I(i) => i as f32,
            V::F(f) => f,
        }
    }

    /// The carrier value between blocks.
    pub fn out(self) -> f64 {
        match self {
            V::I(i) => f64::from(i),
            V::F(f) => f64::from(f),
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, V::F(_))
    }
}

/// Real types of the ports and kinds of the parameters of one block (in `block.toml` order).
/// Missing entries count as `float`.
#[derive(Debug, Clone, Default)]
pub struct Types {
    pub ins: Vec<SignalType>,
    pub outs: Vec<SignalType>,
    pub params: Vec<ParamKind>,
}

impl Types {
    pub fn input(&self, i: usize, x: f64) -> V {
        match self.ins.get(i) {
            Some(SignalType::Int | SignalType::Bool) => V::I(x as i32),
            _ => V::F(x as f32),
        }
    }

    pub fn param(&self, i: usize, x: f64) -> V {
        match self.params.get(i) {
            Some(ParamKind::Float) | None => V::F(x as f32),
            Some(_) => V::I(x as i32),
        }
    }

    /// Output `i` as a value of its type: `int`/`bool` are `i32` (saturating from a wider
    /// number), `float` is `f32`; other types (text) are left alone.
    pub fn quantize(&self, i: usize, v: f64) -> f64 {
        match self.outs.get(i) {
            Some(SignalType::Int | SignalType::Bool) => f64::from(v as i32),
            Some(SignalType::Float) => f64::from(v as f32),
            _ => v,
        }
    }
}

/// Both operands in the wider type: `float` if either is.
fn widen(a: V, b: V) -> (V, V) {
    if a.is_float() || b.is_float() {
        (V::F(a.f()), V::F(b.f()))
    } else {
        (a, b)
    }
}

pub fn add(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(x.wrapping_add(y)),
        (x, y) => V::F(x.f() + y.f()),
    }
}

pub fn sub(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(x.wrapping_sub(y)),
        (x, y) => V::F(x.f() - y.f()),
    }
}

pub fn mul(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(x.wrapping_mul(y)),
        (x, y) => V::F(x.f() * y.f()),
    }
}

/// Division is always `float`; a zero divisor gives 0 (the block's documented behaviour).
pub fn div(a: V, b: V) -> V {
    let (x, y) = (a.f(), b.f());
    V::F(if y != 0.0 { x / y } else { 0.0 })
}

/// Remainder with the sign of the dividend; a zero divisor gives 0.
pub fn rem(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(if y != 0 { x.wrapping_rem(y) } else { 0 }),
        (x, y) => V::F(if y.f() != 0.0 { x.f() % y.f() } else { 0.0 }),
    }
}

/// `a < b ? a : b`: the same select on every target (NaN and zeros included).
pub fn min(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(x.min(y)),
        (x, y) => V::F(if x.f() < y.f() { x.f() } else { y.f() }),
    }
}

/// `a > b ? a : b`.
pub fn max(a: V, b: V) -> V {
    match widen(a, b) {
        (V::I(x), V::I(y)) => V::I(x.max(y)),
        (x, y) => V::F(if x.f() > y.f() { x.f() } else { y.f() }),
    }
}

/// Clamp into `[min(lo, hi), max(lo, hi)]`; the result has the type of `x`.
pub fn limit(x: V, lo: V, hi: V) -> V {
    let (low, high) = (min(lo, hi), max(lo, hi));
    let y = min(max(x, low), high);
    match x {
        V::I(_) => V::I(to_int(y)),
        V::F(_) => V::F(y.f()),
    }
}

/// `float` → `int` with saturation, NaN → 0.
pub fn to_int(v: V) -> i32 {
    match v {
        V::I(i) => i,
        V::F(f) => f as i32,
    }
}

/// Comparison `op`: 0 `==`, 1 `!=`, 2 `<`, 3 `<=`, 4 `>`, else `>=`. Compares with NaN are false
/// (except `!=`, as in IEEE).
pub fn compare(a: V, b: V, op: i64) -> bool {
    match widen(a, b) {
        (V::I(x), V::I(y)) => match op {
            0 => x == y,
            1 => x != y,
            2 => x < y,
            3 => x <= y,
            4 => x > y,
            _ => x >= y,
        },
        (x, y) => {
            let (x, y) = (x.f(), y.f());
            match op {
                0 => x == y,
                1 => x != y,
                2 => x < y,
                3 => x <= y,
                4 => x > y,
                _ => x >= y,
            }
        }
    }
}

/// Linear scaling in `float`, the operations in the order the generated code does them.
pub fn scale(x: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32, clamp: bool) -> f32 {
    let span = in_max - in_min;
    let mut y = if span != 0.0 {
        out_min + (x - in_min) * (out_max - out_min) / span
    } else {
        out_min
    };
    if clamp {
        let lo = if out_min < out_max { out_min } else { out_max };
        let hi = if out_min > out_max { out_min } else { out_max };
        y = if y < lo {
            lo
        } else if y > hi {
            hi
        } else {
            y
        };
    }
    y
}

/// Milliseconds since `start` as an unsigned 32-bit difference: right after the clock wraps (49 days).
pub fn elapsed(now_ms: u64, start: u32) -> u32 {
    (now_ms as u32).wrapping_sub(start)
}

/// A time parameter in milliseconds (negative counts as 0).
pub fn ms(param: f64) -> u32 {
    param as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{behavior, Ctx};

    fn int(v: i32) -> V {
        V::I(v)
    }

    #[test]
    fn int_arithmetic_wraps() {
        assert_eq!(add(int(i32::MAX), int(1)), int(i32::MIN));
        assert_eq!(sub(int(i32::MIN), int(1)), int(i32::MAX));
        assert_eq!(mul(int(65536), int(65536)), int(0));
    }

    #[test]
    fn mixed_operands_are_float() {
        assert_eq!(add(int(16_777_217), V::F(0.0)), V::F(16_777_216.0));
        assert_eq!(add(int(1), V::F(0.5)), V::F(1.5));
    }

    #[test]
    fn division_and_remainder_edges() {
        assert_eq!(div(int(1), int(0)), V::F(0.0));
        assert_eq!(div(int(1), int(4)), V::F(0.25));
        assert_eq!(rem(int(7), int(0)), int(0));
        assert_eq!(rem(int(-7), int(3)), int(-1)); // sign of the dividend
        assert_eq!(rem(int(i32::MIN), int(-1)), int(0));
        assert_eq!(rem(V::F(-7.5), V::F(2.0)), V::F(-1.5));
        assert_eq!(rem(V::F(1.0), V::F(0.0)), V::F(0.0));
    }

    #[test]
    fn nan_compares_false_and_selects_are_fixed() {
        let nan = V::F(f32::NAN);
        for op in [0, 2, 3, 4, 5] {
            assert!(!compare(nan, V::F(1.0), op), "op {op}");
        }
        // min/max are `a < b ? a : b` and `a > b ? a : b`: NaN picks the second operand.
        assert_eq!(min(nan, V::F(1.0)), V::F(1.0));
        assert!(matches!(min(V::F(1.0), nan), V::F(x) if x.is_nan()));
        assert_eq!(limit(int(15), int(0), int(10)), int(10));
        assert_eq!(limit(int(5), int(10), int(0)), int(5));
    }

    #[test]
    fn float_to_int_saturates() {
        assert_eq!(to_int(V::F(1e20)), i32::MAX);
        assert_eq!(to_int(V::F(-1e20)), i32::MIN);
        assert_eq!(to_int(V::F(f32::NAN)), 0);
        assert_eq!(to_int(V::F(-2.9)), -2);
    }

    #[test]
    fn scale_rounds_every_operation_in_f32() {
        // The same expression in f64 gives another result, so the order of f32 operations matters.
        let y = scale(0.1, 0.0, 0.3, 0.0, 100.0, false);
        let expected = 0.0f32 + (0.1f32 - 0.0) * (100.0f32 - 0.0) / (0.3f32 - 0.0);
        assert_eq!(y.to_bits(), expected.to_bits());
        assert_eq!(scale(5.0, 0.0, 0.0, 7.0, 9.0, true), 7.0); // empty span
    }

    fn step(b: &mut dyn crate::Behavior, now_ms: u64, input: f64, preset: f64) -> f64 {
        let inputs = [input];
        let params = [preset];
        let active = [true];
        let ctx = Ctx {
            now_ms,
            inputs: &inputs,
            active: &active,
            params: &params,
            texts: &[],
            param_texts: &[],
            bench: &crate::NO_BENCH,
        };
        let mut out = [0.0];
        b.step(&ctx, &mut out);
        out[0]
    }

    #[test]
    fn on_delay_survives_the_clock_wrap() {
        let mut t = behavior("timer_on").unwrap();
        let start = u64::from(u32::MAX) - 400; // 49 days of uptime, 400 ms before the wrap
        assert_eq!(step(t.as_mut(), start, 1.0, 1000.0), 0.0);
        assert_eq!(step(t.as_mut(), start + 500, 1.0, 1000.0), 0.0);
        assert_eq!(step(t.as_mut(), start + 1000, 1.0, 1000.0), 1.0); // past the wrap
    }

    #[test]
    fn counter_wraps_like_i32() {
        let mut c = behavior("counter").unwrap();
        let mut out = [f64::from(i32::MAX)];
        let inputs = [1.0, 0.0, 0.0];
        let active = [true; 3];
        let ctx = Ctx {
            now_ms: 0,
            inputs: &inputs,
            active: &active,
            params: &[],
            texts: &[],
            param_texts: &[],
            bench: &crate::NO_BENCH,
        };
        c.step(&ctx, &mut out);
        assert_eq!(out[0], f64::from(i32::MIN));
    }
}
