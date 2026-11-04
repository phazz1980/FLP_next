# Задачи
- [x] 1 Права шире 8 бит (проверка: мастер собирается, старые модули грузятся)
- [~] 2 Modbus RTU master+slave готов (тест `modbus_rtu_goes_through_the_master`, пример `examples/modbus_rtu`, мастер на C3 собирается: 51% flash, 11% RAM; на плате не проверено). Осталось: Modbus TCP (после сети)
- [x] 3 Строки в WASM: буфер + длина в памяти модуля, `text_*` через импорт `text` мастера (тест `text_blocks_match_the_simulation`, фикстура `wasm_text`, пример `examples/text_wasm`; на плате не проверено). Не поддержано: строковые переменные, строки между листами, отслеживание строк
- [x] 4a UART-байты (`uart_read` по байту, `uart_send` двоично, `uart_config` скорость; тест `uart_bytes_go_through_the_master`, пример `examples/uart_bytes`; мастер: C3 36% flash/8% RAM, ESP32 32%/10%; на плате не проверено)
- [x] 4 UART-строки: `uart_read` строками, `uart_send` текстом (тест `uart_lines_and_strings_work_in_a_module`, пример `examples/text_wasm`; мастер с FLP_TEXT+FLP_UART: C3 36% flash, 8% RAM; ESP32 32%, 10%; на плате не проверено)
- [ ] 5 Wi-Fi/Ethernet, NTP (проверка: тест + пример)
- [ ] 6 MQTT (проверка: тест + пример)
- [ ] 7 Веб (проверка: тест + пример)
- [ ] 8 Справка, `docs.md`, критерии
