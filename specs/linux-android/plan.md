# План: Linux и Android

## Linux
- `.github/workflows/linux.yml`: `ubuntu-latest`, `cargo build --release -p flp-editor -p flp-cli`,
  архив `crablogic-linux-x86_64.tar.gz` (редактор, `flp`, `library/`, `help/`), загрузка в релиз.
- Места с `cfg(windows)`, `.exe`, `APPDATA`: `flp-build` (`lib.rs`, `direct.rs`, `wasm_c.rs`),
  `flp-core/local.rs`, `flp-editor` (`app.rs`, `assoc.rs`, `selfupdate.rs`, `update.rs`, `theme.rs`),
  `flp-pdf`. Для каждого — замена через `dirs`/`cfg(unix)`; `selfupdate` и `assoc` на Linux отключить.
- `serialport` уже без `libudev` (`default-features = false`): порты ищутся по `/dev/ttyUSB*`, `/dev/ttyACM*`.
  Права: группа `dialout` — описать в справке.
- Риск: `rfd` (диалоги файлов) требует `xdg-desktop-portal` или GTK3; взять фичу `xdg-portal`.

## Android
- Крейт `flp-android` (cdylib): `android-activity` + eframe, тот же `DemoApp`-подобный слой над ядром.
  Открытие проекта — Storage Access Framework через JNI (`rfd` на Android не работает).
- USB: транспорт в `flp-debug` за трейтом (он уже отдельный слой). Реализация для Android — через JNI
  и `UsbManager` (`UsbDeviceConnection`, bulk-эндпойнты), драйверы CDC-ACM/CP210x/CH340 — либо
  готовая библиотека `usb-serial-for-android` (Java, подключается в `build.gradle`).
- Интерфейс: сенсорный — крупные цели нажатия, жесты pan/zoom вместо Ctrl+колесо; начинаем с
  того, что уже есть в `flp-demo` (просмотр, симуляция, правка параметров), затем вкладка «Отладка».
- Сборка: `xbuild` или `cargo-apk`; в Actions — `ubuntu` + Android SDK/NDK; APK кладём в релиз.
- Риски: JNI-склейка для USB — самая трудоёмкая часть; разрешение `USB_PERMISSION` и вторая
  активность при подключении платы; энергосбережение при длительной отладке.

## Порядок
1. Linux (малые правки + Actions).
2. Android: каркас APK с просмотром и симуляцией.
3. Android: USB-транспорт и вкладка отладки.
