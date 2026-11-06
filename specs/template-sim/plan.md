# План: симуляция из шаблона

## Затронуто
- `crab-codegen`: новая функция `sim_source(def, lib) -> Result<String, Unsupported>`. Раскрывает шаблон тем же
  движком, что `arduino.rs` (`split_sections`), но с подстановками для симуляции: `{{ input.x }}` →
  `crab_in[i]`, `{{ output.y }}` → `crab_out[i]`, `{{ param.p }}` → `crab_param[i]`, `{{ id }}` → `blk`.
  Добавляет заглушки `millis()`, `micros()`, `Serial.print*`→`crab_log`, `constrain`, `map`, `min/max`, `abs`,
  типы `uint8_t…`, `bool`. Любое другое имя из списка железа (`digitalRead`, `analogRead`, `Wire`, `WiFi`,
  `#include` библиотек) → `Unsupported { lines }`.
- `crab-build` (или новый крейт `crab-wasmbuild`, если `crab-build` не подходит по границам): `compile_wasm(zig, src, out)`
  командой `zig cc --target=wasm32-freestanding -O2 -nostdlib -Wl,--no-entry -Wl,--allow-undefined -o out src`
  (флаги линковки уточнить на реальном zig). Путь к zig передаёт редактор; крейт сам настроек не читает.
- `crab-editor/src/theme.rs` (`Settings`): поля `template_sim: Option<bool>` и `zig_path: Option<PathBuf>`;
  окно настроек — флажок и поле пути с кнопкой «Обзор…» и проверкой `zig version`.
- `crab-editor/src/blockedit.rs`: после сохранения блока — сборка в фоне, статус на вкладке «Пробный прогон»
  («собирается», «не симулируется: …», «нет компилятора»).
- `crab-sim`: без изменений — модуль того же интерфейса (`step/init`, импорты `crab.*`).

## Формат
- Рядом с `block.wasm` — пустой файл `block.wasm.generated`. Нет метки — файл ручной, автосборка его не трогает.
- Обвязка симуляции: `init()` вызывает инициализаторы глобалов и `@setup`; `step(dt)` наращивает `crab_now`,
  читает входы и параметры, исполняет `@loop`, пишет выходы.

## Риски
- Шаблон — не чистый C++ (условия и циклы шаблонизатора): использовать результат существующего раскрытия.
- Размер zig (оценка: архив ~80 МБ, распакованный 300 МБ с лишним) — в git не класть, `.gitignore` для `tools/zig/`.
- Переменные `static` в `@loop`/`@functions` — проверить на реальных блоках из `library/`.
