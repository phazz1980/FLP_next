//! Host ABI v3: modules that run on the PC in wasmi and on the board in the master firmware.
//!
//! Values are 32 bit: `bool` and `int` travel as `i32`, `float` as `f32` (see `specs/numeric-model`).
//! Exports of a module: `abi_version() -> i32` (must be 3), `step(dt_ms: i32)`; optional
//! `init()`, `alloc(len: i32) -> i32` (where the host puts message bytes) and
//! `on_message(topic_ptr, topic_len, ptr, len)`.
//!
//! Imports from module `flp`: `log(ptr, len)`, `publish(topic_ptr, topic_len, ptr, len)`,
//! `subscribe(topic_ptr, topic_len)` and the typed value access, one pair per type (`_i` for
//! `bool`/`int`, `_f` for `float`): `input_i/f(idx) -> v`, `param_i/f(idx) -> v`,
//! `output_i/f(idx, v)`, `tag_get_i/f(id) -> v`, `tag_set_i/f(id, v)`. A slot is 4 bytes, the
//! type is a view of the same bits. Hardware imports (`gpio_read`, `gpio_write`, `adc_read`,
//! `pwm_write`, `sensor_read`, `millis`) work only if the manifest `allow` list names them. On the PC the
//! hardware is a stub ([`StubIo`]) that tests and the editor fill and read.

use std::collections::HashMap;

use serde::Deserialize;
use wasmi::{
    Caller, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

/// The ABI version this host speaks.
pub const ABI_VERSION: i32 = 3;

/// Imports every module may use.
const ALWAYS: &[&str] = &[
    "input_i",
    "input_f",
    "param_i",
    "param_f",
    "output_i",
    "output_f",
    "log",
    "tag_get_i",
    "tag_get_f",
    "tag_set_i",
    "tag_set_f",
    "publish",
    "subscribe",
];
/// Imports that touch the hardware and need a permission in the manifest.
const HARDWARE: &[&str] = &[
    "gpio_read",
    "gpio_write",
    "adc_read",
    "pwm_write",
    "millis",
    "sensor_read",
    "pcf_read",
    "pcf_write",
    "modbus",
    "uart",
    "text",
    "net",
];

const FUEL_PER_CALL: u64 = 5_000_000;
const MAX_MEMORY: usize = 16 * 1024 * 1024;
const MAX_LOG_LINES: usize = 200;
const MAX_MESSAGE: usize = 4096;
const MAX_TOPICS: usize = 32;

/// `module.toml` next to the `.wasm`: name and the hardware imports the module may use.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_abi")]
    pub abi: i32,
    #[serde(default)]
    pub allow: Vec<String>,
    /// Linear memory the module needs, in 64 KiB pages (0 when it has none or declares it itself).
    #[serde(default)]
    pub memory_pages: u32,
}

/// One 4-byte value slot: the bits of an `i32` (`bool`, `int`) or an `f32` (`float`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Word(pub u32);

impl Word {
    pub fn int(v: i32) -> Self {
        Self(v as u32)
    }
    pub fn float(v: f32) -> Self {
        Self(v.to_bits())
    }
    pub fn as_int(self) -> i32 {
        self.0 as i32
    }
    pub fn as_float(self) -> f32 {
        f32::from_bits(self.0)
    }
}

fn default_abi() -> i32 {
    ABI_VERSION
}

impl Manifest {
    /// Parses `module.toml`; unknown names in `allow` are errors.
    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: Self =
            toml::from_str(text).map_err(|e| format!("манифест модуля не разобран: {e}"))?;
        if manifest.abi != ABI_VERSION {
            return Err(format!(
                "манифест требует ABI {}, поддерживается {ABI_VERSION}",
                manifest.abi
            ));
        }
        for name in &manifest.allow {
            if !HARDWARE.contains(&name.as_str()) {
                return Err(format!(
                    "в allow манифеста неизвестное право «{name}» (допустимы: {})",
                    HARDWARE.join(", ")
                ));
            }
        }
        Ok(manifest)
    }
}

