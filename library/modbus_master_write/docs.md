# Запись Modbus Master

Записывает значение в ведомое устройство через блок «Modbus RTU Master».
`slave` — адрес ведомого; `table` — holding или coil; `kind` — int, float
или bool, от него зависит вход (`in`, `in_f`, `in_b`). Новое значение
уходит ведомому на ближайшем опросе.

Пример: `examples/modbus_rtu`.
