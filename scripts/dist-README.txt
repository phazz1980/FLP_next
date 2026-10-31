CrabLogic — визуальное программирование ESP32 (FBD). Ядро написано на Rust.

Запуск: crablogic-editor.exe. Папки library и libs должны лежать рядом с exe.

Сборка и заливка прошивок:
  arduino-cli.exe (Arduino CLI, лицензия GPLv3, https://github.com/arduino/arduino-cli)
  ищется в папке программы, затем в PATH.
  Ядро ESP32 ищется в папке esp32core рядом с программой, затем в системной
  (%LOCALAPPDATA%\Arduino15).
Если чего-то нет, программа при запуске предложит скачать arduino-cli 1.5.1 и ядро
ESP32 3.3.12 (несколько сотен МБ) в свою папку.

Примеры схем — в папке examples.
