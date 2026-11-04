//! Modules written by `flp-codegen` give the same numbers as the simulation of the schema.

use std::path::PathBuf;

use flp_codegen::Program;
use flp_core::{Library, Project, SignalType};
use flp_sim::mcu::{Manifest, McuModule, Word};
use flp_sim::schema::Simulation;

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn library() -> Library {
    let (lib, errors) = Library::load(repo_path("library")).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    lib
}

/// The carrier value `expected` of the signal `label` as the 32-bit word the module must hold.
fn word(program: &Program, label: &str, expected: f64) -> Word {
    let name = label.replace('.', "_");
    let ty = program
        .signals
        .iter()
        .find(|s| s.name == name)
        .map_or(SignalType::Float, |s| s.ty);
    if ty == SignalType::Float {
        Word::float(expected as f32)
    } else {
        Word::int(expected as i32)
    }
}

/// Runs the sheet as a module next to the simulation and compares every output after every step.
fn compare(project_dir: &str, outputs: usize, steps: u32, dt: u32) {
    let lib = library();
    let project = Project::load(repo_path(project_dir)).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    assert!(built.watched.len() == outputs, "{:?}", built.watched);
    for step in 0..steps {
        sim.step(dt).unwrap();
        module.step(dt).unwrap();
        for (i, label) in built.watched.iter().enumerate() {
            let (block, port) = label.split_once('.').unwrap();
            let expected = word(&program, label, sim.output(block, port).unwrap());
            let got = module.outputs()[i];
            assert_eq!(got, expected, "step {step}: {label}");
        }
    }
}

#[test]
fn stateless_blocks_match_the_simulation() {
    compare("crates/flp-sim/tests/fixtures/wasm_logic", 20, 40, 100);
}

#[test]
fn timers_and_memory_blocks_match_the_simulation() {
    compare("crates/flp-sim/tests/fixtures/wasm_timers", 10, 120, 50);
}

