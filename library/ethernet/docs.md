# Ethernet W5500

Проводная сеть на модуле WizNet W5500 (подходят W5100 и W5200) по шине SPI.
Подключение к ESP32 DevKit: SCK — 18, MISO — 19, MOSI — 23, CS — параметр `cs`
(по умолчанию 5). Выход `ready` — есть ли связь.

Блок один на проект. У «Modbus TCP Slave» выберите `net = "ethernet"`, чтобы
он работал через этот блок.

Пример: `examples/modbus_ethernet`.