/// The stub hardware of the PC: what the module reads is set by the caller, what it writes
/// is kept for the caller to read.
#[derive(Debug, Clone, Default)]
pub struct StubIo {
    pub gpio_in: HashMap<i32, i32>,
    pub adc_in: HashMap<i32, i32>,
    pub gpio_out: HashMap<i32, i32>,
    pub pwm_out: HashMap<i32, i32>,
    /// What the master reports for a sensor: `(key, field)` as the module passes them
    /// (`kind | index << 4 | pin << 8`; field 0 temperature, 1 humidity, 2 read ok).
    pub sensors: HashMap<(i32, i32), f32>,
    /// What the master reports for a PCF8574: `(key, field)` as the module passes them
    /// (`address | sda << 8 | scl << 16`; field 0 the byte read, 1 the chip answered).
    pub pcf_in: HashMap<(i32, i32), i32>,
    /// The last byte the module handed over for a chip, by key.
    pub pcf_out: HashMap<i32, i32>,
    /// Modbus settings the module handed over, as `(code, value)` in order (the apply call is code 63).
    pub modbus_cfg: Vec<(i32, i32)>,
    /// What a Modbus read returns, by `(slave, table | kind << 4 | master << 8, register)`; the bits of an int or float.
    pub modbus_in: HashMap<(i32, i32, i32), i32>,
    /// What a module wrote, by the same key.
    pub modbus_out: HashMap<(i32, i32, i32), i32>,
    /// Status of a master's slave by address (-1: error of our slave).
    pub modbus_status: HashMap<i32, i32>,
    /// UART settings the module handed over, as `(port, code, value)` in order (code 63 opens).
    pub uart_cfg: Vec<(i32, i32, i32)>,
    /// Bytes waiting in a port's receive buffer, by port.
    pub uart_rx: HashMap<i32, std::collections::VecDeque<u8>>,
    /// What the module sent, by port, as `(value, size code)`.
    pub uart_tx: HashMap<i32, Vec<(i32, i32)>>,
    /// Speed changes the module asked for, by port.
    pub uart_speed: HashMap<i32, i32>,
    /// Text the module sent (op 6), by port.
    pub uart_text: HashMap<i32, Vec<u8>>,
    /// Network numbers the module set (`net` op 0), by code.
    pub net_num: HashMap<i32, i32>,
    /// Network text settings (`net` op 1), by code.
    pub net_text: HashMap<i32, String>,
    /// The radio request of the last step: 0 off, 1 client, 2 access point, 3 both.
    pub net_radio: i32,
    /// What the master reports for Wi-Fi (`net` op 3), by field: 0 client ready, 1 AP ready, 2 status, 3 error, 4 address.
    pub net_wifi: HashMap<i32, i32>,
    /// Ethernet numbers the module set (`net` op 6), by code: 1 dhcp, 2 sck, 3 miso, 4 mosi, 5 cs, 6 MAC byte, 63 complete.
    pub eth_num: HashMap<i32, i32>,
    /// Ethernet text settings (`net` op 7), by code: 1 ip, 2 gateway, 3 subnet.
    pub eth_text: HashMap<i32, String>,
    /// What the master reports for Ethernet (`net` op 8), by field: 0 ready, 1 address.
    pub eth_state: HashMap<i32, i32>,
    /// MQTT numbers the module set (`net` op 9), by code: 1 port, 2 keep-alive, 3 network, 4 max subscriptions, 63 complete.
    pub mqtt_num: HashMap<i32, i32>,
    /// MQTT text settings (`net` op 10), by code: 1 broker, 2 client id, 3 user, 4 password.
    pub mqtt_text: HashMap<i32, String>,
    /// What the master reports for the client (`net` op 11), by field: 0 connected, 1 error.
    pub mqtt_state: HashMap<i32, i32>,
    /// Publications the module queued (ops 12, 13): topic, payload, retain.
    pub mqtt_published: Vec<(String, String, bool)>,
    /// Filters the module subscribed to (op 14); the index is the slot.
    pub mqtt_filters: Vec<String>,
    /// Last message and counter of each slot (ops 15, 16).
    pub mqtt_slots: Vec<(String, i32)>,
    /// Layout JSON the module gave the web page (`net` op 17, code 1).
    pub web_layout: String,
    /// Widgets of the page (op 17, code 2); the module writes values (ops 19, 20) and reads what
    /// the page set (ops 21, 22). Tests play the page by changing `value` and `text`.
    pub web_items: Vec<WebStubItem>,
    /// Interfaces the page is served on (op 17, code 3): 1 Wi-Fi, 2 Ethernet.
    pub web_mask: i32,
    /// Password of the page (op 17, code 4).
    pub web_pass: String,
    /// The module finished the setup (op 17, code 63).
    pub web_complete: bool,
    /// Network form (op 17, code 5): the ten fields (ssid, password, dhcp, ip, gateway, subnet, ap ssid,
    /// ap password, mode, ap delay) and the widget of the form; empty until the module sets it up.
    pub web_net: Vec<String>,
    pub web_net_item: i32,
    /// The page saved new settings; read (and cleared) by op 23, code 10.
    pub web_net_changed: bool,
    /// Unix time (UTC) the NTP service knows; `None` until the first sync.
    pub net_unix: Option<u32>,
    /// Seconds since the last sync.
    pub net_age: i32,
    /// Milliseconds since the module started; advances with every step.
    pub now_ms: u32,
}

struct Host {
    inputs: Vec<Word>,
    params: Vec<Word>,
    outputs: Vec<Word>,
    tags: HashMap<i32, Word>,
    io: StubIo,
    subscriptions: Vec<String>,
    published: Vec<(String, Vec<u8>)>,
    log: Vec<String>,
    limits: StoreLimits,
}

/// A loaded ABI v3 module with its state.
pub struct McuModule {
    store: Store<Host>,
    instance: Instance,
    step: TypedFunc<i32, ()>,
}

