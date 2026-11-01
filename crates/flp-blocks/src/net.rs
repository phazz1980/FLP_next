//! Firmware code of the network interfaces: Wi-Fi and Ethernet W5500, with the services
//! they host (web server, firmware upload page, update over the network).

use crate::cpp::{CppCtx, CppParts, Lines};

/// Firmware upload from the web page (raw HTTP POST streamed into `Update`, port 8080).
/// `@ID@` is the block id, `@NET@` the interface object, `@PASS@` the update password.
const UPLOAD_FUNCTIONS: &str = r##"void @ID@_up_reply(const char *status, const char *text) {
  String r = String("HTTP/1.1 ") + status + "\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\nContent-Length: " + String(strlen(text)) + "\r\n\r\n" + text;
  @ID@_up_c->write((const uint8_t *)r.c_str(), r.length());
}

void @ID@_up_fail(const char *status, const char *text) {
  if (@ID@_up_st == 2) {
    Update.abort();
  }
  @ID@_up_reply(status, text);
  @ID@_up_c->stop();
  @ID@_up_c = nullptr;
  @ID@_up_st = 0;
}

// Header line: the request line is checked for path and password, Content-Length is remembered.
void @ID@_up_header_line() {
  char *l = @ID@_up_line;
  if (strncmp(l, "POST /update", 12) == 0) {
    String expect = String("?p=") + @PASS@ + " ";
    @ID@_up_auth = strlen(@PASS@) == 0 || strstr(l, expect.c_str()) != nullptr;
  } else if (strncasecmp(l, "Content-Length:", 15) == 0) {
    @ID@_up_len = (uint32_t)strtoul(l + 15, nullptr, 10);
  }
}

void @ID@_up_poll() {
  if (!@ID@_up_on) {
    if (millis() - @ID@_up_try > 2000) {
      @ID@_up_try = millis();
      @ID@_up_on = @ID@_up.begin(@NET@, 8080);
    }
    return;
  }
  @ID@_up.pool();
  if (@ID@_up_c == nullptr) {
    @ID@_up_c = @ID@_up.takeNewClient();
    if (@ID@_up_c == nullptr) {
      return;
    }
    @ID@_up_st = 1;
    @ID@_up_ll = 0;
    @ID@_up_len = 0;
    @ID@_up_got = 0;
    @ID@_up_auth = false;
    @ID@_up_t = millis();
  }
  if (@ID@_up_st == 3) {
    if (millis() - @ID@_up_t > 800) {
      ESP.restart();
    }
    return;
  }
  if (!@ID@_up_c->active() || millis() - @ID@_up_t > 10000) {
    if (@ID@_up_st == 2) {
      Update.abort();
    }
    @ID@_up_c->stop();
    @ID@_up_c = nullptr;
    @ID@_up_st = 0;
    return;
  }
  if (@ID@_up_st == 1) {
    for (uint16_t n = 0; n < 512 && @ID@_up_st == 1; n++) {
      int b = @ID@_up_c->read();
      if (b < 0) {
        break;
      }
      @ID@_up_t = millis();
      if (b == '\n') {
        @ID@_up_line[@ID@_up_ll] = 0;
        if (@ID@_up_ll > 0 && @ID@_up_line[@ID@_up_ll - 1] == '\r') {
          @ID@_up_line[@ID@_up_ll - 1] = 0;
        }
        bool empty = @ID@_up_line[0] == 0;
        if (!empty) {
          @ID@_up_header_line();
        }
        @ID@_up_ll = 0;
        if (empty) {
          if (!@ID@_up_auth) {
            @ID@_up_fail("403 Forbidden", "Неверный пароль");
          } else if (@ID@_up_len == 0 || !Update.begin(@ID@_up_len)) {
            @ID@_up_fail("500 Internal Server Error", "Нет места под прошивку");
          } else {
            @ID@_up_st = 2;
          }
        }
      } else if (@ID@_up_ll < sizeof(@ID@_up_line) - 1) {
        @ID@_up_line[@ID@_up_ll++] = (char)b;
      }
    }
  }
  if (@ID@_up_st == 2) {
    for (uint8_t n = 0; n < 4 && @ID@_up_got < @ID@_up_len; n++) {
      uint8_t buf[512];
      uint32_t want = @ID@_up_len - @ID@_up_got;
      int got = @ID@_up_c->read(buf, want < sizeof(buf) ? want : sizeof(buf));
      if (got <= 0) {
        break;
      }
      @ID@_up_t = millis();
      if (Update.write(buf, (size_t)got) != (size_t)got) {
        @ID@_up_fail("500 Internal Server Error", "Ошибка записи прошивки");
        return;
      }
      @ID@_up_got += (uint32_t)got;
    }
    if (@ID@_up_got >= @ID@_up_len) {
      if (Update.end(true)) {
        @ID@_up_reply("200 OK", "Прошивка загружена, устройство перезагружается");
        @ID@_up_st = 3;
        @ID@_up_t = millis();
      } else {
        @ID@_up_fail("500 Internal Server Error", "Прошивка не принята");
      }
    }
  }
}"##;

