// Сгенерировано FLP_next из проекта «start_stop». Ручные правки будут перезаписаны.
// Порядок выполнения: start → stop → run → motor → fan_delay → fan → stopped → bell → buzzer → started → starts

#include <Arduino.h>

// Выходы блоков
bool start_out = false;
bool stop_out = false;
bool run_q = false;
bool fan_delay_out = false;
bool stopped_out = false;
bool bell_out = false;
bool started_out = false;
int32_t starts_count = 0;

// fan_delay: timer_off
bool fan_delay_on = false;
unsigned long fan_delay_off_at = 0;

// stopped: falling_edge
bool stopped_prev = false;

// bell: pulse
bool bell_prev = false;
bool bell_active = false;
unsigned long bell_start = 0;

// started: rising_edge
bool started_prev = false;

// starts: counter
bool starts_up_prev = false;
bool starts_down_prev = false;

void setup() {
  // start: digital_in
  pinMode(12, INPUT_PULLUP);

  // stop: digital_in
  pinMode(14, INPUT_PULLUP);

  // motor: digital_out
  pinMode(26, OUTPUT);

  // fan: digital_out
  pinMode(25, OUTPUT);

  // buzzer: digital_out
  pinMode(27, OUTPUT);
}

void loop() {
  // start: digital_in
  start_out = !digitalRead(12);

  // stop: digital_in
  stop_out = !digitalRead(14);

  // run: rs_trigger
  if (stop_out) {
    run_q = false;
  } else if (start_out) {
    run_q = true;
  }

  // motor: digital_out
  digitalWrite(26, (run_q) ? HIGH : LOW);

  // fan_delay: timer_off
  if (run_q) {
    fan_delay_on = true;
    fan_delay_out = true;
  } else {
    if (fan_delay_on) {
      fan_delay_on = false;
      fan_delay_off_at = millis();
    }
    fan_delay_out = fan_delay_out && millis() - fan_delay_off_at < 30000UL;
  }

  // fan: digital_out
  digitalWrite(25, (fan_delay_out) ? HIGH : LOW);

  // stopped: falling_edge
  stopped_out = !run_q && stopped_prev;
  stopped_prev = run_q;

  // bell: pulse
  if (stopped_out && !bell_prev && !bell_active) {
    bell_active = true;
    bell_start = millis();
  }
  bell_prev = stopped_out;
  if (bell_active && millis() - bell_start >= 2000UL) {
    bell_active = false;
  }
  bell_out = bell_active;

  // buzzer: digital_out
  digitalWrite(27, (bell_out) ? HIGH : LOW);

  // started: rising_edge
  started_out = run_q && !started_prev;
  started_prev = run_q;

  // starts: counter
  if (false) {
    starts_count = 0;
  } else {
    if (started_out && !starts_up_prev) {
      starts_count++;
    }
    if (false && !starts_down_prev) {
      starts_count--;
    }
  }
  starts_up_prev = started_out;
  starts_down_prev = false;
}
