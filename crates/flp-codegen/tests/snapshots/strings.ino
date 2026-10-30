// Сгенерировано CrabLogic из проекта «strings». Ручные правки будут перезаписаны.
// Порядок выполнения: read → join → write

#include <Arduino.h>
#include <Preferences.h>

// Выходы блоков
String read_out = String();
String join_out = String();

// Переменные
String var_status = String("старт");
int32_t var_count = 0;

// Сохранение переменных во флеш
Preferences flp_retain;
uint32_t flp_retain_ms = 0;
String var_status_saved;  // последнее сохранённое «status»

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  read_out.reserve(32);
  join_out.reserve(32);
  var_status.reserve(32);

  // Восстановление переменных из флеша
  flp_retain.begin("flp_vars", false);
  if (flp_retain.isKey("v668e56fa")) {  // status
    var_status = flp_retain.getString("v668e56fa");
  }
  var_status_saved = var_status;
}

void loop() {
  // read: var_get
  read_out = var_status;

  // join: text_join
  join_out = read_out + String(" — ok");

  // write: var_set
  var_status = join_out;

  // Сохранение переменных во флеш: только изменившиеся значения
  if (millis() - flp_retain_ms >= 1000) {
    flp_retain_ms = millis();
    if (var_status != var_status_saved) {  // status
      var_status_saved = var_status;
      flp_retain.putString("v668e56fa", var_status);
    }
  }
}
