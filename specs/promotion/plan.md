# Продвижение — план

## Фазы
- **Подготовка (нед. 1–6):** локализация, лендинг, README, видео, примеры, релизные сборки, Discord/Discussions.
- **Мягкий запуск (мес. 2):** r/esp32, r/arduino, r/homeautomation, Arduino Forum, ESP32.com, Home Assistant Community.
  Формат: «I built X, demo, feedback welcome». Цель — 20–50 пользователей и обратная связь.
- **Публичный запуск (мес. 3):** Show HN, Product Hunt, r/rust, This Week in Rust, Hackaday (tips),
  Hackster.io, Instructables.
- **Постоянная работа (мес. 4+):** YouTube-туториал раз в 2 недели, статьи под SEO
  («ESP32 function block diagram», «ESP32 PLC programming»), обзоры у авторов каналов про ESP32,
  блок Home Assistant / MQTT-discovery.

## Затронутые части репозитория
- `site/` (новый): статический лендинг, без бэкенда.
- `README.md`, `README.ru.md`, `LICENSE`, `CONTRIBUTING.md`, `.github/ISSUE_TEMPLATE/`.
- `examples/`: новые примеры (термостат, полив, MQTT-датчик, Modbus).
- `help/` и `help/ru/`: английский основной, русский — перевод (см. `specs/i18n`).
- Релизы: `build-dist`, Windows и Linux.

## Ритм 10 ч/нед
Контент 3 · сообщество 2 · лендинг/SEO 2 · поддержка и партнёры 2 · метрики 1.

## Риски
- Без английского интерфейса всё блокируется — первая задача.
- Поддержка съедает время — сильная справка и FAQ, ответы пакетом раз в день.
- Windows-only отпугивает maker-аудиторию — сборка под Linux до публичного запуска.
- Один автор — публиковать roadmap и changelog.
