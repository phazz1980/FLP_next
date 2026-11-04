# MQTT в модулях: задачи

- [x] 1 Топик — параметр `runtime` (константа + галочка «Вход») в `mqtt_publish`/`mqtt_subscribe` и C++-вывод (повторная подписка при смене) — проверка: снимок `net_params`
- [x] 2 Мастер: `FLP_MQTT`, op 9–16, слоты подписок — проверка: сборка C3 и ESP32
- [x] 3 `flp-build`: признак `mqtt` — проверка: `drivers_follow_the_blocks`
- [x] 4 Генератор: `mqtt`, `mqtt_publish`, `mqtt_subscribe` — проверка: тест вывода
- [x] 5 Симулятор: заглушка op 9–16, тест публикации и приёма — проверка: `wasm_emit`
- [x] 6 Пример `examples/mqtt_wasm`, справка, `docs.md`, размеры — проверка: `flp check`
