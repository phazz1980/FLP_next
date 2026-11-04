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
                let io = &mut c.data_mut().io;
                match op {
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
