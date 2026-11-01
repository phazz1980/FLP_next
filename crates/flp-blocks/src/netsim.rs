//! Simulation of the network and Modbus blocks: the link, the broker and the slaves come
//! from the virtual bench, so the scheme sees an outage the way it would on the board.

use crate::{b, Behavior, Ctx};

/// Address the access point of the board has in the simulation.
const AP_ADDRESS: &str = "192.168.4.1";
/// Address a DHCP client gets in the simulation.
const DHCP_ADDRESS: &str = "192.168.1.100";

/// Wi-Fi client and/or access point. Outputs: `ready`, `ap_ready`, `address`, `status`, `error`.
pub struct Wifi;

impl Behavior for Wifi {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let mut text = vec![String::new(); out.len()];
        self.step_text(c, out, &mut text);
    }

    fn step_text(&mut self, c: &Ctx, out: &mut [f64], text: &mut [String]) {
        let mode = c.param(0) as i64;
        let client = mode != 1;
        let ap = mode != 0;
        // The `En` input is only there when it is switched on.
        let on = !c.active.first().copied().unwrap_or(false) || c.flag(0);
        let up = on && client && !c.bench.offline();
        let address = if up {
            if c.param(3) != 0.0 {
                DHCP_ADDRESS
            } else {
                c.param_text(4)
            }
        } else if on && ap && !client {
            AP_ADDRESS
        } else {
            ""
        };
        let status = if !on {
            0.0
        } else if up || (ap && !client) {
            3.0
        } else {
            2.0
        };
        let error = if on && client && c.bench.offline() {
            1.0
        } else {
            0.0
        };
        let values = [b(up), b(on && ap), 0.0, status, error];
        for (o, v) in out.iter_mut().zip(values) {
            *o = v;
        }
        if let Some(t) = text.get_mut(2) {
            *t = address.to_owned();
        }
    }
}

/// Ethernet: `ready` while the cable is in.
pub struct Ethernet;

impl Behavior for Ethernet {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        if let Some(o) = out.first_mut() {
            *o = b(!c.bench.offline());
        }
    }
}

/// MQTT client: connected while the network is up and the broker answers.
pub struct Mqtt;

impl Behavior for Mqtt {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let (connected, error) = if c.bench.offline() {
            (false, 15.0)
        } else if c.bench.broker_down() {
            (false, 7.0)
        } else {
            (true, 0.0)
        };
        if out.len() >= 2 {
            out[0] = b(connected);
            out[1] = error;
        }
    }
}

/// Read of a register of a Modbus slave: the value from the bench, `ok` off while the slave
/// is silent (the last value stays, as in the firmware).
pub struct ModbusMasterRead;

impl Behavior for ModbusMasterRead {
    fn step(&mut self, c: &Ctx, out: &mut [f64]) {
        let slave = c.param(0) as i64;
        let ok = !c.bench.silent(slave);
        if out.len() < 4 {
            return;
        }
        if ok {
            let v = c.bench.register(slave, c.param_text(1), c.param(3) as i64);
            out[0] = v.round();
            out[1] = v;
            out[2] = b(v != 0.0);
        }
        out[3] = b(ok);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bench;

    fn ctx<'a>(
        bench: &'a Bench,
        params: &'a [f64],
        param_texts: &'a [String],
        active: &'a [bool],
        inputs: &'a [f64],
    ) -> Ctx<'a> {
        Ctx {
            now_ms: 0,
            inputs,
            active,
            params,
            texts: &[],
            param_texts,
            bench,
        }
    }

    #[test]
    fn wifi_follows_the_link() {
        let mut bench = Bench::new();
        let (mut out, mut text) = ([0.0; 5], vec![String::new(); 5]);
        // Client with DHCP, no `En` input.
        let params = [0.0, 0.0, 0.0, 1.0, 0.0];
        let texts = vec![String::new(); 5];
        let c = ctx(&bench, &params, &texts, &[], &[]);
        Wifi.step(&c, &mut out);
        Wifi.step_text(&c, &mut out, &mut text);
        assert_eq!((out[0], out[3], out[4]), (1.0, 3.0, 0.0));
        assert_eq!(text[2], DHCP_ADDRESS);
        bench.set_offline(true);
        let c = ctx(&bench, &params, &texts, &[], &[]);
        Wifi.step(&c, &mut out);
        Wifi.step_text(&c, &mut out, &mut text);
        assert_eq!((out[0], out[3], out[4]), (0.0, 2.0, 1.0));
        assert_eq!(text[2], "");
    }

    #[test]
    fn wifi_en_input_switches_the_radio_off() {
        let bench = Bench::new();
        let mut out = [0.0; 5];
        let params = [0.0, 0.0, 0.0, 1.0, 0.0];
        let texts = vec![String::new(); 5];
        let c = ctx(&bench, &params, &texts, &[true], &[0.0]);
        Wifi.step(&c, &mut out);
        assert_eq!((out[0], out[3]), (0.0, 0.0));
    }

    #[test]
    fn mqtt_needs_the_link_and_the_broker() {
        let mut bench = Bench::new();
        let mut out = [0.0; 2];
        Mqtt.step(&ctx(&bench, &[], &[], &[], &[]), &mut out);
        assert_eq!(out, [1.0, 0.0]);
        bench.set_broker_down(true);
        Mqtt.step(&ctx(&bench, &[], &[], &[], &[]), &mut out);
        assert_eq!(out, [0.0, 7.0]);
        bench.set_offline(true);
        Mqtt.step(&ctx(&bench, &[], &[], &[], &[]), &mut out);
        assert_eq!(out, [0.0, 15.0]);
    }

    #[test]
    fn modbus_read_gives_the_register_and_keeps_it_when_silent() {
        let mut bench = Bench::new();
        bench.set_register(3, "input", 7, 21.5);
        // slave, table, kind, reg
        let params = [3.0, 0.0, 0.0, 7.0];
        let texts = vec![
            String::new(),
            "input".to_owned(),
            String::new(),
            String::new(),
        ];
        let mut out = [0.0; 4];
        ModbusMasterRead.step(&ctx(&bench, &params, &texts, &[], &[]), &mut out);
        assert_eq!(out, [22.0, 21.5, 1.0, 1.0]);
        bench.set_silent(3, true);
        bench.set_register(3, "input", 7, 99.0);
        ModbusMasterRead.step(&ctx(&bench, &params, &texts, &[], &[]), &mut out);
        assert_eq!(out, [22.0, 21.5, 1.0, 0.0]);
    }
}