impl McuModule {
    /// Checks imports against the manifest, loads the module, checks `abi_version` and runs `init`.
    pub fn load(wasm: &[u8], manifest: &Manifest, n_outputs: usize) -> Result<Self, String> {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module =
            Module::new(&engine, wasm).map_err(|e| format!("модуль WASM не загружен: {e}"))?;
        check_imports(&module, manifest)?;

        let mut store = Store::new(
            &engine,
            Host {
                inputs: Vec::new(),
                params: Vec::new(),
                outputs: vec![Word::default(); n_outputs],
                tags: HashMap::new(),
                io: StubIo::default(),
                subscriptions: Vec::new(),
                published: Vec::new(),
                log: Vec::new(),
                limits: StoreLimitsBuilder::new().memory_size(MAX_MEMORY).build(),
            },
        );
        store.limiter(|h| &mut h.limits);

        let mut linker = <Linker<Host>>::new(&engine);
        link_host(&mut linker)?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| format!("модуль не запущен: {e}"))?;

        let abi_version = instance
            .get_typed_func::<(), i32>(&store, "abi_version")
            .map_err(|_| "в модуле нет функции abi_version() (нужен ABI 3)".to_owned())?;
        store
            .set_fuel(FUEL_PER_CALL)
            .map_err(|e| format!("ошибка лимита выполнения: {e}"))?;
        let version = abi_version
            .call(&mut store, ())
            .map_err(|e| format!("abi_version не выполнена: {e}"))?;
        if version != ABI_VERSION {
            return Err(format!(
                "модуль собран для ABI {version}, поддерживается {ABI_VERSION}"
            ));
        }
        let step = instance
            .get_typed_func::<i32, ()>(&store, "step")
            .map_err(|_| "в модуле нет функции step(dt_ms: i32)".to_owned())?;
        let mut m = Self {
            store,
            instance,
            step,
        };
        if let Ok(init) = m.instance.get_typed_func::<(), ()>(&m.store, "init") {
            m.refuel()?;
            init.call(&mut m.store, ()).map_err(|e| describe(&e))?;
        }
        Ok(m)
    }

    /// One step of `dt_ms` milliseconds; the stub clock advances first.
    pub fn step(&mut self, dt_ms: u32) -> Result<(), String> {
        let io = &mut self.store.data_mut().io;
        io.now_ms = io.now_ms.wrapping_add(dt_ms);
        self.refuel()?;
        self.step
            .call(&mut self.store, dt_ms as i32)
            .map_err(|e| describe(&e))
    }

    /// Hands a message to the module if it subscribed to `topic`; returns whether it was delivered.
    pub fn deliver(&mut self, topic: &str, data: &[u8]) -> Result<bool, String> {
        if !self.store.data().subscriptions.iter().any(|t| t == topic) {
            return Ok(false);
        }
        let alloc = self
            .instance
            .get_typed_func::<i32, i32>(&self.store, "alloc")
            .map_err(|_| "модулю нужна функция alloc(len) для приёма сообщений".to_owned())?;
        let on_message = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), ()>(&self.store, "on_message")
            .map_err(|_| {
                "в модуле нет функции on_message(topic_ptr, topic_len, ptr, len)".to_owned()
            })?;
        let memory = self.memory()?;
        self.refuel()?;
        let put = |store: &mut Store<Host>, bytes: &[u8]| -> Result<i32, String> {
            let ptr = alloc
                .call(&mut *store, bytes.len() as i32)
                .map_err(|e| describe(&e))?;
            memory
                .write(&mut *store, ptr as usize, bytes)
                .map_err(|_| "alloc вернула адрес за пределами памяти".to_owned())?;
            Ok(ptr)
        };
        let topic_ptr = put(&mut self.store, topic.as_bytes())?;
        let data_ptr = put(&mut self.store, data)?;
        on_message
            .call(
                &mut self.store,
                (topic_ptr, topic.len() as i32, data_ptr, data.len() as i32),
            )
            .map_err(|e| describe(&e))?;
        Ok(true)
    }

    pub fn set_inputs(&mut self, inputs: &[Word]) {
        self.store.data_mut().inputs = inputs.to_vec();
    }

    pub fn set_params(&mut self, params: &[Word]) {
        self.store.data_mut().params = params.to_vec();
    }

    pub fn outputs(&self) -> &[Word] {
        &self.store.data().outputs
    }

    pub fn tag(&self, id: i32) -> Option<Word> {
        self.store.data().tags.get(&id).copied()
    }

    pub fn set_tag(&mut self, id: i32, value: Word) {
        self.store.data_mut().tags.insert(id, value);
    }

    pub fn io(&self) -> &StubIo {
        &self.store.data().io
    }

    pub fn io_mut(&mut self) -> &mut StubIo {
        &mut self.store.data_mut().io
    }

    /// Topics the module subscribed to.
    pub fn subscriptions(&self) -> &[String] {
        &self.store.data().subscriptions
    }

    /// Messages the module published since the last call.
    pub fn take_published(&mut self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(&mut self.store.data_mut().published)
    }

    /// Lines the module wrote with `log` since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.store.data_mut().log)
    }

    fn memory(&self) -> Result<Memory, String> {
        self.instance
            .get_memory(&self.store, "memory")
            .ok_or_else(|| "модуль не экспортирует память «memory»".to_owned())
    }

    fn refuel(&mut self) -> Result<(), String> {
        self.store
            .set_fuel(FUEL_PER_CALL)
            .map_err(|e| format!("ошибка лимита выполнения: {e}"))
    }
}

