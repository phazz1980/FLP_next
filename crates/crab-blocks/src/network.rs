//! Firmware code of the network service blocks: MQTT and NTP (the Wi-Fi and Ethernet
//! interfaces themselves are in `net.rs`).

use crate::cpp::{CppCtx, CppParts, Lines};

/// Interface object the service binds to: `net = "ethernet"` or Wi-Fi.
fn interface(e: &CppCtx) -> &'static str {
    if e.text("net") == "ethernet" {
        "crabEth"
    } else {
        "crabWifi"
    }
}

const MQTT_FUNCTIONS: &str = "\
void crab_mqtt_receive(const char *topic, const uint8_t *payload, size_t length, bool retained) {
  (void)retained;
  crabMqttTopic = topic;
  crabMqttPayload = \"\";
  crabMqttPayload.reserve(length);
  for (size_t i = 0; i < length; i++) {
    crabMqttPayload += (char)payload[i];
  }
  crabMqttSeq++;
}

// Topic against a filter with + (one level) and # (the rest).
bool crab_mqtt_match(const char *f, const char *t) {
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

/// A text setting as a C string for the client: a literal as it is, a value taken from
/// an input or a variable through a temporary `String`.
fn c_text(e: &CppCtx, name: &str) -> String {
    if e.is_bound(name) {
        format!("String({}).c_str()", e.param(name))
    } else {
        e.param(name).to_owned()
    }
}

pub(crate) fn mqtt(e: &CppCtx) -> CppParts {
    let id = &e.id;
    // Broker, port or credentials come from inputs or variables: re-applied when they change.
    let dynamic = ["broker", "port", "user", "password", "client_id"]
        .iter()
        .any(|n| e.is_bound(n));
    let mut g = Lines::default();
    if dynamic {
        g.line(crate::net::MIX.replace("@ID@", id))
            .line(format!("uint32_t {id}_sig = 0;"));
    }
    g.line("// Shared MQTT client: mqtt_publish / mqtt_subscribe blocks use it.")
        .line("FLProgNetworkMqttClient crabMqtt;")
        .line(format!("bool {id}_bound = false;"))
        .line("// The last received message, shared by all subscriptions.")
        .line("String crabMqttTopic;")
        .line("String crabMqttPayload;")
        .line("uint32_t crabMqttSeq = 0;")
        .line("");
    // Broker address and credentials, applied before every (re)connection.
    let mut apply = Lines::default();
    apply
        .line(format!("crabMqtt.setBroker({});", c_text(e, "broker")))
        .line(format!("crabMqtt.setPort({});", e.param("port")));
    if dynamic && e.is_bound("user") {
        apply.line(format!(
            "if (String({}).length()) crabMqtt.setCredentials({}, {}); else crabMqtt.clearCredentials();",
            e.param("user"),
            c_text(e, "user"),
            c_text(e, "password")
        ));
    } else if !e.text("user").is_empty() || e.is_bound("user") {
        apply.line(format!(
            "crabMqtt.setCredentials({}, {});",
            c_text(e, "user"),
            c_text(e, "password")
        ));
    }
    if e.is_bound("client_id") {
        apply
            .line("{")
            .line(format!("  String cid = String({});", e.param("client_id")))
            .line("  if (!cid.length()) {")
            .line("    char auto_id[24];")
            .line(
                "    snprintf(auto_id, sizeof(auto_id), \"crab-%08lx\", (unsigned long)ESP.getEfuseMac());",
            )
            .line("    cid = auto_id;")
            .line("  }")
            .line("  crabMqtt.setClientId(cid.c_str());")
            .line("}");
    }
    let apply = apply.finish();
    let mut l = Lines::default();
    l.line(format!("if (!{id}_bound) {{"))
        .line("  // Attach once; the client reconnects by itself after that.")
        .line(format!("  {id}_bound = crabMqtt.begin({});", interface(e)))
        .line(format!("  if ({id}_bound) {{"))
        .line(crate::net::indent(&apply, 4));
    if e.is_bound("client_id") {
        // Set with the other settings above.
    } else if e.text("client_id").is_empty() {
        l.line("    char cid[24];")
            .line(
                "    snprintf(cid, sizeof(cid), \"crab-%08lx\", (unsigned long)ESP.getEfuseMac());",
            )
            .line("    crabMqtt.setClientId(cid);");
    } else {
        l.line(format!(
            "    crabMqtt.setClientId({});",
            e.param("client_id")
        ));
    }
    l.line(format!(
        "    crabMqtt.setKeepAlive({});",
        e.param("keepalive")
    ))
    .line("    crabMqtt.setMessageCallback(crab_mqtt_receive);")
    .line("    crabMqtt.setAutoReconnect(true);");
    if dynamic {
        l.line(format!("    {id}_sig = {id}_signature();"));
    }
    l.line("    crabMqtt.connect();").line("  }");
    if dynamic {
        l.line(format!("}} else if ({id}_signature() != {id}_sig) {{"))
            .line("  // Settings written while running: reconnect with the new ones.")
            .line(format!("  {id}_sig = {id}_signature();"))
            .line("  crabMqtt.disconnect();")
            .line(crate::net::indent(&apply, 2))
            .line("  crabMqtt.connect();")
            .line("}");
    } else {
        l.line("}");
    }
    if e.flag("out_connected") && e.output.contains_key("connected") {
        l.line(format!("{} = crabMqtt.connected();", e.output("connected")));
    }
    if e.flag("out_error") && e.output.contains_key("error") {
        l.line(format!("{} = crabMqtt.error();", e.output("error")));
    }
    let mut functions = MQTT_FUNCTIONS.to_owned();
    if dynamic {
        let mut sig = Lines::default();
        sig.line(format!("static uint32_t {id}_signature() {{"))
            .line("  uint32_t cfg = 2166136261u;");
        for n in ["broker", "user", "password", "client_id"]
            .iter()
            .filter(|n| e.is_bound(n) || **n != "client_id")
        {
            sig.line(format!("  {id}_mix(cfg, {});", e.param(n)));
        }
        sig.line(format!(
            "  cfg = (cfg ^ (uint32_t)({})) * 16777619u;",
            e.param("port")
        ))
        .line("  return cfg;")
        .line("}");
        functions.push('\n');
        functions.push_str(&sig.finish());
    }
    CppParts {
        defines: format!(
            "#define FLPROG_MQTT_MAX_SUBSCRIPTIONS {}U",
            e.param("max_subscriptions")
        ),
        global: g.finish(),
        once_functions: functions,
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
        .line(format!("  if ({id}_pending && crabMqtt.connected()) {{"));
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
        "    if (crabMqtt.publish({}, text.c_str(), {})) {{",
        c_text(e, "topic"),
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
    let (topic, received) = (c_text(e, "topic"), e.output("received"));
    let mut g = Lines::default();
    g.line(format!("uint32_t {id}_seq = 0;"))
        .line(format!("bool {id}_subscribed = false;"));
    if e.is_bound("topic") {
        // The topic comes from an input: subscribe again when it changes (the client has no unsubscribe).
        g.line(format!("String {id}_topic;"));
    }
    let mut l = Lines::default();
    l.line("{");
    if e.is_bound("topic") {
        l.line(format!(
            "  if ({id}_topic != String({})) {{",
            e.param("topic")
        ))
        .line(format!("    {id}_topic = String({});", e.param("topic")))
        .line(format!("    {id}_subscribed = false;"))
        .line("  }");
    }
    l.line("  // The client keeps the subscription across reconnects; ask again only until it is accepted.")
        .line(format!(
            "  if (!{id}_subscribed && crabMqtt.connected()) {{"
        ))
        .line(format!(
            "    {id}_subscribed = crabMqtt.subscribe({topic});"
        ))
        .line("  }")
        .line(format!("  {received} = false;"))
        .line(format!("  if ({id}_seq != crabMqttSeq) {{"))
        .line(format!("    {id}_seq = crabMqttSeq;"))
        .line(format!(
            "    if (crab_mqtt_match({topic}, crabMqttTopic.c_str())) {{"
        ))
        .line(format!("      {received} = true;"));
    match e.text("kind") {
        "int" => l.line(format!(
            "      {} = crabMqttPayload.toInt();",
            e.output("out_i")
        )),
        "float" => l.line(format!(
            "      {} = crabMqttPayload.toFloat();",
            e.output("out_f")
        )),
        "bool" => l
            .line("      {")
            .line("        String p = crabMqttPayload;")
            .line("        p.trim();")
            .line("        p.toLowerCase();")
            .line(format!(
                "        {} = p == \"1\" || p == \"true\" || p == \"on\";",
                e.output("out_b")
            ))
            .line("      }"),
        _ => l.line(format!("      {} = crabMqttPayload;", e.output("out"))),
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
