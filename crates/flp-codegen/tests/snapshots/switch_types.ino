// Сгенерировано CrabLogic из проекта «switch_types». Ручные правки будут перезаписаны.
// Порядок выполнения: next → hold → mode → msg → tx → blink → led_src → led → route → pwm1 → pwm2 → pwm3

#include <Arduino.h>

// Выходы блоков
bool next_out = false;
bool hold_out = false;
int32_t mode_count = 0;
String msg_out = String();
bool tx_sent = false;
bool blink_out = false;
bool led_src_out = false;
int32_t route_o1 = 0;
int32_t route_o2 = 0;
int32_t route_o3 = 0;

// mode: counter
bool mode_up_prev = false;
bool mode_down_prev = false;

// tx: uart_send
bool tx_prev = false;

// blink: generator
unsigned long blink_start = 0;

void setup() {
  // USB: 115200 бод
  Serial.begin(115200);

  // Память под строки заранее: меньше дробления кучи.
  msg_out.reserve(32);

  // next: digital_in
  pinMode(12, INPUT_PULLUP);

  // hold: digital_in
  pinMode(13, INPUT_PULLUP);

  // led: digital_out
  pinMode(2, OUTPUT);
}

void loop() {
  // next: digital_in
  next_out = !digitalRead(12);

  // hold: digital_in
  hold_out = !digitalRead(13);

  // mode: counter
  if (false) {
    mode_count = 0;
  } else {
    if (next_out && !mode_up_prev) {
      mode_count++;
    }
    if (false && !mode_down_prev) {
      mode_count--;
    }
  }
  mode_up_prev = next_out;
  mode_down_prev = false;

  // msg: mux
  {
    int idx = (int)mode_count;
    if (idx <= 0) {
      msg_out = String("Стоп");
    } else
    if (idx <= 1) {
      msg_out = String("Работа");
    } else
    msg_out = String("Авария");
  }

  // tx: uart_send
  {
    bool fire = true && !tx_prev;
    tx_prev = true;
    tx_sent = false;
    if (fire) {
      const String& text = msg_out;
      const char* ending = "\r\n";
      // A busy port would stall the loop: drop the message if the transmit buffer
      // is (nearly) full. A long one may still wait while the hardware FIFO drains.
      size_t room = text.length() + strlen(ending);
      if ((size_t)Serial.availableForWrite() >= (room < 64 ? room : 64)) {
        Serial.print(text);
        Serial.print(ending);
        tx_sent = true;
      }
    }
  }

  // blink: generator
  if (true) {
    unsigned long phase = (millis() - blink_start) % (1000UL);
    blink_out = phase < 500UL;
  } else {
    blink_start = millis();
    blink_out = false;
  }

  // led_src: switch
  led_src_out = hold_out ? hold_out : blink_out;

  // led: digital_out
  digitalWrite(2, (led_src_out) ? HIGH : LOW);

  // route: dms
  {
    int idx = (int)mode_count;
    route_o1 = idx == 0 ? mode_count : 0;
    route_o2 = idx == 1 ? mode_count : 0;
    route_o3 = idx == 2 ? mode_count : 0;
  }

  // pwm1: pwm_out
  analogWrite(25, (int)(constrain((float)route_o1, 0.0f, 100.0f) * 2.55f + 0.5f));

  // pwm2: pwm_out
  analogWrite(26, (int)(constrain((float)route_o2, 0.0f, 100.0f) * 2.55f + 0.5f));

  // pwm3: pwm_out
  analogWrite(27, (int)(constrain((float)route_o3, 0.0f, 100.0f) * 2.55f + 0.5f));
}
