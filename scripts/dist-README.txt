CrabLogic — визуальное программирование ESP32 (FBD). Ядро написано на Rust.

Запуск: crablogic-editor.exe. Папки library и libs должны лежать рядом с exe.

Для сборки и заливки прошивок нужны arduino-cli и ядро ESP32:
  winget install ArduinoSA.CLI
  arduino-cli core install esp32:esp32
(после установки перезапустите программу)

Примеры схем — в папке examples.