fn check_imports(module: &Module, manifest: &Manifest) -> Result<(), String> {
    for import in module.imports() {
        let (m, n) = (import.module(), import.name());
        if m != "flp" || !(ALWAYS.contains(&n) || HARDWARE.contains(&n)) {
            return Err(format!("неизвестный импорт {m}.{n}"));
        }
        if HARDWARE.contains(&n) && !manifest.allow.iter().any(|a| a == n) {
            return Err(format!(
                "модулю не разрешён импорт flp.{n}: добавьте «{n}» в allow манифеста"
            ));
        }
    }
    Ok(())
}

fn link_host(linker: &mut Linker<Host>) -> Result<(), String> {
    let bad = |e: wasmi::errors::LinkerError| format!("ошибка подключения функций: {e}");
    linker
        .func_wrap("flp", "input_i", |c: Caller<'_, Host>, i: i32| -> i32 {
            slot(&c.data().inputs, i).as_int()
        })
        .map_err(bad)?
        .func_wrap("flp", "input_f", |c: Caller<'_, Host>, i: i32| -> f32 {
            slot(&c.data().inputs, i).as_float()
        })
        .map_err(bad)?
        .func_wrap("flp", "param_i", |c: Caller<'_, Host>, i: i32| -> i32 {
            slot(&c.data().params, i).as_int()
        })
        .map_err(bad)?
        .func_wrap("flp", "param_f", |c: Caller<'_, Host>, i: i32| -> f32 {
            slot(&c.data().params, i).as_float()
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "output_i",
            |mut c: Caller<'_, Host>, i: i32, v: i32| -> i32 {
                set_slot(&mut c.data_mut().outputs, i, Word::int(v));
                v
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "output_f",
            |mut c: Caller<'_, Host>, i: i32, v: f32| -> f32 {
                set_slot(&mut c.data_mut().outputs, i, Word::float(v));
                v
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "log",
            |mut c: Caller<'_, Host>, ptr: i32, len: i32| {
                if let Some(bytes) = read_bytes(&mut c, ptr, len, 1024) {
                    let log = &mut c.data_mut().log;
                    if log.len() < MAX_LOG_LINES {
                        log.push(String::from_utf8_lossy(&bytes).into_owned());
                    }
                }
            },
        )
        .map_err(bad)?
        .func_wrap("flp", "tag_get_i", |c: Caller<'_, Host>, id: i32| -> i32 {
            c.data().tags.get(&id).copied().unwrap_or_default().as_int()
        })
        .map_err(bad)?
        .func_wrap("flp", "tag_get_f", |c: Caller<'_, Host>, id: i32| -> f32 {
            c.data()
                .tags
                .get(&id)
                .copied()
                .unwrap_or_default()
                .as_float()
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "tag_set_i",
            |mut c: Caller<'_, Host>, id: i32, v: i32| {
                c.data_mut().tags.insert(id, Word::int(v));
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "tag_set_f",
            |mut c: Caller<'_, Host>, id: i32, v: f32| {
                c.data_mut().tags.insert(id, Word::float(v));
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "publish",
            |mut c: Caller<'_, Host>, tp: i32, tl: i32, p: i32, l: i32| {
                let (Some(topic), Some(data)) = (
                    read_bytes(&mut c, tp, tl, 128),
                    read_bytes(&mut c, p, l, MAX_MESSAGE),
                ) else {
                    return;
                };
                let topic = String::from_utf8_lossy(&topic).into_owned();
                c.data_mut().published.push((topic, data));
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "subscribe",
            |mut c: Caller<'_, Host>, tp: i32, tl: i32| {
                let Some(topic) = read_bytes(&mut c, tp, tl, 128) else {
                    return;
                };
                let topic = String::from_utf8_lossy(&topic).into_owned();
                let subs = &mut c.data_mut().subscriptions;
                if subs.len() < MAX_TOPICS && !subs.contains(&topic) {
                    subs.push(topic);
                }
            },
        )
        .map_err(bad)?
        .func_wrap("flp", "gpio_read", |c: Caller<'_, Host>, pin: i32| -> i32 {
            // Bit 8 of the pin asks the master for the pull-up; the stub has no resistors.
            c.data().io.gpio_in.get(&(pin & 0xFF)).copied().unwrap_or(0)
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "gpio_write",
            |mut c: Caller<'_, Host>, pin: i32, v: i32| {
                c.data_mut().io.gpio_out.insert(pin, v);
            },
        )
        .map_err(bad)?
        .func_wrap("flp", "adc_read", |c: Caller<'_, Host>, pin: i32| -> i32 {
            c.data().io.adc_in.get(&pin).copied().unwrap_or(0)
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "pwm_write",
            |mut c: Caller<'_, Host>, pin: i32, v: i32| {
                c.data_mut().io.pwm_out.insert(pin, v);
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "sensor_read",
            |c: Caller<'_, Host>, key: i32, _period: i32, field: i32| -> f32 {
                c.data()
                    .io
                    .sensors
                    .get(&(key, field))
                    .copied()
                    .unwrap_or(0.0)
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "pcf_read",
            |c: Caller<'_, Host>, key: i32, _period: i32, field: i32| -> i32 {
                c.data().io.pcf_in.get(&(key, field)).copied().unwrap_or(0)
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "modbus",
            |mut c: Caller<'_, Host>, op: i32, a: i32, b: i32, reg: i32, d: i32| -> i32 {
                let io = &mut c.data_mut().io;
                match op {
                    0 => {
                        io.modbus_cfg.push((a, b));
                        0
                    }
                    1 => io.modbus_in.get(&(a, b, reg)).copied().unwrap_or(0),
                    2 => {
                        io.modbus_out.insert((a, b, reg), d);
                        0
                    }
                    _ => io.modbus_status.get(&a).copied().unwrap_or(0),
                }
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "uart",
            |mut c: Caller<'_, Host>, op: i32, port: i32, a: i32, b: i32| -> i32 {
                let text = if op == 6 {
                    read_bytes(&mut c, a, b, 1024)
                } else {
                    None
                };
                let io = &mut c.data_mut().io;
                match op {
                    6 => match text {
                        Some(t) => {
                            io.uart_text.entry(port).or_default().extend(t);
                            1
                        }
                        None => 0,
                    },
                    0 => {
                        io.uart_cfg.push((port, a, b));
                        0
                    }
                    1 => io
                        .uart_rx
                        .get_mut(&port)
                        .and_then(|q| q.pop_front())
                        .map_or(-1, i32::from),
                    2 => io.uart_rx.get(&port).map_or(0, |q| q.len() as i32),
                    3 => {
                        io.uart_tx.entry(port).or_default().push((a, b));
                        1
                    }
                    4 => io.uart_speed.get(&port).copied().unwrap_or(0),
                    _ => {
                        io.uart_speed.insert(port, a);
                        0
                    }
                }
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "text",
            |mut c: Caller<'_, Host>,
             op: i32,
             out: i32,
             cap: i32,
             a: i32,
             alen: i32,
             b: i32,
             blen: i32|
             -> i32 {
                let Some(memory) = c.get_export("memory").and_then(|e| e.into_memory()) else {
                    return 0;
                };
                text_op(memory.data_mut(&mut c), [op, out, cap, a, alen, b, blen])
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "net",
            |mut c: Caller<'_, Host>, op: i32, a: i32, b: i32, len: i32, d: i32| -> i32 {
                let text = if op == 1 || op == 7 || op == 10 {
                    read_bytes(&mut c, b, len, 128)
                } else {
                    None
                };
                match op {
                    12 | 13 => {
                        let topic = read_bytes(&mut c, a, b, 128);
                        let payload = read_bytes(&mut c, len, d, 128);
                        let io = &mut c.data_mut().io;
                        return match (topic, payload) {
                            (Some(t), Some(p)) if b > 0 && io.mqtt_state.get(&0) == Some(&1) => {
                                io.mqtt_published.push((
                                    String::from_utf8_lossy(&t).into_owned(),
                                    String::from_utf8_lossy(&p).into_owned(),
                                    op == 13,
                                ));
                                1
                            }
                            _ => 0,
                        };
                    }
                    17 => {
                        match a {
                            1 => {
                                let t = read_bytes(&mut c, b, len, 8191);
                                if let Some(t) = t {
                                    c.data_mut().io.web_layout =
                                        String::from_utf8_lossy(&t).into_owned();
                                }
                            }
                            2 => {
                                let t = read_bytes(&mut c, b, len * 16, 32 * 16);
                                if let Some(t) = t {
                                    let f = |r: &[u8]| f32::from_le_bytes(r.try_into().unwrap());
                                    c.data_mut().io.web_items = t
                                        .as_chunks::<16>()
                                        .0
                                        .iter()
                                        .map(|r| WebStubItem {
                                            kind: r[0],
                                            rw: r[1] != 0,
                                            dec: r[2],
                                            points: r[3],
                                            lo: f(&r[4..8]),
                                            hi: f(&r[8..12]),
                                            value: f(&r[12..16]),
                                            text: String::new(),
                                        })
                                        .collect();
                                }
                            }
                            3 => c.data_mut().io.web_mask = b,
                            4 => {
                                let t = read_bytes(&mut c, b, len, 64);
                                if let Some(t) = t {
                                    c.data_mut().io.web_pass =
                                        String::from_utf8_lossy(&t).into_owned();
                                }
                            }
                            5 => {
                                let t = read_bytes(&mut c, b, len, 700);
                                if let Some(t) = t {
                                    let io = &mut c.data_mut().io;
                                    io.web_net = String::from_utf8_lossy(&t)
                                        .split('\n')
                                        .map(str::to_string)
                                        .collect();
                                    io.web_net_item = d;
                                }
                            }
                            63 => c.data_mut().io.web_complete = true,
                            _ => {}
                        }
                        return 0;
                    }
                    19 => {
                        if let Some(it) = c.data_mut().io.web_items.get_mut(a as usize) {
                            it.value = f32::from_bits(b as u32);
                        }
                        return 0;
                    }
                    20 => {
                        let t = read_bytes(&mut c, b, len, 128);
                        if let (Some(t), Some(it)) =
                            (t, c.data_mut().io.web_items.get_mut(a as usize))
                        {
                            it.text = String::from_utf8_lossy(&t).into_owned();
                        }
                        return 0;
                    }
                    21 => {
                        return c
                            .data()
                            .io
                            .web_items
                            .get(a as usize)
                            .map_or(0, |it| it.value.to_bits() as i32);
                    }
                    22 => {
                        let Some(memory) = c.get_export("memory").and_then(|e| e.into_memory())
                        else {
                            return 0;
                        };
                        let Some(text) = c
                            .data()
                            .io
                            .web_items
                            .get(a as usize)
                            .map(|it| it.text.clone())
                        else {
                            return 0;
                        };
                        let data = memory.data_mut(&mut c);
                        let (p, cap) = (b as usize, len.max(0) as usize);
                        if cap == 0 || p + cap > data.len() {
                            return 0;
                        }
                        let n = text.len().min(cap - 1);
                        data[p..p + n].copy_from_slice(&text.as_bytes()[..n]);
                        data[p + n] = 0;
                        return n as i32;
                    }
                    23 => {
                        let io = &mut c.data_mut().io;
                        return match a {
                            10 => std::mem::take(&mut io.web_net_changed) as i32,
                            11 => io.web_net.get(2).is_some_and(|v| v != "0") as i32,
                            12 => io.web_net.get(8).and_then(|v| v.parse().ok()).unwrap_or(0),
                            0..=9 => {
                                let text = io.web_net.get(a as usize).cloned().unwrap_or_default();
                                let Some(memory) =
                                    c.get_export("memory").and_then(|e| e.into_memory())
                                else {
                                    return 0;
                                };
                                let data = memory.data_mut(&mut c);
                                let (p, cap) = (b as usize, len.max(0) as usize);
                                if cap == 0 || p + cap > data.len() {
                                    return 0;
                                }
                                let n = text.len().min(cap - 1);
                                data[p..p + n].copy_from_slice(&text.as_bytes()[..n]);
                                data[p + n] = 0;
                                n as i32
                            }
                            _ => 0,
                        };
                    }
                    14 => {
                        let Some(f) = read_bytes(&mut c, a, b, 128) else {
                            return -1;
                        };
                        let f = String::from_utf8_lossy(&f).into_owned();
                        let io = &mut c.data_mut().io;
                        let limit = io.mqtt_num.get(&4).copied().unwrap_or(4).clamp(1, 16) as usize;
                        if let Some(i) = io.mqtt_filters.iter().position(|x| *x == f) {
                            return i as i32;
                        }
                        if io.mqtt_filters.len() >= limit {
                            return -1;
                        }
                        io.mqtt_filters.push(f);
                        io.mqtt_slots.push((String::new(), 0));
                        return io.mqtt_filters.len() as i32 - 1;
                    }
                    15 => {
                        let Some(memory) = c.get_export("memory").and_then(|e| e.into_memory())
                        else {
                            return 0;
                        };
                        let Some((text, _)) = c.data().io.mqtt_slots.get(a as usize).cloned()
                        else {
                            return 0;
                        };
                        let data = memory.data_mut(&mut c);
                        let (p, cap) = (b as usize, len.max(0) as usize);
                        if p + cap > data.len() {
                            return 0;
                        }
                        let n = text.len().min(cap);
                        data[p..p + n].copy_from_slice(&text.as_bytes()[..n]);
                        return n as i32;
                    }
                    _ => {}
                }
                if op == 5 {
                    let Some(memory) = c.get_export("memory").and_then(|e| e.into_memory()) else {
                        return 0;
                    };
                    let Some(unix) = c.data().io.net_unix else {
                        return 0;
                    };
                    let (h, m, sec, ..) = civil(unix as i64 + len as i64);
                    let line = format!("{h:02}:{m:02}:{sec:02}");
                    let data = memory.data_mut(&mut c);
                    let (p, cap) = (a as usize, b as usize);
                    if p + cap > data.len() || cap == 0 {
                        return 0;
                    }
                    let n = line.len().min(cap);
                    data[p..p + n].copy_from_slice(&line.as_bytes()[..n]);
                    return n as i32;
                }
                let io = &mut c.data_mut().io;
                match op {
                    0 => {
                        io.net_num.insert(a, b);
                        0
                    }
                    1 => match text {
                        Some(t) => {
                            io.net_text
                                .insert(a, String::from_utf8_lossy(&t).into_owned());
                            1
                        }
                        None => 0,
                    },
                    2 => {
                        io.net_radio = a;
                        0
                    }
                    3 => io.net_wifi.get(&a).copied().unwrap_or(0),
                    4 => match (a, io.net_unix) {
                        (0, u) => i32::from(u.is_some()),
                        (1, _) => io.net_age,
                        (_, None) => 0,
                        (9, Some(u)) => u as i32,
                        (f, Some(u)) => {
                            let (h, m, sec, day, month, year, wd) = civil(u as i64 + b as i64);
                            [h, m, sec, day, month, year, wd]
                                .get(f as usize - 2)
                                .copied()
                                .unwrap_or(0)
                        }
                    },
                    6 => {
                        io.eth_num.insert(a, b);
                        0
                    }
                    7 => match text {
                        Some(t) => {
                            io.eth_text
                                .insert(a, String::from_utf8_lossy(&t).into_owned());
                            1
                        }
                        None => 0,
                    },
                    8 => io.eth_state.get(&a).copied().unwrap_or(0),
                    9 => {
                        io.mqtt_num.insert(a, b);
                        0
                    }
                    10 => match text {
                        Some(t) => {
                            io.mqtt_text
                                .insert(a, String::from_utf8_lossy(&t).into_owned());
                            1
                        }
                        None => 0,
                    },
                    11 => io.mqtt_state.get(&a).copied().unwrap_or(0),
                    16 => io.mqtt_slots.get(a as usize).map_or(0, |s| s.1),
                    _ => 0,
                }
            },
        )
        .map_err(bad)?
        .func_wrap(
            "flp",
            "pcf_write",
            |mut c: Caller<'_, Host>, key: i32, v: i32| -> i32 {
                c.data_mut().io.pcf_out.insert(key, v);
                1
            },
        )
        .map_err(bad)?
        .func_wrap("flp", "millis", |c: Caller<'_, Host>| -> i32 {
            c.data().io.now_ms as i32
        })
        .map_err(bad)?;
    Ok(())
}