#[test]
fn pins_match_the_simulation() {
    let lib = library();
    let project = Project::load(repo_path("crates/flp-sim/tests/fixtures/wasm_hw")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(
        manifest.allow,
        ["gpio_read", "gpio_write", "adc_read", "pwm_write"]
    );
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    for step in 0..120u32 {
        // The button is pressed (pulled low) for a while; the level sweeps over the ADC range.
        let pressed = (30..70).contains(&step);
        let counts = (step * 53) % 4400;
        sim.bench_mut()
            .set_level(4, if pressed { 0.0 } else { 1.0 });
        sim.bench_mut().set_level(34, f64::from(counts));
        module.io_mut().gpio_in.insert(4, i32::from(!pressed));
        module.io_mut().adc_in.insert(34, counts as i32);
        sim.step(50).unwrap();
        module.step(50).unwrap();
        for (i, label) in built.watched.iter().enumerate() {
            let (block, port) = label.split_once('.').unwrap();
            let expected = word(&program, label, sim.output(block, port).unwrap());
            assert_eq!(module.outputs()[i], expected, "step {step}: {label}");
        }
        let io = module.io();
        for pin in [2_i64, 5] {
            let want = sim.bench().driven(pin).unwrap_or(0.0) as i32;
            assert_eq!(
                io.gpio_out.get(&(pin as i32)).copied().unwrap_or(0),
                want,
                "step {step}: pin {pin}"
            );
        }
        // Rounded half away from zero, in 32 bits like the module.
        let duty = sim.bench().driven(18).unwrap_or(0.0) as f32;
        let want = (duty * 2.55f32).round() as i32;
        assert_eq!(
            io.pwm_out.get(&18).copied().unwrap_or(0),
            want,
            "step {step}: pwm"
        );
    }
}

#[test]
fn blink_example_drives_the_led_like_the_simulation() {
    let lib = library();
    let project = Project::load(repo_path("examples/blink")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    assert!(built.manifest.contains("allow = [\"gpio_write\"]"));
    let manifest = Manifest::parse(&built.manifest).unwrap();
    let mut module = McuModule::load(&built.wasm, &manifest, 0).unwrap();
    let mut sim = Simulation::new(&project.logic, &lib).unwrap();
    let mut levels = std::collections::BTreeSet::new();
    for step in 0..60 {
        sim.step(50).unwrap();
        module.step(50).unwrap();
        let want = sim.bench().driven(2).unwrap_or(0.0) as i32;
        let got = module.io().gpio_out.get(&2).copied().unwrap_or(0);
        assert_eq!(got, want, "step {step}");
        levels.insert(got);
    }
    assert_eq!(levels.len(), 2, "the led must blink");
}

#[test]
fn numeric_edges_match_the_simulation() {
    // `n` starts 3 below i32::MAX and wraps; division by zero, remainders and mixed operands.
    compare("crates/flp-sim/tests/fixtures/wasm_edges", 13, 12, 100);
}

#[test]
fn sensors_read_what_the_master_reports() {
    let lib = library();
    let project = Project::load(repo_path("crates/flp-sim/tests/fixtures/wasm_sensors")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["sensor_read"]);
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built.watched.iter().position(|l| l == label).unwrap();
        module.outputs()[i]
    };
    // Keys are `kind | index << 4 | pin << 8`; fields: 0 temperature, 1 humidity, 2 read ok.
    let sensors = &mut module.io_mut().sensors;
    sensors.insert((4 << 8, 0), 21.5);
    sensors.insert((4 << 8, 2), 1.0);
    sensors.insert((4 << 8 | 1 << 4, 0), -3.25);
    sensors.insert((4 << 8 | 1 << 4, 2), 1.0);
    sensors.insert((1 | 5 << 8, 0), 24.0);
    sensors.insert((1 | 5 << 8, 1), 55.5);
    sensors.insert((1 | 5 << 8, 2), 1.0);
    module.step(50).unwrap();
    assert_eq!(out(&module, "t1.temp"), Word::float(21.5));
    assert_eq!(out(&module, "t1.ok"), Word::int(1));
    assert_eq!(out(&module, "t2.temp"), Word::float(-3.25));
    assert_eq!(out(&module, "air.temp"), Word::float(24.0));
    assert_eq!(out(&module, "air.humidity"), Word::float(55.5));
    assert_eq!(out(&module, "air.ok"), Word::int(1));
    // A failed read: the master keeps the last temperature and drops `ok`.
    module.io_mut().sensors.insert((4 << 8, 2), 0.0);
    module.step(50).unwrap();
    assert_eq!(out(&module, "t1.temp"), Word::float(21.5));
    assert_eq!(out(&module, "t1.ok"), Word::int(0));
    assert_eq!(out(&module, "t2.ok"), Word::int(1));
}

#[test]
fn pcf8574_goes_through_the_master() {
    let lib = library();
    let project = Project::load(repo_path("crates/flp-sim/tests/fixtures/wasm_pcf")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["pcf_read", "pcf_write"]);
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built.watched.iter().position(|l| l == label).unwrap();
        module.outputs()[i]
    };
    // Keys are `address | sda << 8 | scl << 16`; fields: 0 the byte, 1 the chip answered.
    let inputs = 0x22 | 21 << 8 | 22 << 16;
    let relays = 0x24 | 21 << 8 | 22 << 16;
    // Optocouplers pull to ground: P1 low and the rest high means "second input pressed".
    module.io_mut().pcf_in.insert((inputs, 0), 0b1111_1101);
    module.io_mut().pcf_in.insert((inputs, 1), 1);
    module.step(50).unwrap();
    assert_eq!(out(&module, "inputs.p0"), Word::int(0));
    assert_eq!(out(&module, "inputs.p1"), Word::int(1));
    assert_eq!(out(&module, "inputs.ok"), Word::int(1));
    // Inverted relays: P3 on is a low bit, the rest stay high.
    assert_eq!(module.io().pcf_out[&relays], 0b1111_0111);
    module.io_mut().pcf_in.insert((inputs, 0), 0b1111_1110);
    module.step(50).unwrap();
    assert_eq!(module.io().pcf_out[&relays], 0b1111_1110);
}

