# Задачи
- [x] 1 Права шире 8 бит (проверка: мастер собирается, старые модули грузятся)
- [~] 2 Modbus RTU master+slave готов (тест `modbus_rtu_goes_through_the_master`, пример `examples/modbus_rtu`, мастер на C3 собирается: 51% flash, 11% RAM; на плате не проверено). Осталось: Modbus TCP (после сети)
- [ ] 3 Строки в WASM: `Ty::S`, `text_*` (проверка: сравнение со симулятором `sim-strings`)
- [x] 4a UART-байты (`uart_read` по байту, `uart_send` двоично, `uart_config` скорость; тест `uart_bytes_go_through_the_master`, пример `examples/uart_bytes`; мастер: C3 36% flash/8% RAM, ESP32 32%/10%; на плате не проверено)
- [ ] 4 UART-строки (`uart_read`/`uart_send`) (проверка: тест + пример)
- [ ] 5 Wi-Fi/Ethernet, NTP (проверка: тест + пример)
- [ ] 6 MQTT (проверка: тест + пример)
- [ ] 7 Веб (проверка: тест + пример)
- [ ] 8 Справка, `docs.md`, критерии
