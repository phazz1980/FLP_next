# Чтение Modbus Master

Читает значение ведомого, которое опросил блок «Modbus RTU Master».
`slave` — адрес ведомого; `table` — holding, input, coil или discrete;
`kind` — int, float или bool, от него зависит выход (`out`, `out_f`, `out_b`).
Выход `ok` показывает, отвечает ли ведомый.

Пример: `examples/modbus_rtu`.
