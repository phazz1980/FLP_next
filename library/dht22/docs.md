# DHT22

Читает температуру и влажность с датчика DHT22 (AM2302) раз в `period`.
Первое измерение — через `period` после включения, до него выходы равны 0,
`ok = false`. При ошибке чтения `ok` становится false, а значения хранят
последнее удачное измерение.

Нужны библиотеки «DHT sensor library» и «Adafruit Unified Sensor» —
`flp build` ставит их сам.

```toml
[[block]]
id = "air"
type = "dht22"
pin = 15
```