/// Which services the block hosts. A setting taken from a variable is switched while
/// running, so its code is always built in.
struct Services {
    web: bool,
    update: bool,
    ota: bool,
    /// Address settings come from variables: re-applied when they change.
    dynamic: bool,
    /// Services are polled at most once per period.
    period: bool,
}

impl Services {
    fn of(e: &CppCtx, settings: &[&str]) -> Self {
        let on = |n: &str| e.flag(n) || e.is_bound(n);
        let web = on("web");
        Self {
            web,
            update: web && on("web_update"),
            ota: on("ota"),
            dynamic: settings.iter().any(|n| e.is_bound(n)),
            period: e.int("net_period", 0) > 0,
        }
    }
}

pub(crate) const MIX: &str = "\
// Settings signature (FNV-1a): compared every cycle without building a String.
uint32_t @ID@_cfg = 0;
bool @ID@_cfg_set = false;
static void @ID@_mix(uint32_t &h, const char *s) {
  for (; *s; s++) h = (h ^ (uint8_t)*s) * 16777619u;
  h = (h ^ '|') * 16777619u;
}
static void @ID@_mix(uint32_t &h, const String &s) { @ID@_mix(h, s.c_str()); }";

/// The update password as a C string expression; from an input it is a `String`.
fn ota_pass(e: &CppCtx) -> String {
    if e.is_bound("ota_password") {
        format!("String({}).c_str()", e.param("ota_password"))
    } else {
        e.param("ota_password").to_owned()
    }
}

fn upload_functions(e: &CppCtx, net: &str) -> String {
    UPLOAD_FUNCTIONS
        .replace("@ID@", &e.id)
        .replace("@PASS@", &ota_pass(e))
        .replace("@NET@", net)
}

/// Globals of the services: web server, update over the network, upload page.
fn service_globals(l: &mut Lines, e: &CppCtx, s: &Services, net: &str, with_define: bool) {
    let id = &e.id;
    if s.web {
        l.line("// Web server (port 80): the web_ui block adds its pages to it.")
            .line(format!("FLProgWebServer {id}_web(&{net}, 80);"));
        if with_define {
            l.line("// Shared web server: the web_ui block adds its pages to it.")
                .line(format!("FLProgWebServer &flpWebSrv = {id}_web;"));
        }
    }
    if s.ota {
        l.line("// Update over the network (ArduinoOTA through the shared interface).")
            .line(format!("FLProgNetworkOta {id}_ota;"))
            .line(format!("bool {id}_ota_on = false;"));
        if e.is_bound("ota_password") {
            l.line(format!("String {id}_ota_pass;"));
        }
    }
    if s.update {
        if with_define {
            l.line("#define FLP_WEB_UPD 1");
        }
        l.line(
            "// Firmware upload from the web page: raw HTTP POST streamed into Update (port 8080).",
        )
        .line(format!("FLProgNetworkTcpServer {id}_up;"))
        .line(format!(
            "FLProgNetworkTcpServerClient *{id}_up_c = nullptr;"
        ))
        .line(format!("bool {id}_up_on = false;"))
        .line(format!(
            "uint8_t {id}_up_st = 0;  // 0 idle, 1 headers, 2 body, 3 done (restart pending)"
        ))
        .line(format!("char {id}_up_line[160];"))
        .line(format!("uint8_t {id}_up_ll = 0;"))
        .line(format!(
            "uint32_t {id}_up_len = 0, {id}_up_got = 0, {id}_up_t = 0, {id}_up_try = 0;"
        ))
        .line(format!("bool {id}_up_auth = false;"));
    }
}

