# Задачи

- [x] `CppCtx.bound`, `flag_code` (`is_bound`, `flag_code()`); `arduino.rs` заполняет их до подмены `crabNet`.
- [x] `net.rs`: `wifi`, `ethernet` (общие службы, пул, загрузка прошивки).
- [x] `network.rs`: `mqtt*`, `ntp`.
- [x] `web.rs`: `web_ui`, `web_net`, 15 виджетов; большие тексты — константы с метками.
- [x] Побайтное сравнение со старыми шаблонами: все виджеты, `system`, `crabNet`, MQTT на Wi-Fi/Ethernet.
- [x] Все шаблоны удалены; `crab-web` и тест `load.rs` переведены на код блока.
- [x] fmt, clippy, тесты всего workspace зелёные.
- [x] Справка, архитектура обновлены.
- [ ] Сборка `arduino-cli` и проверка на плате с Wi-Fi (не проверено на железе).
