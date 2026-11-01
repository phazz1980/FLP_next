//! Firmware code of the network service blocks: MQTT and NTP (the Wi-Fi and Ethernet
//! interfaces themselves are in `net.rs`).

use crate::cpp::{CppCtx, CppParts, Lines};

/// Interface object the service binds to: `net = "ethernet"` or Wi-Fi.
fn interface(e: &CppCtx) -> &'static str {
    if e.text("net") == "ethernet" {
        "flpEth"
    } else {
        "flpWifi"
    }
}

const MQTT_FUNCTIONS: &str = "\
void flp_mqtt_receive(const char *topic, const uint8_t *payload, size_t length, bool retained) {
  (void)retained;
  flpMqttTopic = topic;
  flpMqttPayload = \"\";
  flpMqttPayload.reserve(length);
  for (size_t i = 0; i < length; i++) {
    flpMqttPayload += (char)payload[i];
  }
  flpMqttSeq++;
}

// Topic against a filter with + (one level) and # (the rest).
bool flp_mqtt_match(const char *f, const char *t) {
  for (;;) {
    const char *fe = strchr(f, '/');
    const char *te = strchr(t, '/');
    size_t fn = fe ? (size_t)(fe - f) : strlen(f);
    size_t tn = te ? (size_t)(te - t) : strlen(t);
    if (fn == 1 && *f == '#') {
      return true;
    }
    if (!(fn == 1 && *f == '+') && (fn != tn || strncmp(f, t, fn) != 0)) {
      return false;
    }
    if (!fe || !te) {
      return !fe && !te;
    }
    f = fe + 1;
    t = te + 1;
  }
}";

