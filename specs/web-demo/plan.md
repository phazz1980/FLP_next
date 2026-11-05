# План: веб-демо

## Затронутые крейты
- Новый `crates/flp-demo` (cdylib, только wasm32): eframe web, окно «просмотр + Сборка + Симуляция». Вне workspace default-members, чтобы обычная сборка не требовала wasm-цели.
- `flp-core`, `flp-blocks`, `flp-codegen`, `flp-sim`: проверка `cargo check --target wasm32-unknown-unknown`. Уже найдено: падает `getrandom` (нужна feature `js`/`wasm_js` в целевой зависимости); `std::fs` и `SystemTime` в `flp-sim`/`flp-codegen` — только в путях загрузки с диска и модулей `block.wasm`, спрятать за `cfg(not(target_arch = "wasm32"))`.
- Отрисовку листа (сейчас в `flp-editor`) использовать повторно: если она сцеплена с файлами/serialport — выделить в общий модуль или крейт `flp-view`.

## Данные
- Библиотека блоков и 6 примеров вшиваются через `include_dir`/`include_bytes` в один архив при сборке; загрузка `.flpn` — через выбор файла в браузере.
- Скачивание `.ino` — через Blob.

## Сборка и публикация
- `trunk build --release` (или wasm-bindgen + wasm-opt) → `docs/demo/`; скрипт `scripts/build-demo.ps1`; ссылка с лендинга.

## Риски
- Размер WASM (egui + minijinja + wasmi): оценка 5–10 МБ, нужен `opt-level="s"`, wasm-opt, gzip на Pages.
- `wasmi` для пользовательских блоков в браузере — возможен, но в демо хватит встроенных блоков.
- `flp-editor` тянет много desktop-зависимостей; без выделения вида листа придётся дублировать рисование.
