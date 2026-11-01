//! The virtual bench: the world outside the board that blocks with pins and sensors see
//! in simulation. Input pins carry a level set by hand, sensors a signal over time, and
//! output blocks leave their pin state here so the editor can show it.

use std::collections::BTreeMap;

/// A sensor reading: constant, or a straight line from `from` to `to` over `duration_ms`
/// counted from `start_ms`; `fault` makes the sensor "not found".
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    pub from: f64,
    pub to: f64,
    pub duration_ms: u64,
    pub start_ms: u64,
    pub fault: bool,
}

impl Signal {
    pub fn constant(value: f64) -> Self {
        Self {
            from: value,
            to: value,
            duration_ms: 0,
            start_ms: 0,
            fault: false,
        }
    }

    /// The reading at the simulated time `now_ms`; after the ramp it stays at `to`.
    pub fn at(&self, now_ms: u64) -> f64 {
        if self.duration_ms == 0 {
            return self.to;
        }
        let done = now_ms.saturating_sub(self.start_ms).min(self.duration_ms);
        self.from + (self.to - self.from) * done as f64 / self.duration_ms as f64
    }
}

/// What a sensor shows until it is set by hand.
fn default_signal(channel: &str) -> Signal {
    Signal::constant(if channel == "humidity" { 50.0 } else { 25.0 })
}

#[derive(Clone, Debug, Default)]
pub struct Bench {
    levels: BTreeMap<i64, f64>,
    signals: BTreeMap<(i64, String), Signal>,
    driven: BTreeMap<i64, f64>,
}

impl Bench {
    pub const fn new() -> Self {
        Self {
            levels: BTreeMap::new(),
            signals: BTreeMap::new(),
            driven: BTreeMap::new(),
        }
    }

    /// Level set on an input pin: `0`/`1` for a digital pin, counts for an analog one.
    pub fn level(&self, pin: i64) -> Option<f64> {
        self.levels.get(&pin).copied()
    }

    pub fn set_level(&mut self, pin: i64, value: f64) {
        self.levels.insert(pin, value);
    }

    pub fn signal(&self, pin: i64, channel: &str) -> Signal {
        self.signals
            .get(&(pin, channel.to_owned()))
            .cloned()
            .unwrap_or_else(|| default_signal(channel))
    }

    pub fn set_signal(&mut self, pin: i64, channel: &str, signal: Signal) {
        self.signals.insert((pin, channel.to_owned()), signal);
    }

    /// What a sensor channel reads at `now_ms`: (value, fault).
    pub fn read(&self, pin: i64, channel: &str, now_ms: u64) -> (f64, bool) {
        let s = self.signal(pin, channel);
        (s.at(now_ms), s.fault)
    }

    /// State an output block put on its pin: `0`/`1`, or the duty in percent.
    pub fn driven(&self, pin: i64) -> Option<f64> {
        self.driven.get(&pin).copied()
    }

    pub fn set_driven(&mut self, pin: i64, value: f64) {
        self.driven.insert(pin, value);
    }

    pub fn clear_driven(&mut self) {
        self.driven.clear();
    }
}

/// A bench with nothing set, for callers that simulate a block alone.
pub static NO_BENCH: Bench = Bench::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_runs_between_the_ends_and_holds() {
        let s = Signal {
            from: 10.0,
            to: 20.0,
            duration_ms: 1000,
            start_ms: 500,
            fault: false,
        };
        assert_eq!(s.at(0), 10.0);
        assert_eq!(s.at(1000), 15.0);
        assert_eq!(s.at(9999), 20.0);
        assert_eq!(Signal::constant(7.0).at(123), 7.0);
    }

    #[test]
    fn unset_sensor_shows_room_values() {
        let b = Bench::new();
        assert_eq!(b.read(4, "temp", 0), (25.0, false));
        assert_eq!(b.read(4, "humidity", 0), (50.0, false));
    }
}
