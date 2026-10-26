// Сгенерировано FLP_next из проекта «web_ui». Ручные правки будут перезаписаны.
// Порядок выполнения: wifi → ui → head → sw → btn → sld → inp → val → bar → any → lamp → led → txt → info

#include <Arduino.h>
#include <flprogNetwork.h>
#include <flprogWebServer.h>
#include <service/flprogNetworkOta.h>
#include <Update.h>

// Выходы блоков
bool wifi_ready = false;
String wifi_address = String();
bool sw_out = false;
bool btn_out = false;
float sld_out = 0.0f;
float inp_out = 0.0f;
bool any_out = false;
String txt_out = String();

// web_ui: общий код
// Web interface registry: widget blocks register their items, the web_ui block serves them.
struct FlpUiItem {
  const char *kind;
  const char *caption;
  const char *unit;
  uint8_t col, row, w, h;
  bool rw;      // the page may change the value
  float value;
  float lo, hi, step;
  uint8_t dec;  // digits after the point on the page
  String text;
};
#define FLP_UI_MAX 32
FlpUiItem *flpUiItems[FLP_UI_MAX];
uint8_t flpUiCount = 0;
void flpUiAdd(FlpUiItem *item) {
  if (flpUiCount < FLP_UI_MAX) flpUiItems[flpUiCount++] = item;
}
#define FLP_WEBUI 1

// wifi: wifi
// Shared network interface: Modbus TCP and other network blocks use it.
FLProgOnBoardWifiInterface flpWifi;
// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.
uint8_t wifi_state = 255;
// Address shown on the address output; the text is rebuilt only when it changes.
uint32_t wifi_ip_raw = 0;
// Stub web page: technical data of the device (port 80).
FLProgWebServer wifi_web(&flpWifi, 80);
// Shared web server: the web_ui block adds its pages to it.
FLProgWebServer &flpWebSrv = wifi_web;
// Update over the network (ArduinoOTA through the shared interface).
FLProgNetworkOta wifi_ota;
// Firmware upload from the web page: raw HTTP POST streamed into Update (port 8080).
FLProgNetworkTcpServer wifi_up;
FLProgNetworkTcpServerClient *wifi_up_c = nullptr;
bool wifi_up_on = false;
uint8_t wifi_up_st = 0;  // 0 idle, 1 headers, 2 body, 3 done (restart pending)
char wifi_up_line[160];
uint8_t wifi_up_ll = 0;
uint32_t wifi_up_len = 0, wifi_up_got = 0, wifi_up_t = 0, wifi_up_try = 0;
bool wifi_up_auth = false;

