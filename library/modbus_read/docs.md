# Чтение Modbus

Читает значение из таблицы ведомого устройства — то, что туда записал мастер.
`slave` — tcp («Modbus TCP Slave») или rtu («Modbus RTU Slave»);
`table` — holding, input, coil или discrete; `kind` — int, float или bool,
от него зависит выход (`out`, `out_f`, `out_b`).

Примеры: `examples/modbus_wifi`, `examples/modbus_rtu`.
