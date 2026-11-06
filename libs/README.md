# Сторонние Arduino-библиотеки

Сюда кладём библиотеки, которые нужны сгенерированным скетчам. `flp-build` передаёт эту папку
в arduino-cli (`--libraries`): каждая вложенная папка — одна библиотека. Хранятся только нужные
для сборки файлы (`src`, `library.properties`, `keywords.txt`); примеры, документацию и блоки
других редакторов не оставляем. Другая папка — переменная `FLP_ARDUINO_LIBS`.

| Папка | Откуда | Зачем |
|---|---|---|
| `FLProg_Network` | https://github.com/Anderand/FLProg_Network (r17, `Библиотека/FLProg_Network`) | Wi-Fi, Ethernet, веб-сервер, OTA |
| `Flprog_Modbus` | там же (`Библиотека/Flprog_Modbus`) | Modbus RTU/TCP |
| `Flprog_Utilites` | стандартный набор FLProg (https://github.com/Totuin/Flprog_Utilites) | общие утилиты и UART; нужна двум предыдущим |
| `RT_HW_BASE` | https://github.com/Totuin/RT_HW_00_BASE (v7.1.1) | базовый слой железа; нужна `Flprog_Utilites` |
| `Wasm3` | Менеджер библиотек Arduino, 0.5.0 | интерпретатор WASM в мастер-прошивке (всегда) |
| `OneWire` | там же, 2.3.8 | шина 1-Wire (DS18B20) |
| `DallasTemperature` | там же, 4.0.6 | DS18B20 |
| `DHT_sensor_library` | там же, 1.4.7 | DHT11/DHT22 |
| `Adafruit_Unified_Sensor` | там же, 1.1.15 | нужна `DHT_sensor_library` |

Библиотеки из этой папки программа не ставит из интернета: сборка работает без сети. Лицензии (`LICENSE`) оставлены.
Если библиотеку нужно положить сюда, ставьте её в пустую папку (`ARDUINO_DIRECTORIES_USER`), копируйте без `examples` и
документации.

## Обновление

Эти библиотеки часто обновляются — раз в какое-то время сверяйте с источниками: клонируйте репозиторий,
сравните `version` в `library.properties`, при новой версии замените `src` (лишнее не переносим),
проверьте сборку `examples/wifi_modes`, `examples/modbus_wifi`, `examples/web_ui` с чистой папкой
библиотек пользователя (`ARDUINO_DIRECTORIES_USER` на пустую папку) и поправьте таблицу выше.