// ui: web_ui
const char ui_html[] PROGMEM = R"FLPUI(<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FLP_next</title>
<style>
:root{color-scheme:light dark;--b:#fff;--f:#222;--c:#f1f3f6;--a:#2f7de1;--g:#2da44e}
@media(prefers-color-scheme:dark){:root{--b:#14171c;--f:#e6e8eb;--c:#222831;--a:#5aa0f5;--g:#3fb950}}
body{margin:0;padding:12px;background:var(--b);color:var(--f);font:15px system-ui,sans-serif}
h1{font-size:18px;margin:0 0 10px}
#g{display:grid;grid-template-columns:repeat(12,1fr);grid-auto-rows:56px;gap:8px}
.w{background:var(--c);border-radius:8px;padding:6px 10px;display:flex;flex-direction:column;justify-content:center;min-width:0;overflow:hidden}
.w small{opacity:.7;font-size:12px}
.w b{font-size:22px;font-weight:600}
.w.p{padding:0}
button,input{font:inherit;color:inherit}
.btn{border:0;border-radius:8px;background:var(--a);color:#fff;width:100%;height:100%;touch-action:manipulation;user-select:none}
.btn.on{background:var(--g)}
.led{width:22px;height:22px;border-radius:50%;background:#888;flex:none}
.led.on{background:var(--g);box-shadow:0 0 8px var(--g)}
.r{display:flex;align-items:center;gap:8px}
.bar{height:10px;border-radius:5px;background:#8884;overflow:hidden;margin-top:4px}
.bar i{display:block;height:100%;background:var(--a)}
input[type=range]{width:100%}
input[type=number]{width:100%;box-sizing:border-box;background:transparent;border:1px solid #8886;border-radius:6px;padding:4px}
</style></head><body><h1 id="t"></h1><div id="g"></div><script>
var G=document.getElementById('g'),T=document.getElementById('t'),E=[];
function S(i,v){fetch('/ui/set?i='+i+'&v='+v).catch(function(){})}
function h(t,c,x){var e=document.createElement(t);if(c)e.className=c;if(x!=null)e.textContent=x;return e}
function mk(d,i){
 var w=h('div','w'),k=d.k,u=d.u?' '+d.u:'',dn=false;
 w.style.gridColumn=d.c+' / span '+d.w;w.style.gridRow=d.r+' / span '+d.h;
 if(k=='label'){w.appendChild(h('b','',d.l));return[w,function(){}]}
 if(k=='button'||k=='switch'){
  var b=h('button','btn',d.l);w.className='w p';w.appendChild(b);
  if(k=='switch')b.onclick=function(){var n=b.classList.contains('on')?0:1;b.classList.toggle('on',n==1);S(i,n)};
  else{
   b.onpointerdown=function(){dn=true;S(i,1)};
   b.onpointerup=b.onpointerleave=b.onpointercancel=function(){if(dn){dn=false;S(i,0)}}}
  return[w,function(v){b.classList.toggle('on',v!=0)}]}
 if(k=='slider'){
  var s=h('input'),o=h('b','','');s.type='range';s.min=d.a;s.max=d.b;s.step=d.s;
  w.appendChild(h('small','',d.l));w.appendChild(o);w.appendChild(s);
  s.oninput=function(){o.textContent=s.value+u;S(i,s.value)};
  return[w,function(v){if(document.activeElement!=s){s.value=v;o.textContent=(+v)+u}}]}
 if(k=='input'){
  var n=h('input');n.type='number';n.step=d.s;if(d.b>d.a){n.min=d.a;n.max=d.b}
  w.appendChild(h('small','',d.l+u));w.appendChild(n);
  n.onchange=function(){S(i,n.value)};
  return[w,function(v){if(document.activeElement!=n)n.value=v}]}
 if(k=='led'){
  var r=h('div','r'),l=h('div','led');r.appendChild(l);r.appendChild(h('span','',d.l));w.appendChild(r);
  return[w,function(v){l.classList.toggle('on',v!=0)}]}
 if(k=='gauge'){
  var q=h('b','',''),a=h('i'),z=h('div','bar');z.appendChild(a);
  w.appendChild(h('small','',d.l));w.appendChild(q);w.appendChild(z);
  return[w,function(v){q.textContent=(+v).toFixed(d.d)+u;a.style.width=Math.max(0,Math.min(100,(v-d.a)/(d.b-d.a)*100))+'%'}]}
 var x=h('b','','');w.appendChild(h('small','',d.l));w.appendChild(x);
 return[w,function(v){x.textContent=(k=='value'?(+v).toFixed(d.d)+u:v)}]}
function poll(){fetch('/ui/state').then(function(r){return r.json()}).then(function(a){a.forEach(function(v,i){if(E[i])E[i](v)})}).catch(function(){}).then(function(){setTimeout(poll,500)})}
fetch('/ui/layout').then(function(r){return r.json()}).then(function(d){
 T.textContent=d.t;document.title=d.t;
 d.i.forEach(function(x,i){var m=mk(x,i);G.appendChild(m[0]);E.push(m[1])});poll()})
</script></body></html>)FLPUI";

// head: web_label
FlpUiItem head_ui = {"web_label", "Управление", "", 1, 1, 12, 1, false, 0, 0, 0, 0, 0};

// sw: web_switch
FlpUiItem sw_ui = {"web_switch", "Свет", "", 1, 2, 4, 1, true, 0, 0, 0, 0, 0};

// btn: web_button
FlpUiItem btn_ui = {"web_button", "Нажать", "", 5, 2, 4, 1, true, 0, 0, 0, 0, 0};

// sld: web_slider
FlpUiItem sld_ui = {"web_slider", "Ползунок", "%", 1, 3, 8, 1, true, 25.0f, 0.0f, 100.0f, 5.0f, 3};

// inp: web_input
FlpUiItem inp_ui = {"web_input", "Уставка", "°C", 9, 3, 4, 1, true, 21.5f, -50.0f, 150.0f, 0.5f, 3};

// val: web_value
FlpUiItem val_ui = {"web_value", "Ползунок", "%", 1, 4, 4, 2, false, 0, 0, 0, 0, 0};

// bar: web_gauge
FlpUiItem bar_ui = {"web_gauge", "Шкала", "%", 5, 4, 8, 2, false, 0, 0.0f, 100.0f, 0, 1};

// lamp: web_led
FlpUiItem lamp_ui = {"web_led", "Светодиод", "", 9, 2, 4, 1, false, 0, 0, 0, 0, 0};

// info: web_text
FlpUiItem info_ui = {"web_text", "Состояние", "", 1, 6, 12, 1, false, 0, 0, 0, 0, 0};

// wifi: wifi
void wifi_up_reply(const char *status, const char *text) {
  String r = String("HTTP/1.1 ") + status + "\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: " + String(strlen(text)) + "\r\n\r\n" + text;
  wifi_up_c->write((const uint8_t *)r.c_str(), r.length());
}

void wifi_up_fail(const char *status, const char *text) {
  if (wifi_up_st == 2) {
    Update.abort();
  }
  wifi_up_reply(status, text);
  wifi_up_c->stop();
  wifi_up_c = nullptr;
  wifi_up_st = 0;
}

// Header line: the request line is checked for path and password, Content-Length is remembered.
void wifi_up_header_line() {
  char *l = wifi_up_line;
  if (strncmp(l, "POST /update", 12) == 0) {
    String expect = String("?p=") + "" + " ";
    wifi_up_auth = strlen("") == 0 || strstr(l, expect.c_str()) != nullptr;
  } else if (strncasecmp(l, "Content-Length:", 15) == 0) {
    wifi_up_len = (uint32_t)strtoul(l + 15, nullptr, 10);
  }
}

void wifi_up_poll() {
  if (!wifi_up_on) {
    if (millis() - wifi_up_try > 2000) {
      wifi_up_try = millis();
      wifi_up_on = wifi_up.begin(flpWifi, 8080);
    }
    return;
  }
  wifi_up.pool();
  if (wifi_up_c == nullptr) {
    wifi_up_c = wifi_up.takeNewClient();
    if (wifi_up_c == nullptr) {
      return;
    }
    wifi_up_st = 1;
    wifi_up_ll = 0;
    wifi_up_len = 0;
    wifi_up_got = 0;
    wifi_up_auth = false;
    wifi_up_t = millis();
  }
  if (wifi_up_st == 3) {
    if (millis() - wifi_up_t > 800) {
      ESP.restart();
    }
    return;
  }
  if (!wifi_up_c->active() || millis() - wifi_up_t > 10000) {
    if (wifi_up_st == 2) {
      Update.abort();
    }
    wifi_up_c->stop();
    wifi_up_c = nullptr;
    wifi_up_st = 0;
    return;
  }
  if (wifi_up_st == 1) {
    for (uint16_t n = 0; n < 512 && wifi_up_st == 1; n++) {
      int b = wifi_up_c->read();
      if (b < 0) {
        break;
      }
      wifi_up_t = millis();
      if (b == '\n') {
        wifi_up_line[wifi_up_ll] = 0;
        if (wifi_up_ll > 0 && wifi_up_line[wifi_up_ll - 1] == '\r') {
          wifi_up_line[wifi_up_ll - 1] = 0;
        }
        bool empty = wifi_up_line[0] == 0;
        if (!empty) {
          wifi_up_header_line();
        }
        wifi_up_ll = 0;
        if (empty) {
          if (!wifi_up_auth) {
            wifi_up_fail("403 Forbidden", "Неверный пароль");
          } else if (wifi_up_len == 0 || !Update.begin(wifi_up_len)) {
            wifi_up_fail("500 Internal Server Error", "Нет места под прошивку");
          } else {
            wifi_up_st = 2;
          }
        }
      } else if (wifi_up_ll < sizeof(wifi_up_line) - 1) {
        wifi_up_line[wifi_up_ll++] = (char)b;
      }
    }
  }
  if (wifi_up_st == 2) {
    for (uint8_t n = 0; n < 4 && wifi_up_got < wifi_up_len; n++) {
      uint8_t buf[512];
      uint32_t want = wifi_up_len - wifi_up_got;
      int got = wifi_up_c->read(buf, want < sizeof(buf) ? want : sizeof(buf));
      if (got <= 0) {
        break;
      }
      wifi_up_t = millis();
      if (Update.write(buf, (size_t)got) != (size_t)got) {
        wifi_up_fail("500 Internal Server Error", "Ошибка записи прошивки");
        return;
      }
      wifi_up_got += (uint32_t)got;
    }
    if (wifi_up_got >= wifi_up_len) {
      if (Update.end(true)) {
        wifi_up_reply("200 OK", "Прошивка загружена, устройство перезагружается");
        wifi_up_st = 3;
        wifi_up_t = millis();
      } else {
        wifi_up_fail("500 Internal Server Error", "Прошивка не принята");
      }
    }
  }
}

void wifi_page(FLProgNetworkHttpServer *server) {
  char mac[18];
  uint8_t *m = flpWifi.mac();
  snprintf(mac, sizeof(mac), "%02X:%02X:%02X:%02X:%02X:%02X", m[0], m[1], m[2], m[3], m[4], m[5]);
  uint32_t sec = millis() / 1000;
  String page = F("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>wifi</title></head><body><h1>FLP_next</h1><table>");
  page += F("<tr><td>IP-адрес</td><td>"); page += flpWifi.localIP().toString(); page += F("</td></tr>");
  page += F("<tr><td>MAC</td><td>"); page += mac; page += F("</td></tr>");
  page += F("<tr><td>Сигнал Wi-Fi</td><td>"); page += String(flpWifi.rssi()); page += F(" дБм</td></tr>");
  page += F("<tr><td>Аптайм</td><td>"); page += String(sec / 3600); page += F(" ч "); page += String(sec / 60 % 60); page += F(" мин</td></tr>");
  page += F("<tr><td>Сборка</td><td>" __DATE__ " " __TIME__ "</td></tr>");
  page += F("<tr><td>Обновление</td><td>по сети (OTA), имя: "); page += wifi_ota.getName(); page += F("</td></tr>");
  page += F("</table>");
#ifdef FLP_WEBUI
  page += F("<p><a href=\"/ui\">Панель управления</a></p>");
#endif
  page += F("<h2>Обновление прошивки</h2><input type=file id=f accept=.bin><button onclick=\"u()\">Загрузить</button> <span id=s></span><script>function u(){var f=f.files[0];if(!f)return;var p=prompt('Пароль','');s.textContent='...';fetch('http://'+location.hostname+':8080/update?p='+encodeURIComponent(p||''),{method:'POST',headers:{'Content-Type':'text/plain'},body:f}).then(function(r){return r.text()}).then(function(x){s.textContent=x}).catch(function(){s.textContent='ошибка'})}</script>");
  page += F("</body></html>");
  server->send(200, "text/html", page);
}

// ui: web_ui
void ui_esc(String &o, const char *s) {
  for (; *s; s++) {
    if (*s == '"' || *s == '\\') o += '\\';
    if ((uint8_t)*s >= 32) o += *s;
  }
}
void ui_page(FLProgNetworkHttpServer *server) {
  server->sendProgmem(200, "text/html; charset=utf-8", ui_html);
}
void ui_layout(FLProgNetworkHttpServer *server) {
  String o = "{\"t\":\"";
  ui_esc(o, "Панель ESP32");
  o += "\",\"i\":[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    o += "{\"k\":\""; o += (it->kind + 4);
    o += "\",\"l\":\""; ui_esc(o, it->caption);
    o += "\",\"u\":\""; ui_esc(o, it->unit);
    o += "\",\"c\":"; o += it->col;
    o += ",\"r\":"; o += it->row;
    o += ",\"w\":"; o += it->w;
    o += ",\"h\":"; o += it->h;
    o += ",\"d\":"; o += it->dec;
    o += ",\"a\":"; o += String(it->lo, 3);
    o += ",\"b\":"; o += String(it->hi, 3);
    o += ",\"s\":"; o += String(it->step, 3);
    o += '}';
  }
  o += "]}";
  server->send(200, "application/json", o);
}
void ui_state(FLProgNetworkHttpServer *server) {
  String o = "[";
  for (uint8_t i = 0; i < flpUiCount; i++) {
    FlpUiItem *it = flpUiItems[i];
    if (i) o += ',';
    if (!strcmp(it->kind, "web_text")) {
      o += '"'; ui_esc(o, it->text.c_str()); o += '"';
    } else {
      o += String(it->value, (unsigned int)it->dec);
    }
  }
  o += ']';
  server->send(200, "application/json", o);
}
void ui_set(FLProgNetworkHttpServer *server) {
  if (server->hasArgumentKey("i") && server->hasArgumentKey("v")) {
    int i = server->argumentValueAtKey("i").toInt();
    float v = server->argumentValueAtKey("v").toFloat();
    if (i >= 0 && i < flpUiCount && flpUiItems[i]->rw) {
      FlpUiItem *it = flpUiItems[i];
      if (it->hi > it->lo) v = constrain(v, it->lo, it->hi);
      it->value = v;
    }
  }
  server->send(200, "text/plain", "ok");
}

void setup() {
  // Память под строки заранее: меньше дробления кучи.
  wifi_address.reserve(32);
  txt_out.reserve(32);

  // wifi: wifi
  flpWifi.setClientSsid("MyNetwork");
  flpWifi.setClientPassword("");
  if (True) {
    flpWifi.setDhcp();
  } else {
    IPAddress addr;
    addr.fromString("192.168.1.177");
    flpWifi.localIP(addr);
    addr.fromString("192.168.1.1");
    flpWifi.gateway(addr);
    flpWifi.dns(addr);
    addr.fromString("255.255.255.0");
    flpWifi.subnet(addr);
    flpWifi.resetDhcp();
  }
  wifi_web.addHandler("/", wifi_page);
  wifi_ota.setHostname("wifi");
  wifi_ota.begin(flpWifi);

  // ui: web_ui
  flpWebSrv.addHandler("/ui", ui_page);
  flpWebSrv.addHandler("/ui/layout", ui_layout);
  flpWebSrv.addHandler("/ui/state", ui_state);
  flpWebSrv.addHandler("/ui/set", ui_set);

  // head: web_label
  flpUiAdd(&head_ui);

  // sw: web_switch
  flpUiAdd(&sw_ui);

  // btn: web_button
  flpUiAdd(&btn_ui);

  // sld: web_slider
  flpUiAdd(&sld_ui);

  // inp: web_input
  flpUiAdd(&inp_ui);

  // val: web_value
  flpUiAdd(&val_ui);

  // bar: web_gauge
  flpUiAdd(&bar_ui);

  // lamp: web_led
  flpUiAdd(&lamp_ui);

  // led: digital_out
  pinMode(8, OUTPUT);

  // info: web_text
  flpUiAdd(&info_ui);
}

void loop() {
  // wifi: wifi
  {
    // Client and access point follow the inputs; switched only when they change.
    int32_t mode = 0;
    uint8_t state = (uint8_t)(mode + 1);
    if (state != wifi_state) {
      wifi_state = state;
      if (state & 1) {
        flpWifi.clientOn();
      } else {
        flpWifi.clientOff();
      }
      if (state & 2) {
        flpWifi.apOn();
      } else {
        flpWifi.apOff();
      }
    }
  }
  flpWifi.pool();
  wifi_ready = flpWifi.clientIsReady();
  {
    uint32_t ip = 0;
    if (flpWifi.clientIsReady()) {
      ip = (uint32_t)flpWifi.localIP();
    } else if (flpWifi.apIsReady()) {
      ip = (uint32_t)flpWifi.accessPointIP();
    }
    if (ip != wifi_ip_raw) {
      wifi_ip_raw = ip;
      wifi_address = ip ? IPAddress(ip).toString() : String();
    }
  }
  wifi_web.pool();
  wifi_up_poll();
  wifi_ota.pool();

  // sw: web_switch
  sw_out = sw_ui.value != 0;

  // btn: web_button
  btn_out = btn_ui.value != 0;

  // sld: web_slider
  sld_out = sld_ui.value;

  // inp: web_input
  inp_out = inp_ui.value;

  // val: web_value
  val_ui.value = sld_out;

  // bar: web_gauge
  bar_ui.value = sld_out;

  // any: or
  any_out = sw_out || btn_out;

  // lamp: web_led
  lamp_ui.value = any_out ? 1 : 0;

  // led: digital_out
  digitalWrite(8, (any_out) ? HIGH : LOW);

  // txt: num_to_text
  txt_out = "Уставка: ";
  txt_out += String(inp_out, 1);
  txt_out += "";

  // info: web_text
  info_ui.text = txt_out;
}
