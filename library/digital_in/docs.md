# Цифровой вход

Читает уровень на выводе `pin` каждый цикл программы и выдаёт его на выход `out`.

Кнопка, замыкающая вывод на землю: `pullup = true`, `inverted = true` —
тогда нажатие даёт `out = true`.

```toml
[[block]]
id = "button"
type = "digital_in"
pin = 13
pullup = true
inverted = true
```
