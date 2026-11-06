# Ethernet в модулях: задачи

- [x] 1 Блок `ethernet`: `sck/miso/mosi`, `address`; C++-вывод — проверка: снимки, старые проекты читаются
- [x] 2 Мастер: `CRAB_WIFI`/`CRAB_ETH`, op 6–8, NTP на Ethernet — проверка: сборка C3 и ESP32 (только Wi-Fi, только Ethernet, оба)
- [x] 3 `crab-build`: признаки `wifi`/`eth` — проверка: `drivers_follow_the_blocks`
- [x] 4 Генератор: `ethernet`, `ntp` на Ethernet — проверка: тест вывода
- [x] 5 Симулятор: заглушка op 6–8, тест — проверка: `wasm_emit`
- [x] 6 Пример `examples/net_eth_wasm`, справка, `docs.md`, размеры — проверка: `crablogic check`
