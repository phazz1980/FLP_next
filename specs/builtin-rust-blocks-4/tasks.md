# Задачи

- [x] `CppCtx.uart` (`UartCtx`), `CppCtx::text` и `flag` для строковых и булевых параметров.
- [x] `uart_config`, `uart_read`, `uart_send` (все режимы, триггеры, формат отправки).
- [x] `modbus_rtu_master`, `modbus_rtu_slave`, `modbus_tcp_slave`, `modbus_read`, `modbus_write`, `modbus_master_read`, `modbus_master_write`.
- [x] Побайтное сравнение со старыми шаблонами: 4 проекта UART (отправка 72 вариантами, чтение во всех режимах, `runtime`), RTU со всеми таблицами и типами, TCP на Ethernet и Wi-Fi; 8 примеров из `examples/`.
- [x] 10 шаблонов удалены; тесты `crab-blocks`, `crab-core`, `crab-codegen` зелёные.
- [x] Справка, архитектура обновлены.
- [ ] Сборка `arduino-cli` и проверка на плате (UART/Modbus без железа не проверить).
