# ИЛИ

Выход true, если хотя бы один вход true.

| a | b | out |
| --- | --- | --- |
| false | false | false |
| false | true | true |
| true | false | true |
| true | true | true |

```toml
[[block]]
id = "both"
type = "or"
```