pub(crate) fn mqtt(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let mut g = Lines::default();
    g.line("// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.")
        .line("FLProgNetworkMqttClient flpMqtt;")
        .line(format!("bool {id}_bound = false;"))
        .line("// The last received message, shared by all subscriptions.")
        .line("String flpMqttTopic;")
        .line("String flpMqttPayload;")
        .line("uint32_t flpMqttSeq = 0;")
        .line("");
    let mut l = Lines::default();
    l.line(format!("if (!{id}_bound) {{"))
        .line("  // Attach once; the client reconnects by itself after that.")
        .line(format!("  {id}_bound = flpMqtt.begin({});", interface(e)))
        .line(format!("  if ({id}_bound) {{"))
        .line(format!("    flpMqtt.setBroker({});", e.param("broker")))
        .line(format!("    flpMqtt.setPort({});", e.param("port")));
    if e.text("client_id").is_empty() {
        l.line("    char cid[24];")
            .line(
                "    snprintf(cid, sizeof(cid), \"flp-%08lx\", (unsigned long)ESP.getEfuseMac());",
            )
            .line("    flpMqtt.setClientId(cid);");
    } else {
        l.line(format!(
            "    flpMqtt.setClientId({});",
            e.param("client_id")
        ));
    }
    if !e.text("user").is_empty() {
        l.line(format!(
            "    flpMqtt.setCredentials({}, {});",
            e.param("user"),
            e.param("password")
        ));
    }
    l.line(format!(
        "    flpMqtt.setKeepAlive({});",
        e.param("keepalive")
    ))
    .line("    flpMqtt.setMessageCallback(flp_mqtt_receive);")
    .line("    flpMqtt.setAutoReconnect(true);")
    .line("    flpMqtt.connect();")
    .line("  }")
    .line("}")
    .line(format!("{} = flpMqtt.connected();", e.output("connected")))
    .line(format!("{} = flpMqtt.error();", e.output("error")));
    CppParts {
        defines: format!(
            "#define FLPROG_MQTT_MAX_SUBSCRIPTIONS {}U",
            e.param("max_subscriptions")
        ),
        global: g.finish(),
        once_functions: MQTT_FUNCTIONS.to_owned(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn mqtt_publish(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let t = e.input_type("value");
    let (en, value, sent) = (e.input("en"), e.input("value"), e.output("sent"));
    let trigger = e.int("trigger", 0);
    let mut g = Lines::default();
    g.line(format!("bool {id}_pending = false;"));
    match trigger {
        0 => g.line(format!("bool {id}_prev = false;")),
        1 => {
            let ct = match t {
                "bool" => "bool",
                "int" => "int32_t",
                "float" => "float",
                _ => "String",
            };
            g.line(format!("{ct} {id}_last;"))
                .line(format!("bool {id}_started = false;"))
        }
        _ => g.line(format!("uint32_t {id}_t = 0;")),
    };
    let mut l = Lines::default();
    l.line("{");
    match trigger {
        0 => l
            .line(format!("  if ({en} && !{id}_prev) {{"))
            .line(format!("    {id}_pending = true;"))
            .line("  }")
            .line(format!("  {id}_prev = {en};")),
        1 => {
            if t == "float" && e.num("deadband") > 0.0 {
                l.line(format!(
                    "  bool changed = !{id}_started || fabsf({value} - {id}_last) >= {};",
                    e.param("deadband")
                ));
            } else {
                l.line(format!(
                    "  bool changed = !{id}_started || {value} != {id}_last;"
                ));
            }
            l.line(format!("  if ({en} && changed) {{"))
                .line(format!("    {id}_pending = true;"))
                .line("  }")
        }
        _ => l
            .line(format!(
                "  if ({en} && millis() - {id}_t >= {}) {{",
                e.param("period")
            ))
            .line(format!("    {id}_pending = true;"))
            .line("  }"),
    };
    l.line(format!("  {sent} = false;"))
        .line(format!("  if ({id}_pending && flpMqtt.connected()) {{"));
    match t {
        "string" => l.line(format!("    String text = {value};")),
        "float" => l.line(format!(
            "    String text = String({value}, {});",
            e.param("decimals")
        )),
        "bool" => l.line(format!("    String text = {value} ? \"1\" : \"0\";")),
        _ => l.line(format!("    String text = String({value});")),
    };
    l.line(format!(
        "    if (flpMqtt.publish({}, text.c_str(), {})) {{",
        e.param("topic"),
        if e.flag("retain") { "true" } else { "false" }
    ))
    .line(format!("      {sent} = true;"))
    .line(format!("      {id}_pending = false;"));
    match trigger {
        1 => l
            .line(format!("      {id}_last = {value};"))
            .line(format!("      {id}_started = true;")),
        2 => l.line(format!("      {id}_t = millis();")),
        _ => &mut l,
    };
    l.line("    }").line("  }").line("}");
    CppParts {
        global: g.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn mqtt_subscribe(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let (topic, received) = (e.param("topic"), e.output("received"));
    let mut g = Lines::default();
    g.line(format!("uint32_t {id}_seq = 0;"))
        .line(format!("bool {id}_subscribed = false;"));
    let mut l = Lines::default();
    l.line("{")
        .line("  // The client keeps the subscription across reconnects; ask again only until it is accepted.")
        .line(format!(
            "  if (!{id}_subscribed && flpMqtt.connected()) {{"
        ))
        .line(format!(
            "    {id}_subscribed = flpMqtt.subscribe({topic});"
        ))
        .line("  }")
        .line(format!("  {received} = false;"))
        .line(format!("  if ({id}_seq != flpMqttSeq) {{"))
        .line(format!("    {id}_seq = flpMqttSeq;"))
        .line(format!(
            "    if (flp_mqtt_match({topic}, flpMqttTopic.c_str())) {{"
        ))
        .line(format!("      {received} = true;"));
    match e.text("kind") {
        "int" => l.line(format!(
            "      {} = flpMqttPayload.toInt();",
            e.output("out_i")
        )),
        "float" => l.line(format!(
            "      {} = flpMqttPayload.toFloat();",
            e.output("out_f")
        )),
        "bool" => l
            .line("      {")
            .line("        String p = flpMqttPayload;")
            .line("        p.trim();")
            .line("        p.toLowerCase();")
            .line(format!(
                "        {} = p == \"1\" || p == \"true\" || p == \"on\";",
                e.output("out_b")
            ))
            .line("      }"),
        _ => l.line(format!("      {} = flpMqttPayload;", e.output("out"))),
    };
    l.line("    }").line("  }").line("}");
    CppParts {
        global: g.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn ntp(e: &CppCtx) -> CppParts {
    let id = &e.id;
    let mut g = Lines::default();
    g.line("// The FLProg_Network SNTP service is the same on every board and network: it keeps")
        .line("// the time in UTC itself, the zone and daylight saving are added below.")
        .line(format!("FLProgNetworkNtp {id}_ntp;"))
        .line(format!("bool {id}_bound = false;"))
        .line(format!("uint32_t {id}_tick = 0;"))
        .line(format!("uint32_t {id}_retry = 0;"));
    let mut l = Lines::default();
    l.line(format!("if (!{id}_bound) {{"))
        .line("  // Attach once; the service waits for the network and repeats requests by itself.")
        .line(format!("  {id}_bound = {id}_ntp.begin({});", interface(e)))
        .line(format!("  if ({id}_bound) {{"))
        .line(format!("    {id}_ntp.setServer({});", e.param("server")))
        .line(format!(
            "    {id}_ntp.setRequestPeriodMs({});",
            e.param("period")
        ))
        .line("  }")
        .line("}");
    let parts = [
        "out_hour",
        "out_minute",
        "out_second",
        "out_day",
        "out_month",
        "out_year",
        "out_weekday",
        "out_text",
    ]
    .iter()
    .any(|k| e.flag(k));
    l.line("// The time is read once per second.")
        .line(format!(
            "if ({id}_bound && (uint32_t)(millis() - {id}_tick) >= 1000) {{"
        ))
        .line(format!("  {id}_tick = millis();"))
        .line("  // After a failed request the service would wait for the whole period: ask again in 30 s.")
        .line(format!(
            "  if ({id}_ntp.failed() && (uint32_t)(millis() - {id}_retry) >= 30000) {{"
        ))
        .line(format!("    {id}_retry = millis();"))
        .line(format!("    {id}_ntp.syncNow();"))
        .line("  }")
        .line("  // Stays true while the service asks the server again.")
        .line(format!("  bool ok = {id}_ntp.hasTime();"));
    if e.flag("out_valid") {
        l.line(format!("  {} = ok;", e.output("valid")));
    }
    if e.flag("out_age") {
        l.line(format!(
            "  {} = ok ? (int32_t)((uint32_t)(millis() - {id}_ntp.lastSyncMillis()) / 1000) : -1;",
            e.output("age")
        ));
    }
    l.line("  if (ok) {")
        .line(format!("    uint32_t utc = {id}_ntp.unixTime();"));
    if e.flag("out_unix") {
        l.line(format!("    {} = (int32_t)utc;", e.output("unix")));
    }
    if parts {
        l.line(format!(
            "    time_t local = (time_t)((int64_t)utc + (long)({} * 3600) + {});",
            e.param("zone"),
            if e.flag("dst") { 3600 } else { 0 }
        ))
        .line("    struct tm lt;")
        .line("    gmtime_r(&local, &lt);");
    }
    for (key, port, expr) in [
        ("out_hour", "hour", "lt.tm_hour"),
        ("out_minute", "minute", "lt.tm_min"),
        ("out_second", "second", "lt.tm_sec"),
        ("out_day", "day", "lt.tm_mday"),
        ("out_month", "month", "lt.tm_mon + 1"),
        ("out_year", "year", "lt.tm_year + 1900"),
        ("out_weekday", "weekday", "lt.tm_wday == 0 ? 7 : lt.tm_wday"),
    ] {
        if e.flag(key) {
            l.line(format!("    {} = {expr};", e.output(port)));
        }
    }
    if e.flag("out_text") {
        l.line("    char buf[9];")
            .line("    snprintf(buf, sizeof(buf), \"%02d:%02d:%02d\", lt.tm_hour, lt.tm_min, lt.tm_sec);")
            .line(format!("    {} = buf;", e.output("text")));
    }
    l.line("  }").line("}");
    CppParts {
        global: g.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}
