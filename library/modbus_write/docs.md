# Запись Modbus

Записывает значение в таблицу ведомого устройства, откуда его читает мастер.
`slave` — tcp («Modbus TCP Slave») или rtu («Modbus RTU Slave»);
`table` — holding, input, coil или discrete; `kind` — int, float или bool,
от него зависит вход (`in`, `in_f`, `in_b`).

Примеры: `examples/modbus_wifi`, `examples/modbus_rtu`.
