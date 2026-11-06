# Ethernet в модулях: план (задача 5b)

## Подход
Как в задаче 5: интерфейсом W5500 владеет мастер (`FLProgWiznetInterface`, создаётся при первом применении
настроек, потому что вывод CS известен только из модуля), модуль пишет запрос и читает снимок.
Wi-Fi и Ethernet независимы: свои настройки, свои `ready` и `address`.
Мастер собирается с нужной частью: `CRAB_NET` — общее (NTP, текст), `CRAB_WIFI`, `CRAB_ETH` — интерфейсы.

## ABI (к `net`, права те же)
- op 0, код 3: интерфейс NTP (0 Wi-Fi, 1 Ethernet); задаётся до кода 64.
- op 6: число Ethernet: a — код (1 dhcp, 2 sck, 3 miso, 4 mosi, 5 cs, 6 последний байт MAC, 63 настройки готовы), b — значение.
- op 7: текст Ethernet: a — код (1 ip, 2 gateway, 3 subnet), b — указатель, c — длина.
- op 8: чтение Ethernet: a — поле (0 готов, 1 IP как число).

## Файлы
- `library/ethernet/block.toml`: параметры `sck`, `miso`, `mosi`, выход `address` (`out_address`).
- `crab-blocks/src/net.rs`: `setSpiPins` (только при нестандартных), строка адреса.
- `master.ino`: секции `CRAB_WIFI`/`CRAB_ETH`, `netPoll`, `h_net` op 6–8.
- `crab-build/master.rs`: `MasterFeatures.wifi/eth`, `ethernet` → `CRAB_ETH`, `wifi` → `CRAB_WIFI`.
- `crab-codegen/src/wasm/net.rs`: `ethernet_block`, `ntp` с `net = "ethernet"`.
- `crab-sim/src/mcu.rs`: заглушка op 6–8; тест; `examples/net_eth_wasm`; справка и `docs.md`.

## Риски
Размер мастера с обоими интерфейсами (замерить); блок `ethernet` скрытый (`hidden`) — проверить, как он попадает в схему.