fn period_globals(l: &mut Lines, id: &str) {
    l.line("// Network services (MQTT, web, OTA) are polled at most once per period.")
        .line(format!("uint32_t {id}_net_t = 0;"))
        .line(format!("bool {id}_net_due = true;"));
}

/// Polls the interface, once per period when one is set.
fn pool(l: &mut Lines, e: &CppCtx, net: &str) {
    let id = &e.id;
    if e.int("net_period", 0) > 0 {
        l.line("{")
            .line("  uint32_t now = millis();")
            .line(format!(
                "  {id}_net_due = (uint32_t)(now - {id}_net_t) >= {};",
                e.param("net_period")
            ))
            .line(format!("  if ({id}_net_due) {{"))
            .line(format!("    {id}_net_t = now;"))
            .line("  }")
            .line("}")
            .line(format!("if ({id}_net_due) {{"))
            .line(format!("  {net}.pool();"))
            .line("}");
    } else {
        l.line(format!("{net}.pool();"));
    }
}

fn service_loop(l: &mut Lines, e: &CppCtx, s: &Services, net: &str) {
    let id = &e.id;
    let due = if s.period {
        format!("{id}_net_due && ")
    } else {
        String::new()
    };
    if s.web {
        l.line(format!("if ({due}{}) {{", e.flag_code("web")))
            .line(format!("  {id}_web.pool();"))
            .line("}");
    }
    if s.update {
        l.line(format!(
            "if ({due}{} && {}) {{",
            e.flag_code("web"),
            e.flag_code("web_update")
        ))
        .line(format!("  {id}_up_poll();"))
        .line("}");
    }
    if s.ota {
        let pass = ota_pass(e);
        // From an input the password lives in a String, so the library never keeps a
        // pointer to a temporary.
        let bound = e.is_bound("ota_password");
        let held = if bound {
            format!("{id}_ota_pass.c_str()")
        } else {
            pass.clone()
        };
        l.line(format!("if ({due}{}) {{", e.flag_code("ota")))
            .line(format!("  if (!{id}_ota_on) {{"))
            .line(format!("    {id}_ota_on = true;"));
        if bound {
            l.line(format!("    {id}_ota_pass = {pass};"));
        }
        l.line(format!("    if ({held}[0] != 0) {{"))
            .line(format!("      {id}_ota.setPasswordValue({held});"))
            .line("    }")
            .line(format!("    {id}_ota.begin({net});"));
        if bound {
            l.line(format!(
                "  }} else if (strcmp({id}_ota_pass.c_str(), {pass}) != 0) {{"
            ))
            .line("    // Password written while running: the next upload asks for the new one.")
            .line(format!("    {id}_ota_pass = {pass};"))
            .line(format!(
                "    if ({held}[0] != 0) {id}_ota.setPasswordValue({held});"
            ))
            .line("  }");
        } else {
            l.line("  }");
        }
        l.line(format!("  {id}_ota.pool();")).line("}");
    }
}

/// DHCP or the fixed address, gateway and subnet.
fn address(l: &mut Lines, net: &str, e: &CppCtx) {
    l.line(format!("if ({}) {{", e.flag_code("dhcp")))
        .line(format!("  {net}.setDhcp();"))
        .line("} else {")
        .line("  IPAddress addr;")
        .line(format!("  addr.fromString({});", e.param("ip")))
        .line(format!("  {net}.localIP(addr);"))
        .line(format!("  addr.fromString({});", e.param("gateway")))
        .line(format!("  {net}.gateway(addr);"))
        .line(format!("  {net}.dns(addr);"))
        .line(format!("  addr.fromString({});", e.param("subnet")))
        .line(format!("  {net}.subnet(addr);"))
        .line(format!("  {net}.resetDhcp();"))
        .line("}");
}