#[test]
fn modbus_rtu_goes_through_the_master() {
    let lib = library();
    let project = Project::load(repo_path("examples/modbus_rtu")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["modbus"]);
    let mut module = McuModule::load(&built.wasm, &manifest, 0).unwrap();
    // Master reads: slave 10, input table (1), float (1), master (1); the coil of the panel is a slave read.
    let temp = (10, 1 | 1 << 4 | 1 << 8, 0);
    let coil = (0, 2 | 2 << 4, 0);
    module
        .io_mut()
        .modbus_in
        .insert(temp, 21.5f32.to_bits() as i32);
    module.io_mut().modbus_in.insert(coil, 1);
    module.io_mut().modbus_status.insert(10, 1);
    module.step(50).unwrap();
    let io = module.io();
    // Both roles are set up once, each list starting with its role (code 0) and ending with code 63.
    let roles: Vec<_> = io.modbus_cfg.iter().filter(|(c, _)| *c == 0).collect();
    assert_eq!(roles, [&(0, 1), &(0, 2)]);
    assert_eq!(io.modbus_cfg.iter().filter(|(c, _)| *c == 63).count(), 2);
    assert!(
        io.modbus_cfg.contains(&(2, 9600)),
        "master speed from UART2"
    );
    assert!(
        io.modbus_cfg.contains(&(2, 19200)),
        "slave speed from UART1"
    );
    assert!(io.modbus_cfg.contains(&(16, 10)) && io.modbus_cfg.contains(&(24, 11)));
    // The temperature goes to the panel's input registers, the relay command to slave 11.
    assert_eq!(io.modbus_out[&(0, 1 | 1 << 4, 0)], 21.5f32.to_bits() as i32);
    assert_eq!(io.modbus_out[&(0, 3 | 2 << 4, 0)], 1);
    assert_eq!(io.modbus_out[&(11, 2 | 2 << 4 | 1 << 8, 0)], 1);
    let n = module.io().modbus_cfg.len();
    module.step(50).unwrap();
    assert_eq!(module.io().modbus_cfg.len(), n, "settings are sent once");
}

#[test]
fn sensors_example_switches_pins_from_sensor_values() {
    let lib = library();
    let project = Project::load(repo_path("examples/sensors")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["gpio_write", "sensor_read"]);
    let mut module = McuModule::load(&built.wasm, &manifest, 0).unwrap();
    let (water, air) = (4 << 8, 1 | 15 << 8);
    let sensors = &mut module.io_mut().sensors;
    sensors.extend([((water, 0), 30.0), ((water, 2), 1.0)]);
    sensors.extend([((air, 0), 20.0), ((air, 1), 80.0), ((air, 2), 1.0)]);
    module.step(50).unwrap();
    let pin = |module: &McuModule, p: i32| module.io().gpio_out.get(&p).copied();
    assert_eq!(pin(&module, 26), Some(1), "humid: fan on");
    assert_eq!(pin(&module, 27), Some(1), "cold water: heater on");
    assert_eq!(pin(&module, 2), Some(0), "both sensors ok: no fault");
    module.io_mut().sensors.insert((air, 2), 0.0);
    module.step(50).unwrap();
    assert_eq!(pin(&module, 2), Some(1), "air sensor lost: fault lamp");
}

