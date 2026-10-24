# Modbus RTU Slave

Ведомое устройство Modbus RTU на UART, обычно через преобразователь RS-485
(MAX485 и подобные: вывод `de` управляет DE и RE). Отдаёт четыре таблицы
с адреса 0, их размеры задаются в параметрах.

Значения пишут и читают блоки «Запись Modbus» и «Чтение Modbus» с `slave = "rtu"`.
UART 0 занят USB и онлайн-отладкой, поэтому берите UART 1 или 2.

Нужна Arduino-библиотека FLProg_Utilites (`flprogUart.h`), её ставит FLProg.

Пример: `examples/modbus_rtu`.
