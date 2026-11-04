# Чтение Modbus Master

Читает значение ведомого, которое опросил блок «Modbus RTU Master».
`slave` — адрес ведомого; `table` — holding, input, coil или discrete;
`kind` — int, float или bool, от него зависит выход (`out`, `out_f`, `out_b`).
Выход `ok` показывает, отвечает ли ведомый.

Пример: `examples/modbus_rtu`.

В модуле WASM протокол ведёт мастер-прошивка (в неё включается Modbus, около +14% flash на ESP32-C3); на плате не проверено.
