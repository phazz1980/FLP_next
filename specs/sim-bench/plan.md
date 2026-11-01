# План

- `flp-blocks/src/bench.rs`: `Bench` (уровни входных пинов, сигналы датчиков `Signal{from,to,duration_ms,start_ms,fault}`, состояние выходных пинов); `Ctx.bench`; `Behavior::drive()` — что блок выставил на свой пин.
- `hardware.rs`: `behavior()` для `digital_in`, `analog_in`, `dht22`, `ds18b20`, `digital_out`, `pwm_out` (инверсия, подтяжка, разрешение записи, период опроса).
- `flp-sim`: `Simulation.bench`, после шага собирает `drive()` по параметру `pin`; `Computed::Bench` для блоков, чей «выход» задаётся стендом; API `set_pin_level`, `set_signal`, `pin_state`.
- `flp-editor/simview.rs`: меню порта таких блоков правит стенд; состояние выходных пинов видно на блоке.
- Риски: `follow()` должен переносить настройки стенда при пересборке; один пин у нескольких блоков (шина 1-Wire) — ключ сигнала `(пин, канал)`.
