//! Host ABI v2: modules that run on the PC in wasmi and on the board in the master firmware.
//!
//! Exports of a module: `abi_version() -> i32` (must be 2), `step(dt_ms: i32)`; optional
//! `init()`, `alloc(len: i32) -> i32` (where the host puts message bytes) and
//! `on_message(topic_ptr, topic_len, ptr, len)`.
//!
//! Imports from module `flp`: `input`, `param`, `output`, `log` as in ABI v1, plus
//! `tag_get(id) -> f64`, `tag_set(id, v)`, `publish(topic_ptr, topic_len, ptr, len)` and
//! `subscribe(topic_ptr, topic_len)`. Hardware imports (`gpio_read`, `gpio_write`, `adc_read`,
//! `pwm_write`, `millis`) work only if the manifest `allow` list names them. On the PC the
//! hardware is a stub ([`StubIo`]) that tests and the editor fill and read.

use std::collections::HashMap;

use serde::Deserialize;
use wasmi::{
    Caller, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

/// The ABI version this host speaks.
pub const ABI_VERSION: i32 = 2;

/// Imports every module may use.
const ALWAYS: &[&str] = &[
    "input",
    "param",
    "output",
    "log",
    "tag_get",
    "tag_set",
    "publish",
    "subscribe",
];
/// Imports that touch the hardware and need a permission in the manifest.
const HARDWARE: &[&str] = &["gpio_read", "gpio_write", "adc_read", "pwm_write", "millis"];

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
    /// Milliseconds since the module started; advances with every step.
    pub now_ms: u32,
}

struct Host {
    inputs: Vec<f64>,
    params: Vec<f64>,
    outputs: Vec<f64>,
    tags: HashMap<i32, f64>,
    io: StubIo,
    subscriptions: Vec<String>,
    published: Vec<(String, Vec<u8>)>,
    log: Vec<String>,
    limits: StoreLimits,
}

/// A loaded ABI v2 module with its state.
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
                outputs: vec![0.0; n_outputs],
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
            .map_err(|_| "в модуле нет функции abi_version() (нужен ABI 2)".to_owned())?;
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

    pub fn set_inputs(&mut self, inputs: &[f64]) {
        self.store.data_mut().inputs = inputs.to_vec();
    }

    pub fn set_params(&mut self, params: &[f64]) {
        self.store.data_mut().params = params.to_vec();
    }

    pub fn outputs(&self) -> &[f64] {
        &self.store.data().outputs
    }

    pub fn tag(&self, id: i32) -> Option<f64> {
        self.store.data().tags.get(&id).copied()
    }

    pub fn set_tag(&mut self, id: i32, value: f64) {
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
        .func_wrap("flp", "input", |c: Caller<'_, Host>, i: i32| -> f64 {
            c.data().inputs.get(i as usize).copied().unwrap_or(0.0)
        })
        .map_err(bad)?
        .func_wrap("flp", "param", |c: Caller<'_, Host>, i: i32| -> f64 {
            c.data().params.get(i as usize).copied().unwrap_or(0.0)
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "output",
            |mut c: Caller<'_, Host>, i: i32, v: f64| {
                if let Some(slot) = c.data_mut().outputs.get_mut(i as usize) {
                    *slot = v;
                }
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
        .func_wrap("flp", "tag_get", |c: Caller<'_, Host>, id: i32| -> f64 {
            c.data().tags.get(&id).copied().unwrap_or(0.0)
        })
        .map_err(bad)?
        .func_wrap(
            "flp",
            "tag_set",
            |mut c: Caller<'_, Host>, id: i32, v: f64| {
                c.data_mut().tags.insert(id, v);
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
            c.data().io.gpio_in.get(&pin).copied().unwrap_or(0)
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
        .func_wrap("flp", "millis", |c: Caller<'_, Host>| -> i32 {
            c.data().io.now_ms as i32
        })
        .map_err(bad)?;
    Ok(())
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