#[test]
fn uart_bytes_go_through_the_master() {
    let lib = library();
    let project = Project::load(repo_path("examples/uart_bytes")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module(1).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert_eq!(manifest.allow, ["uart"]);
    let mut module = McuModule::load(&built.wasm, &manifest, 0).unwrap();
    module
        .io_mut()
        .uart_rx
        .entry(2)
        .or_default()
        .extend([65, 200]);
    module.step(10).unwrap();
    module.step(10).unwrap();
    let io = module.io();
    // The port is opened once by each of its four blocks, with the speed and frame of the board.
    assert!(io.uart_cfg.contains(&(2, 1, 115200)));
    assert!(io.uart_cfg.contains(&(2, 2, 8)) && io.uart_cfg.contains(&(2, 5, 16)));
    assert_eq!(io.uart_cfg.iter().filter(|c| c.1 == 63).count(), 4);
    assert_eq!(
        io.uart_speed.get(&2),
        Some(&57600),
        "apply switches the speed"
    );
    // Echo: the bytes come back as one byte each; the scaled value goes out as a float.
    let tx = &io.uart_tx[&2];
    let bytes: Vec<_> = tx.iter().filter(|t| t.1 == 0).map(|t| t.0).collect();
    assert_eq!(bytes, [65, 200]);
    let floats: Vec<_> = tx
        .iter()
        .filter(|t| t.1 == 2)
        .map(|t| f32::from_bits(t.0 as u32))
        .collect();
    assert!(
        floats.len() >= 2 && (floats[0] - 25.49).abs() < 0.1,
        "{floats:?}"
    );
    let n = module.io().uart_cfg.len();
    module.step(10).unwrap();
    assert_eq!(module.io().uart_cfg.len(), n, "settings are sent once");
}

#[test]
fn uart_lines_and_strings_work_in_a_module() {
    let lib = library();
    let project = Project::load(repo_path("examples/text_wasm")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert!(manifest.allow.iter().any(|a| a == "text"), "{manifest:?}");
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built.watched.iter().position(|l| l == label).unwrap();
        module.outputs()[i]
    };
    let rx = |module: &mut McuModule, text: &str| {
        module
            .io_mut()
            .uart_rx
            .entry(2)
            .or_default()
            .extend(text.bytes());
    };
    // One line per step, as in the C++ block; CR before LF is skipped.
    rx(&mut module, "25.55\r\nOn\n");
    module.step(10).unwrap();
    assert_eq!(module.io().uart_text[&2], b"T=25.5C\r\n");
    assert_eq!(out(&module, "len.out"), Word::int(5));
    module.step(10).unwrap();
    assert_eq!(module.io().uart_text[&2], b"T=25.5C\r\nT=25.5C\r\n");
    assert_eq!(out(&module, "is_on.out"), Word::int(1));
    assert_eq!(out(&module, "len.out"), Word::int(2));
    // A half line waits for its end.
    rx(&mut module, "7");
    module.step(10).unwrap();
    assert_eq!(out(&module, "len.out"), Word::int(2));
    rx(&mut module, "\n");
    module.step(10).unwrap();
    assert_eq!(out(&module, "len.out"), Word::int(1));
}

#[test]
fn text_blocks_match_the_simulation() {
    compare("crates/flp-sim/tests/fixtures/wasm_text", 19, 3, 100);
}

#[test]
fn wifi_and_ntp_work_in_a_module() {
    let lib = library();
    let project = Project::load(repo_path("examples/net_wasm")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert!(manifest.allow.iter().any(|a| a == "net"), "{manifest:?}");
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built
            .watched
            .iter()
            .position(|l| l == label)
            .unwrap_or_else(|| panic!("{label}: {:?}", built.watched));
        module.outputs()[i]
    };
    module.step(10).unwrap();
    // Settings go out once, the radio request is the client every step.
    let io = module.io();
    assert_eq!(io.net_text[&1], "MyNetwork");
    assert_eq!(io.net_text[&2], "secret123");
    assert_eq!(io.net_text[&8], "pool.ntp.org");
    assert_eq!(io.net_num[&1], 1);
    assert_eq!(io.net_num[&2], 3_600_000);
    assert!(io.net_num.contains_key(&63) && io.net_num.contains_key(&64));
    assert_eq!(io.net_radio, 1);
    assert_eq!(out(&module, "clock.valid"), Word::int(0));
    assert_eq!(out(&module, "addr_len.out"), Word::int(0));
    assert_eq!(out(&module, "work.out"), Word::int(0));
    // Connected, time known: 2026-11-04 (Wednesday) 10:30:15 UTC, zone +3 -> 13:30:15.
    let unix = 1_793_788_215u32;
    let io = module.io_mut();
    io.net_wifi.insert(0, 1);
    io.net_wifi.insert(4, i32::from_le_bytes([192, 168, 1, 77]));
    io.net_unix = Some(unix);
    module.step(10).unwrap();
    assert_eq!(out(&module, "clock.valid"), Word::int(1));
    assert_eq!(out(&module, "clock.hour"), Word::int(13));
    assert_eq!(out(&module, "clock.minute"), Word::int(30));
    assert_eq!(out(&module, "clock.unix"), Word::int(unix as i32));
    assert_eq!(out(&module, "addr_len.out"), Word::int(12));
    assert_eq!(out(&module, "time_len.out"), Word::int(8));
    assert_eq!(out(&module, "work.out"), Word::int(1));
}

#[test]
fn ethernet_and_wifi_work_together_in_a_module() {
    let lib = library();
    let project = Project::load(repo_path("examples/net_eth_wasm")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert!(manifest.allow.iter().any(|a| a == "net"), "{manifest:?}");
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    let out = |module: &McuModule, label: &str| {
        let i = built
            .watched
            .iter()
            .position(|l| l == label)
            .unwrap_or_else(|| panic!("{label}: {:?}", built.watched));
        module.outputs()[i]
    };
    module.step(10).unwrap();
    // The pins, the chip select and the MAC byte go to the master once; DHCP is on.
    let io = module.io();
    assert_eq!(io.eth_num[&2], 14);
    assert_eq!(io.eth_num[&3], 12);
    assert_eq!(io.eth_num[&4], 13);
    assert_eq!(io.eth_num[&5], 15);
    assert_eq!(io.eth_num[&6], 41);
    assert_eq!(io.eth_num[&1], 1);
    assert!(io.eth_num.contains_key(&63));
    assert!(!io.eth_text.contains_key(&1));
    // NTP asks for the Ethernet interface; Wi-Fi settings are independent.
    assert_eq!(io.net_num[&3], 1);
    assert_eq!(io.net_text[&1], "MyNetwork");
    assert_eq!(out(&module, "work.out"), Word::int(0));
    assert_eq!(out(&module, "eth_len.out"), Word::int(0));
    // Both up, time known.
    let io = module.io_mut();
    io.eth_state.insert(0, 1);
    io.eth_state.insert(1, i32::from_le_bytes([10, 0, 0, 5]));
    io.net_wifi.insert(0, 1);
    io.net_wifi.insert(4, i32::from_le_bytes([192, 168, 1, 77]));
    io.net_unix = Some(1_793_788_215);
    module.step(10).unwrap();
    assert_eq!(out(&module, "eth_len.out"), Word::int(8));
    assert_eq!(out(&module, "wifi_len.out"), Word::int(12));
    assert_eq!(out(&module, "work.out"), Word::int(1));
}

#[test]
fn mqtt_goes_through_the_master() {
    let lib = library();
    let project = Project::load(repo_path("examples/mqtt_wasm")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert!(manifest.allow.iter().any(|a| a == "net"), "{manifest:?}");
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    module.step(10).unwrap();
    // Settings go out once, the filter is registered in the first slot.
    let io = module.io();
    assert_eq!(io.mqtt_text[&1], "broker.hivemq.com");
    assert_eq!(io.mqtt_num[&1], 1883);
    assert_eq!(io.mqtt_num[&4], 2);
    assert!(io.mqtt_num.contains_key(&63));
    assert_eq!(io.mqtt_filters, ["flp/led"]);
    // No link, nothing published.
    assert!(io.mqtt_published.is_empty());
    // A message arrives and the module echoes it back once the client is connected.
    let io = module.io_mut();
    io.mqtt_state.insert(0, 1);
    io.mqtt_receive("flp/led", "1");
    module.step(10).unwrap();
    module.step(10).unwrap();
    let io = module.io();
    assert_eq!(
        io.mqtt_published,
        [("flp/echo".to_string(), "1".to_string(), false)]
    );
}

#[test]
fn web_page_goes_through_the_master() {
    let lib = library();
    let project = Project::load(repo_path("examples/web_wasm")).unwrap();
    let program = Program::build(&project, &lib).unwrap();
    let built = program.wasm_module_with(1, true).unwrap();
    let manifest = Manifest::parse(&built.manifest).unwrap();
    assert!(manifest.allow.iter().any(|a| a == "net"), "{manifest:?}");
    let mut module = McuModule::load(&built.wasm, &manifest, built.watched.len()).unwrap();
    module.step(10).unwrap();
    let io = module.io();
    // The setup comes once, before any widget works: layout, table, interface, password.
    assert!(io.web_complete);
    // Wi-Fi (bit 1) and the «Система» page (bit 4).
    assert_eq!(io.web_mask, 5);
    assert_eq!(io.web_pass, "1234");
    assert!(
        io.web_layout.starts_with("{\"t\":\"Модуль\""),
        "{}",
        io.web_layout
    );
    assert!(io.web_layout.contains("\"k\":\"slider\""));
    assert!(io.web_layout.contains("\"k\":\"textin\""));
    assert!(io.web_layout.contains("\"y\":1"));
    assert!(io.web_layout.contains("\"k\":\"chart\""));
    let kinds: Vec<u8> = io.web_items.iter().map(|i| i.kind).collect();
    assert_eq!(kinds, [0, 0, 2, 0, 0, 4, 1, 1, 1]);
    // The network form: defaults from the block, one widget of the table; the saved ssid is shown.
    assert_eq!(io.web_net.len(), 10);
    assert_eq!(io.web_net[0], "home");
    assert_eq!(io.web_net[2], "1");
    // The slider keeps its initial value; the text input its initial text.
    assert_eq!(io.web_items[1].value, 25.0);
    assert_eq!(io.web_items[2].text, "FLP");
    // The chart: 30 points and a period of 2 s go to the master, which samples the value itself.
    assert_eq!(io.web_items[5].points, 30);
    assert_eq!(io.web_items[5].value, 25.0);
    module.step(10).unwrap();
    let io = module.io();
    assert_eq!(io.web_items[3].value, 25.0, "value shown from the slider");
    assert_eq!(io.web_items[6].text, "Привет, FLP");
    assert_eq!(io.web_items[8].text, "home", "ssid from the form");
    // The page changes the switch, the slider and the name.
    let io = module.io_mut();
    io.web_items[0].value = 1.0;
    io.web_items[1].value = 70.0;
    io.web_items[2].text = "Мир".to_string();
    module.step(10).unwrap();
    module.step(10).unwrap();
    let io = module.io();
    assert_eq!(io.web_items[3].value, 70.0);
    assert_eq!(io.web_items[4].value, 1.0, "led follows the switch");
    assert_eq!(io.web_items[6].text, "Привет, Мир");
}
