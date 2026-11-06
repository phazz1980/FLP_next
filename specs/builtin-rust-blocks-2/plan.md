# План: встроенные блоки на Rust, шаг 2

- `crates/crab-blocks/src/more.rs` — числа, биты и слова, `number_change`, `string_change`, `pid`, `scale_table`; `random` — xorshift32 с общим `RANDOM_SEED` в симуляции и в прошивке.
- `crates/crab-blocks/src/text.rs` — текстовые блоки: поведение перенесено из `crab-sim::builtin::text`, плюс `emit_cpp` (`String` Arduino).
- `crab-sim::builtin` берёт поведение из `crab-blocks`; в `crab-codegen` PRELUDE симуляции дополнен математикой и `random`.
- Шаблоны `library/<блок>/esp32.cpp.tmpl` у 39 блоков удалены.
- Проверка: побайтное совпадение C++ со старыми шаблонами (проект из всех блоков и вариантов параметров), снимки `crab-codegen`, `builtin_cpp` (числа, биты, `number_change`, `pid`), `template.rs` (текстовые блоки из C++ против `behavior()`).
- Риски: у `scale_table` разбор `strtof` в симуляции не знает `inf/nan/hex`; числовой выход, связанный со входом int, в прошивке усекается, в симуляции — нет.
