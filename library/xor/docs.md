# Исключающее ИЛИ

Выход true, если входы различаются.

| a | b | out |
| --- | --- | --- |
| false | false | false |
| false | true | true |
| true | false | true |
| true | true | false |

```toml
[[block]]
id = "both"
type = "xor"
```