/// A widget of the web page as the simulator keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WebStubItem {
    /// 0 number, 1 text shown, 2 text input, 3 secret text input, 4 chart.
    pub kind: u8,
    pub rw: bool,
    pub dec: u8,
    /// Chart: points the master keeps.
    pub points: u8,
    pub lo: f32,
    pub hi: f32,
    pub value: f32,
    pub text: String,
}

impl StubIo {
    /// A message from the broker: every slot whose filter matches takes it (up to 128 bytes).
    pub fn mqtt_receive(&mut self, topic: &str, payload: &str) {
        for (i, f) in self.mqtt_filters.iter().enumerate() {
            if mqtt_match(f, topic) {
                let cut = payload.len().min(128);
                self.mqtt_slots[i].0 =
                    String::from_utf8_lossy(&payload.as_bytes()[..cut]).into_owned();
                self.mqtt_slots[i].1 += 1;
            }
        }
    }
}

/// Topic against a filter with `+` and `#`.
fn mqtt_match(filter: &str, topic: &str) -> bool {
    let (mut f, mut t) = (filter.split('/'), topic.split('/'));
    loop {
        match (f.next(), t.next()) {
            (Some("#"), _) => return true,
            (Some("+"), Some(_)) => {}
            (Some(a), Some(b)) if a == b => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Date and time of a Unix second: hour, minute, second, day, month, year, weekday (1 = Monday... 7 = Sunday, as the C++ block).
fn civil(t: i64) -> (i32, i32, i32, i32, i32, i32, i32) {
    let days = t.div_euclid(86400);
    let rest = t.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let weekday = (days + 3).rem_euclid(7) + 1;
    (
        (rest / 3600) as i32,
        (rest % 3600 / 60) as i32,
        (rest % 60) as i32,
        day as i32,
        month as i32,
        year as i32,
        weekday as i32,
    )
}

/// Port of `h_text` of the master (`master.ino`): operations on strings in the module memory.
fn text_op(m: &mut [u8], args: [i32; 7]) -> i32 {
    let [op, out, cap, a, alen, b, blen] = args;
    let size = m.len();
    let inm = |p: i32, l: i32| p >= 0 && l >= 0 && p as usize + l as usize <= size;
    let reads_a = op == 1 || (5..=10).contains(&op);
    let reads_b = op == 5 || op == 8;
    let mut ok = (1..=11).contains(&op) && (!reads_a || inm(a, alen)) && (!reads_b || inm(b, blen));
    if (1..=4).contains(&op) || op == 7 || op == 11 {
        ok = ok && inm(out, cap);
    }
    if op == 6 || op == 10 {
        ok = ok && inm(out, 4);
    }
    if !ok {
        return if op == 8 || op == 9 { -1 } else { 0 };
    }
    let slice = |m: &[u8], p: i32, l: i32| m[p as usize..(p + l) as usize].to_vec();
    let put = |m: &mut [u8], cur: i32, s: &[u8]| -> i32 {
        let cur = cur.clamp(0, cap);
        let n = s.len().min((cap - cur) as usize);
        let at = (out + cur) as usize;
        m[at..at + n].copy_from_slice(&s[..n]);
        cur + n as i32
    };
    match op {
        1 => {
            let s = slice(m, a, alen);
            put(m, b, &s)
        }
        2 => {
            let t = format!(
                "{:.*}",
                alen.clamp(0, 9) as usize,
                f64::from(f32::from_bits(a as u32))
            );
            put(m, b, &t.as_bytes()[..t.len().min(47)])
        }
        3 => put(m, b, a.to_string().as_bytes()),
        4 => put(m, b, &[(a & 0xFF) as u8]),
        5 => {
            let (x, y) = (slice(m, a, alen), slice(m, b, blen));
            i32::from(text_match(&x, &y, out, cap != 0))
        }
        6 => {
            let n = (alen as usize).min(47);
            let t = String::from_utf8_lossy(&m[a as usize..a as usize + n]).replace(',', ".");
            let t = t.trim_matches([' ', '\t', '\r', '\n']);
            match t.parse::<f64>() {
                Ok(v) if !t.is_empty() && alen < 48 && v.is_finite() => {
                    m[out as usize..out as usize + 4].copy_from_slice(&(v as f32).to_le_bytes());
                    1
                }
                _ => 0,
            }
        }
        7 => {
            let start = b.clamp(0, alen);
            let end = if blen < 0 || start + blen > alen {
                alen
            } else {
                start + blen
            };
            let s = slice(m, a + start, end - start);
            put(m, 0, &s)
        }
        8 => {
            let from = out.max(0);
            if from > alen {
                return -1;
            }
            if blen == 0 {
                return from;
            }
            let (x, y) = (slice(m, a, alen), slice(m, b, blen));
            (from as usize..)
                .take_while(|&at| at + y.len() <= x.len())
                .find(|&at| x[at..at + y.len()] == y[..])
                .map_or(-1, |at| at as i32)
        }
        9 => {
            if b >= 0 && b < alen {
                i32::from(m[(a + b) as usize])
            } else {
                -1
            }
        }
        10 => {
            let (mut v, mut part, mut digits, mut good) = ([0i32; 4], 0usize, false, true);
            for &ch in &m[a as usize..(a + alen) as usize] {
                if ch.is_ascii_digit() {
                    v[part] = v[part] * 10 + i32::from(ch - b'0');
                    digits = true;
                    good &= v[part] <= 255;
                } else if ch == b'.' && digits && part < 3 {
                    part += 1;
                    digits = false;
                } else {
                    good = false;
                }
                if !good {
                    break;
                }
            }
            if good && digits && part == 3 {
                for (i, x) in v.iter().enumerate() {
                    m[out as usize + i] = *x as u8;
                }
                1
            } else {
                0
            }
        }
        _ => {
            let t = format!(
                "{}.{}.{}.{}",
                a & 255,
                (a >> 8) & 255,
                (a >> 16) & 255,
                (a >> 24) & 255
            );
            put(m, 0, t.as_bytes())
        }
    }
}

/// `textMatch` of the master: mode 0 equal, 1 contains, 2 starts with, 3 ends with.
fn text_match(a: &[u8], b: &[u8], mode: i32, nocase: bool) -> bool {
    if (mode == 0 && a.len() != b.len()) || b.len() > a.len() {
        return false;
    }
    let first = if mode == 3 { a.len() - b.len() } else { 0 };
    let last = match mode {
        1 => a.len() - b.len(),
        0 => 0,
        _ => first,
    };
    (first..=last).any(|at| {
        a[at..at + b.len()].iter().zip(b).all(|(x, y)| {
            if nocase {
                x.eq_ignore_ascii_case(y)
            } else {
                x == y
            }
        })
    })
}

fn slot(slots: &[Word], i: i32) -> Word {
    slots.get(i as usize).copied().unwrap_or_default()
}

fn set_slot(slots: &mut [Word], i: i32, v: Word) {
    if let Some(s) = slots.get_mut(i as usize) {
        *s = v;
    }
}

/// Reads at most `max` bytes from the module memory; `None` if the range is outside it.
fn read_bytes(c: &mut Caller<'_, Host>, ptr: i32, len: i32, max: usize) -> Option<Vec<u8>> {
    let memory = c.get_export("memory").and_then(|e| e.into_memory())?;
    let mut bytes = vec![0u8; (len.max(0) as usize).min(max)];
    memory.read(&*c, ptr as usize, &mut bytes).ok()?;
    Some(bytes)
}

fn describe(e: &wasmi::Error) -> String {
    use wasmi::TrapCode;
    match e.as_trap_code() {
        Some(TrapCode::OutOfFuel) => {
            "модуль выполняется слишком долго (возможно, бесконечный цикл)".to_owned()
        }
        Some(TrapCode::MemoryOutOfBounds) => "модуль обратился за пределы памяти".to_owned(),
        Some(code) => format!("модуль остановлен: {code}"),
        None => format!("ошибка модуля: {e}"),
    }
}
