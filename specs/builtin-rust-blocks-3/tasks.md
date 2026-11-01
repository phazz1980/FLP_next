# Задачи

- [x] `CppCtx.expander`, `behavior()` необязателен, булевы параметры как `true`/`false`.
- [x] Группа 1: `digital_in digital_out analog_in pwm_out`.
- [x] Группа 2: `dht22 ds18b20 flash_store dms cycle_report decoder encoder`.
- [x] Группа 3: `pcf8574_in pcf8574_out`.
- [x] Побайтное сравнение с шаблонами на проекте из всех 13 блоков (варианты параметров, расширители, `write_enable`, `inverted`, `pullup`, `period`).
- [x] Шаблоны удалены; снимки не изменились; 35 примеров из `examples/` дают тот же скетч.
- [x] Устройства плат (`devices.rs`) работают с Rust-родителем (`BlockInfo.code_of`).
- [x] Справка, `library/README.md`, архитектура обновлены.
- [x] Симуляция чистой логики: `dms`, `decoder`, `encoder` (сверка с C++ в `builtin_cpp`), `flash_store` (юнит-тест), `cycle_report` (пусто).
- [ ] Виртуальный стенд (тумблеры, ползунки, лампочки) для `digital_*`, `analog_in`, `pwm_out`, `dht22`, `ds18b20`, `pcf8574_*` — нужна спека.
- [ ] Сборка `arduino-cli` и проверка на плате (не делались).
- [ ] Шаги 4–6: UART/Modbus, сеть, веб.
