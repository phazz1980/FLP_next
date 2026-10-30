# План: тип порта `same`

- **flp-core / library.rs** — вариант `SignalType::Same` (`same` в `block.toml`), `can_feed` пропускает
  его в обе стороны, `common()` сводит типы входов; проверка блока: выход `same` только при входах `same`.
- **flp-core / check.rs** — `same_type()`: общий тип поданных `same`-входов (иначе — значений по
  умолчанию, иначе `float`); `resolved_outputs` даёт его выходам `same`; `check_number_flow` ловит
  разнотипные входы и выход `same`, ушедший на вход чужого типа. Переменная на `same` — любого типа.
- **flp-codegen / program.rs** — у входа `same` тип = общий тип блока; значение по умолчанию чужого
  типа заменяется нулём. `input_type.<порт>` в шаблоне — общий тип.
- **library** — `switch`: a, b, out → `same`; `mux`: sel → `number`, a…p и out → `same`;
  `dms`: sel → `number`, in и o1…o16 → `same`, ноль на прочих выходах по `input_type.in`.
- **flp-editor, flp-debug** — новый вариант в списках типов и сопоставлениях.

Риск: старые схемы с `float` на этих блоках дают тот же код — проверяется снимками.
