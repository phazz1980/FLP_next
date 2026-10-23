# RS-триггер

Выход `q` включается, когда `s = true`, и остаётся включённым, пока не придёт `r = true`.
Если оба входа true одновременно, побеждает сброс (`q = false`).
После включения питания `q = false`.

Типичное применение — кнопки «Пуск» и «Стоп»:

```toml
[[block]]
id = "run"
type = "rs_trigger"

[[link]]
from = "start.out"
to = "run.s"

[[link]]
from = "stop.out"
to = "run.r"
```
