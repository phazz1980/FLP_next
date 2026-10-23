# Счётчик

Каждый переход `up` из false в true прибавляет к `count` единицу, каждый
переход `down` — вычитает. Пока `reset` true, счётчик держится на нуле.
Неподключённые входы считаются false. После включения питания `count = 0`.

```toml
[[block]]
id = "presses"
type = "counter"

[[link]]
from = "button.out"
to = "presses.up"
```