pub(crate) fn indent(text: &str, by: usize) -> String {
    let pad = " ".repeat(by);
    text.lines()
        .map(|t| format!("{pad}{t}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wifi_apply(e: &CppCtx, client: bool, ap: bool) -> String {
    let mut l = Lines::default();
    if client {
        l.line(format!("flpWifi.setClientSsid({});", e.param("ssid")))
            .line(format!(
                "flpWifi.setClientPassword({});",
                e.param("password")
            ));
        address(&mut l, "flpWifi", e);
    }
    if ap {
        l.line(format!("flpWifi.setApSsid({});", e.param("ap_ssid")))
            .line(format!(
                "flpWifi.setApPassword({});",
                e.param("ap_password")
            ));
    }
    l.finish()
}

fn functions(e: &CppCtx, s: &Services, net: &str) -> String {
    if s.update {
        upload_functions(e, net)
    } else {
        String::new()
    }
}

pub(crate) fn wifi(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let s = Services::of(
        e,
        &[
            "ssid",
            "password",
            "dhcp",
            "ip",
            "gateway",
            "subnet",
            "ap_ssid",
            "ap_password",
        ],
    );
    // 0 client, 1 access point, 2 both, 3 chosen by the mode input
    let mode = e.int("wifi_mode", 0);
    let (client, ap) = (mode != 1, mode != 0);
    let mut g = Lines::default();
    g.line("// Shared network interface: Modbus TCP and other network blocks use it.")
        .line("FLProgOnBoardWifiInterface flpWifi;");
    if s.period {
        period_globals(&mut g, id);
    }
    g.line("// Applied radio state: 0 off, 1 client, 2 access point, 3 both; 255 not yet.")
        .line(format!("uint8_t {id}_state = 255;"));
    if s.dynamic {
        g.line(MIX.replace("@ID@", id));
    }
    if e.flag("out_address") {
        g.line("// Address shown on the address output; the text is rebuilt only when it changes.")
            .line(format!("uint32_t {id}_ip_raw = 0;"));
    }
    service_globals(&mut g, e, &s, "flpWifi", true);

    let mut su = Lines::default();
    if !s.dynamic {
        su.line(wifi_apply(e, client, ap));
    }
    if s.ota {
        su.line(format!("{id}_ota.setHostname(\"{id}\");"));
    }

    let mut l = Lines::default();
    if s.dynamic {
        l.line("{")
            .line("  // Settings written while running are applied when they change.")
            .line("  uint32_t cfg = 2166136261u;");
        for n in ["ssid", "password"] {
            l.line(format!("  {id}_mix(cfg, {});", e.param(n)));
        }
        l.line(format!(
            "  {id}_mix(cfg, ({}) ? \"1\" : \"0\");",
            e.flag_code("dhcp")
        ));
        for n in ["ip", "gateway", "subnet", "ap_ssid", "ap_password"] {
            l.line(format!("  {id}_mix(cfg, {});", e.param(n)));
        }
        l.line(format!("  if (!{id}_cfg_set || cfg != {id}_cfg) {{"))
            .line(format!("    bool first = !{id}_cfg_set;"))
            .line(format!("    {id}_cfg_set = true;"))
            .line(format!("    {id}_cfg = cfg;"))
            .line(indent(&wifi_apply(e, client, ap), 4))
            .line("    if (!first) {")
            .line("      flpWifi.clientOff();")
            .line("      flpWifi.apOff();")
            .line(format!("      {id}_state = 255;"))
            .line("    }")
            .line("  }")
            .line("}");
    }
    l.line("{")
        .line("  // Client and access point follow the inputs; switched only when they change.");
    if mode == 3 {
        l.line(format!("  int32_t mode = {};", e.input("mode")))
            .line("  if (mode < 0 || mode > 2) {")
            .line("    mode = 0;")
            .line("  }");
    } else {
        l.line(format!("  int32_t mode = {};", e.param("wifi_mode")));
    }
    if e.flag("use_en") {
        l.line(format!(
            "  uint8_t state = {} ? (uint8_t)(mode + 1) : 0;",
            e.input("en")
        ));
    } else {
        l.line("  uint8_t state = (uint8_t)(mode + 1);");
    }
    l.line(format!("  if (state != {id}_state) {{"))
        .line(format!("    {id}_state = state;"))
        .line("    if (state & 1) {")
        .line("      flpWifi.clientOn();")
        .line("    } else {")
        .line("      flpWifi.clientOff();")
        .line("    }")
        .line("    if (state & 2) {")
        .line("      flpWifi.apOn();")
        .line("    } else {")
        .line("      flpWifi.apOff();")
        .line("    }")
        .line("  }")
        .line("}");
    pool(&mut l, e, "flpWifi");
    let has = |p: &str| e.output.contains_key(p);
    if e.flag("out_ready") && client && has("ready") {
        l.line(format!("{} = flpWifi.clientIsReady();", e.output("ready")));
    }
    if e.flag("out_ap_ready") && ap && has("ap_ready") {
        l.line(format!("{} = flpWifi.apIsReady();", e.output("ap_ready")));
    }
    if e.flag("out_status") {
        l.line(format!("{} = flpWifi.getStatus();", e.output("status")));
    }
    if e.flag("out_error") && client && has("error") {
        l.line(format!("{} = flpWifi.getError();", e.output("error")));
    }
    if e.flag("out_address") {
        l.line("{")
            .line("  uint32_t ip = 0;")
            .line("  if (flpWifi.clientIsReady()) {")
            .line("    ip = (uint32_t)flpWifi.localIP();")
            .line("  } else if (flpWifi.apIsReady()) {")
            .line("    ip = (uint32_t)flpWifi.accessPointIP();")
            .line("  }")
            .line(format!("  if (ip != {id}_ip_raw) {{"))
            .line(format!("    {id}_ip_raw = ip;"))
            .line(format!(
                "    {} = ip ? IPAddress(ip).toString() : String();",
                e.output("address")
            ))
            .line("  }")
            .line("}");
    }
    service_loop(&mut l, e, &s, "flpWifi");

    CppParts {
        global: g.finish(),
        setup: su.finish(),
        looped: l.finish(),
        functions: functions(e, &s, "flpWifi"),
        ..Default::default()
    }
}

pub(crate) fn ethernet(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let s = Services::of(e, &["dhcp", "ip", "gateway", "subnet"]);
    let mut g = Lines::default();
    if s.dynamic {
        g.line(MIX.replace("@ID@", id));
    }
    g.line("// Shared network interface: Modbus TCP and other network blocks use it.")
        .line(format!("FLProgWiznetInterface flpEth({});", e.param("cs")));
    if s.period {
        period_globals(&mut g, id);
    }
    service_globals(&mut g, e, &s, "flpEth", false);

    let mut su = Lines::default();
    su.line(format!(
        "flpEth.mac(0x78, 0xAC, 0xC0, 0x2C, 0x3E, {});",
        e.param("mac")
    ));
    let mut apply = Lines::default();
    address(&mut apply, "flpEth", e);
    let apply = apply.finish();
    if !s.dynamic {
        su.line(&apply);
    }
    if s.ota {
        su.line(format!("{id}_ota.setHostname(\"{id}\");"));
    }

    let mut l = Lines::default();
    if s.dynamic {
        l.line("{")
            .line("  // Settings written while running are applied when they change.")
            .line("  uint32_t cfg = 2166136261u;")
            .line(format!(
                "  {id}_mix(cfg, ({}) ? \"1\" : \"0\");",
                e.flag_code("dhcp")
            ));
        for n in ["ip", "gateway", "subnet"] {
            l.line(format!("  {id}_mix(cfg, {});", e.param(n)));
        }
        l.line(format!("  if (!{id}_cfg_set || cfg != {id}_cfg) {{"))
            .line(format!("    {id}_cfg_set = true;"))
            .line(format!("    {id}_cfg = cfg;"))
            .line(indent(&apply, 4))
            .line("  }")
            .line("}");
    }
    pool(&mut l, e, "flpEth");
    l.line(format!("{} = flpEth.ethernetIsReady();", e.output("ready")));
    service_loop(&mut l, e, &s, "flpEth");

    CppParts {
        global: g.finish(),
        setup: su.finish(),
        looped: l.finish(),
        functions: functions(e, &s, "flpEth"),
        ..Default::default()
    }
}
