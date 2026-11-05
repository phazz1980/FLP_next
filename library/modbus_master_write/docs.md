# Запись Modbus Master

Записывает значение в ведомое устройство через блок «Modbus RTU Master» или «Modbus TCP Master».
`master` — rtu (по умолчанию) или tcp; `slave` — адрес ведомого (для TCP — Unit ID); `table` — holding или coil; `kind` — int, float
или bool, от него зависит вход (`in`, `in_f`, `in_b`). Новое значение
уходит ведомому на ближайшем опросе.

Пример: `examples/modbus_rtu`.

В модуле WASM протокол ведёт мастер-прошивка (в неё включается Modbus, около +14% flash на ESP32-C3); на плате не проверено.
