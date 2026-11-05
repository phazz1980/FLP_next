//! Firmware code of the network interfaces: Wi-Fi and Ethernet W5500, with the services
//! they host (web server).

use crate::cpp::{CppCtx, CppParts, Lines};

/// Which services the block hosts. A setting taken from a variable is switched while
/// running, so its code is always built in. The web server is always hosted.
struct Services {
    /// Address settings come from variables: re-applied when they change.
    dynamic: bool,
    /// Services are polled at most once per period.
    period: bool,
}

impl Services {
    fn of(e: &CppCtx, settings: &[&str]) -> Self {
        Self {
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

fn period_globals(l: &mut Lines, id: &str) {
    l.line("// Network services (MQTT, web) are polled at most once per period.")
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

    let mut su = Lines::default();
    if !s.dynamic {
        su.line(wifi_apply(e, client, ap));
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
    if e.is_bound("ssid") {
        // A client without a network name keeps scanning and makes the own access point unreachable.
        l.line(format!("  if (!String({}).length()) {{", e.param("ssid")))
            .line("    state &= ~1;")
            .line("  }");
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

    CppParts {
        global: g.finish(),
        setup: su.finish(),
        looped: l.finish(),
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
    if e.flag("out_address") {
        g.line("// Address shown on the address output; the text is rebuilt only when it changes.")
            .line(format!("uint32_t {id}_ip_raw = 0;"));
    }

    let mut su = Lines::default();
    // The module sits on the default SPI pins unless the block moves them.
    if [("sck", 18), ("miso", 19), ("mosi", 23)]
        .iter()
        .any(|(n, d)| e.int(n, *d) != *d)
    {
        su.line(format!(
            "flpEth.setSpiPins({}, {}, {});",
            e.param("sck"),
            e.param("miso"),
            e.param("mosi")
        ));
    }
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
    if e.flag("out_address") {
        l.line("{")
            .line("  uint32_t ip = flpEth.ethernetIsReady() ? (uint32_t)flpEth.localIP() : 0;")
            .line(format!("  if (ip != {id}_ip_raw) {{"))
            .line(format!("    {id}_ip_raw = ip;"))
            .line(format!(
                "    {} = ip ? IPAddress(ip).toString() : String();",
                e.output("address")
            ))
            .line("  }")
            .line("}");
    }

    CppParts {
        global: g.finish(),
        setup: su.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}
