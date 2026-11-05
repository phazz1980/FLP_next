//! Firmware code of the UART and Modbus blocks. The port settings come in `CppCtx::uart`.

use crate::cpp::{CppCtx, CppParts, Lines, UartCtx};

fn uart(e: &CppCtx) -> UartCtx {
    e.uart.clone().unwrap_or_default()
}

pub(crate) fn uart_config(e: &CppCtx) -> CppParts {
    let (id, u) = (&e.id, uart(e));
    let serial = &u.serial;
    let baud = format!("{} = (int32_t){serial}.baudRate();", e.output("baud"));
    if !e.flag("runtime") {
        return CppParts {
            looped: baud,
            ..Default::default()
        };
    }
    let (apply, new_speed, new_format) = (
        e.input("apply"),
        e.input("new_speed"),
        e.input("new_format"),
    );
    let mut f = Lines::default();
    f.line("// Arduino frame constant for a format like \"8E1\"; 0 if the text is not a format.")
        .line("uint32_t flp_uart_format(const String& text) {")
        .line("  String f = text;")
        .line("  f.trim();")
        .line("  f.toUpperCase();");
    for d in [5, 6, 7, 8] {
        for p in ["N", "E", "O"] {
            for s in [1, 2] {
                f.line(format!(
                    "  if (f == \"{d}{p}{s}\") return SERIAL_{d}{p}{s};"
                ));
            }
        }
    }
    f.line("  return 0;").line("}");
    let mut l = Lines::default();
    l.line("// Speed and format from the inputs take effect on a rising edge of apply:")
        .line("// 0 keeps the speed, an empty string keeps the format.")
        .line(format!("if ({apply} && !{id}_prev) {{"))
        .line(format!("  String format = {new_format};"))
        .line("  format.trim();")
        .line("  uint32_t config = format.length() > 0 ? flp_uart_format(format) : 0;")
        .line(format!(
            "  {} = format.length() == 0 || config != 0;",
            e.output("ok")
        ))
        .line(format!(
            "  unsigned long speed = {new_speed} > 0 ? (unsigned long){new_speed} : {serial}.baudRate();"
        ))
        .line("  if (config != 0) {")
        .line("    // A new frame format needs the port restarted.")
        .line(format!("    {serial}.end();"))
        .line(format!("    {serial}.setRxBufferSize({});", u.rx_buffer))
        .line(format!(
            "    {serial}.begin(speed, config, {}, {});",
            u.rx, u.tx
        ));
    if u.de >= 0 {
        l.line(format!("    {serial}.setPins(-1, -1, -1, {});", u.de))
            .line(format!(
                "    {serial}.setMode(UART_MODE_RS485_HALF_DUPLEX);"
            ));
    }
    l.line(format!("  }} else if ({new_speed} > 0) {{"))
        .line(format!("    {serial}.updateBaudRate(speed);"))
        .line("  }")
        .line("}")
        .line(format!("{id}_prev = {apply};"))
        .line(baud);
    CppParts {
        once_functions: f.finish(),
        global: format!("bool {id}_prev = false;"),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn uart_read(e: &CppCtx) -> CppParts {
    let (id, u) = (&e.id, uart(e));
    let input = &u.input;
    let available = format!("{} = {input}.available();", e.output("available"));
    if e.int("mode", 0) != 0 {
        let mut l = Lines::default();
        l.line(format!("{} = {input}.available() > 0;", e.output("new")))
            .line(format!("if ({}) {{", e.output("new")))
            .line(format!("  {} = {input}.read();", e.output("byte")))
            .line("}")
            .line(available);
        return CppParts {
            looped: l.finish(),
            ..Default::default()
        };
    }
    let (line, ready, max_len) = (e.output("line"), e.output("ready"), e.param("max_len"));
    let mut l = Lines::default();
    l.line(format!("{ready} = false;"))
        .line("// One line per cycle: the next one waits in the UART buffer.")
        .line(format!("while ({input}.available() > 0) {{"))
        .line(format!("  char c = (char){input}.read();"))
        .line(format!("  {id}_last = millis();"));
    match e.int("end", 0) {
        0 => l
            .line("  if (c == '\\r') {")
            .line("    continue;")
            .line("  }")
            .line("  bool end = c == '\\n';"),
        1 => l
            .line("  if (c == '\\n') {")
            .line("    continue;")
            .line("  }")
            .line("  bool end = c == '\\r';"),
        _ => l.line(format!("  bool end = c == {}[0];", e.param("end_char"))),
    };
    l.line("  if (!end) {")
        .line(format!("    {id}_buf += c;"))
        .line(format!("    end = {id}_buf.length() >= {max_len};"))
        .line("  }")
        .line("  if (end) {")
        .line(format!("    {line} = {id}_buf;"))
        .line(format!("    {id}_buf = \"\";"))
        .line(format!("    {ready} = true;"))
        .line("    break;")
        .line("  }")
        .line("}");
    if e.int("timeout", 0) > 0 {
        l.line("// No separator for a while: what came is a line too.")
            .line(format!(
                "if (!{ready} && {id}_buf.length() > 0 && millis() - {id}_last >= {}) {{",
                e.param_ul("timeout")
            ))
            .line(format!("  {line} = {id}_buf;"))
            .line(format!("  {id}_buf = \"\";"))
            .line(format!("  {ready} = true;"))
            .line("}");
    }
    l.line(available);
    let mut g = Lines::default();
    g.line(format!("String {id}_buf;"))
        .line(format!("unsigned long {id}_last = 0;"));
    CppParts {
        global: g.finish(),
        setup: format!("{id}_buf.reserve({max_len});"),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn uart_send(e: &CppCtx) -> CppParts {
    let (id, u) = (&e.id, uart(e));
    let serial = &u.serial;
    let (en, value, sent) = (e.input("en"), e.input("value"), e.output("sent"));
    let t = e.input_type("value");
    let trigger = e.int("trigger", 0);
    let send_as = e.int("send_as", 0);
    let mut g = Lines::default();
    if trigger == 0 {
        g.line(format!("bool {id}_prev = false;"));
    } else if trigger == 2 {
        let ct = match t {
            "bool" => "bool",
            "int" => "int32_t",
            "float" => "float",
            _ => "String",
        };
        g.line(format!("{ct} {id}_last;"))
            .line(format!("bool {id}_started = false;"));
    }
    let mut l = Lines::default();
    l.line("{");
    match trigger {
        0 => l
            .line(format!("  bool fire = {en} && !{id}_prev;"))
            .line(format!("  {id}_prev = {en};")),
        1 => l.line(format!("  bool fire = {en};")),
        _ => {
            l.line("  // On change; the first value after start goes out too.");
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
            l.line(format!("  bool fire = {en} && changed;"))
        }
    };
    l.line(format!("  {sent} = false;")).line("  if (fire) {");
    if send_as == 0 || t == "string" {
        match t {
            "string" => l.line(format!("    const String& text = {value};")),
            "float" => l.line(format!(
                "    String text = String({value}, {});",
                e.param("decimals")
            )),
            "bool" => l.line(format!("    String text = {value} ? \"1\" : \"0\";")),
            _ => l.line(format!("    String text = String({value});")),
        };
        let ending = if send_as == 1 || e.int("ending", 2) == 0 {
            ""
        } else if e.int("ending", 2) == 1 {
            "\\n"
        } else {
            "\\r\\n"
        };
        l.line(format!("    const char* ending = \"{ending}\";"))
            .line("    // A busy port would stall the loop: drop the message if the transmit buffer")
            .line("    // is (nearly) full. A long one may still wait while the hardware FIFO drains.")
            .line("    size_t room = text.length() + strlen(ending);")
            .line(format!(
                "    if ((size_t){serial}.availableForWrite() >= (room < 64 ? room : 64)) {{"
            ))
            .line(format!("      {serial}.print(text);"))
            .line(format!("      {serial}.print(ending);"))
            .line(format!("      {sent} = true;"))
            .line("    }");
    } else {
        match (t, e.int("int_size", 2)) {
            ("bool", _) => l.line(format!("    uint8_t data = {value} ? 1 : 0;")),
            ("float", _) => l.line(format!("    float data = {value};")),
            (_, 0) => l.line(format!(
                "    uint8_t data = (uint8_t)constrain({value}, 0L, 255L);"
            )),
            (_, 1) => l.line(format!(
                "    int16_t data = (int16_t)constrain({value}, -32768L, 32767L);"
            )),
            _ => l.line(format!("    int32_t data = {value};")),
        };
        l.line(format!(
            "    if ((size_t){serial}.availableForWrite() >= sizeof(data)) {{"
        ))
        .line(format!(
            "      {serial}.write((const uint8_t*)&data, sizeof(data));"
        ))
        .line(format!("      {sent} = true;"))
        .line("    }");
    }
    if trigger == 2 {
        l.line(format!("    if ({sent}) {{"))
            .line(format!("      {id}_last = {value};"))
            .line(format!("      {id}_started = true;"))
            .line("    }");
    }
    l.line("  }").line("}");
    CppParts {
        global: g.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

/// Shared UART access of the RTU blocks, one copy per sketch.
const UART_EXECUTOR: &str = "\
// Hardware UART access shared by Modbus RTU blocks; the guard keeps one copy even
// if the slave and the master ever stop sharing the same text of this section.
#ifndef FLP_UART_EXECUTOR
#define FLP_UART_EXECUTOR
FlprogUartExecutor flpUart;
#endif";

fn table(name: &str) -> &'static str {
    match name {
        "input" => "FLPROG_INPUT_REGISTR",
        "coil" => "FLPROG_COIL",
        "discrete" => "FLPROG_DISCRETE_INPUT",
        _ => "FLPROG_HOLDING_REGISTR",
    }
}

/// UART settings of an RTU object.
fn rtu_setup(l: &mut Lines, obj: &str, u: &UartCtx) {
    l.line(format!("// UART{} from the board settings", u.number))
        .line(format!("{obj}.setSpeedUart({});", u.speed))
        .line(format!("{obj}.setParityUart({});", u.parity));
    if u.rx >= 0 {
        l.line(format!("{obj}.setPinRxUart({});", u.rx));
    }
    if u.tx >= 0 {
        l.line(format!("{obj}.setPinTxUart({});", u.tx));
    }
    if u.de >= 0 {
        l.line(format!("{obj}.setPinPeDe({});", u.de));
    }
}

/// `configDataTable` of the tables with a non-zero size; `addr` — slave address for a master.
fn tables(l: &mut Lines, e: &CppCtx, obj: &str, addr: Option<&str>) {
    for (key, name) in [
        ("holding", "FLPROG_HOLDING_REGISTR"),
        ("input", "FLPROG_INPUT_REGISTR"),
        ("coils", "FLPROG_COIL"),
        ("discrete", "FLPROG_DISCRETE_INPUT"),
    ] {
        if e.int(key, 0) > 0 {
            let lead = addr.map_or(String::new(), |a| format!("{a}, "));
            l.line(format!(
                "{obj}.configDataTable({lead}{name}, {});",
                e.param(key)
            ));
        }
    }
}

pub(crate) fn modbus_rtu_master(e: &CppCtx) -> CppParts {
    let u = uart(e);
    let obj = "flpMaster";
    let mut s = Lines::default();
    rtu_setup(&mut s, obj, &u);
    for n in 1..=e.int("count", 1) {
        let addr = e.int(&format!("slave{n}"), 0).to_string();
        s.line(format!("{obj}.setSlaveAddress({}, {addr});", n - 1))
            .line(format!(
                "{obj}.setPollingPeriod({addr}, {}UL);",
                e.param("period")
            ))
            .line(format!(
                "{obj}.setTimeOutTime({addr}, {}UL);",
                e.param("timeout")
            ));
        tables(&mut s, e, obj, Some(&addr));
    }
    s.line(format!("{obj}.begin();"));
    CppParts {
        once_global: UART_EXECUTOR.to_owned(),
        global: format!(
            "// Shared master: modbus_master_read / modbus_master_write blocks access its slaves.\nModbusMasterRTU {obj}({}, {}, &flpUart);",
            e.param("uart"),
            e.param("count")
        ),
        setup: s.finish(),
        looped: format!("{obj}.pool();"),
        ..Default::default()
    }
}

/// `IPAddress(a, b, c, d)` for a dotted address; a text that is not one gives 0.0.0.0.
fn ip_literal(text: &str) -> String {
    let parts: Vec<u8> = text
        .split('.')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    match parts[..] {
        [a, b, c, d] => format!("IPAddress({a}, {b}, {c}, {d})"),
        _ => "IPAddress(0, 0, 0, 0)".to_owned(),
    }
}

pub(crate) fn modbus_tcp_master(e: &CppCtx) -> CppParts {
    let obj = "flpMasterTcp";
    let net = if e.text("net") == "ethernet" {
        "flpEth"
    } else {
        "flpWifi"
    };
    let mut s = Lines::default();
    s.line(format!(
        "{obj}.setServerIpAdress(0, {});",
        ip_literal(e.text("ip"))
    ))
    .line(format!("{obj}.setServerPort(0, {});", e.param("port")))
    .line(format!(
        "{obj}.setServerSlavesSize(0, {});",
        e.param("count")
    ));
    for n in 1..=e.int("count", 1) {
        let addr = e.int(&format!("slave{n}"), 0).to_string();
        s.line(format!("{obj}.setSlaveAddress(0, {}, {addr});", n - 1))
            .line(format!(
                "{obj}.setPollingPeriod(0, {addr}, {}UL);",
                e.param("period")
            ))
            .line(format!(
                "{obj}.setTimeOutTime(0, {addr}, {}UL);",
                e.param("timeout")
            ));
        tables(&mut s, e, obj, Some(&format!("0, {addr}")));
    }
    CppParts {
        global: format!(
            "// Shared master: modbus_master_read / modbus_master_write blocks (master = tcp) access its slaves.\nModbusMasterTCP {obj}(&{net}, 1);"
        ),
        setup: s.finish(),
        looped: format!("{obj}.pool();"),
        ..Default::default()
    }
}

pub(crate) fn modbus_rtu_slave(e: &CppCtx) -> CppParts {
    let u = uart(e);
    let obj = "flpModbusRtu";
    let mut s = Lines::default();
    rtu_setup(&mut s, obj, &u);
    s.line(format!("{obj}.setSlaveAddress({});", e.param("address")));
    tables(&mut s, e, obj, None);
    s.line(format!("{obj}.begin();"));
    let mut l = Lines::default();
    l.line(format!("{obj}.pool();"))
        .line(format!("{} = {obj}.getLastError();", e.output("error")));
    CppParts {
        once_global: UART_EXECUTOR.to_owned(),
        global: format!(
            "// Shared slave: modbus_read / modbus_write blocks with slave = \"rtu\" access its tables.\nModbusSlaveRTU {obj}({}, &flpUart);",
            e.param("uart")
        ),
        setup: s.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

pub(crate) fn modbus_tcp_slave(e: &CppCtx) -> CppParts {
    let obj = "flpModbus";
    let net = if e.text("net") == "ethernet" {
        "flpEth"
    } else {
        "flpWifi"
    };
    let mut s = Lines::default();
    s.line(format!("{obj}.setSlaveAddress({});", e.param("address")))
        .line(format!("{obj}.setTcpPort({});", e.param("port")));
    tables(&mut s, e, obj, None);
    let mut l = Lines::default();
    l.line(format!("{obj}.pool();"))
        .line(format!("{} = {obj}.getLastError();", e.output("error")));
    CppParts {
        global: format!(
            "// Shared slave: modbus_read / modbus_write blocks access its tables.\nModbusSlaveTCP {obj}(&{net});"
        ),
        setup: s.finish(),
        looped: l.finish(),
        ..Default::default()
    }
}

fn slave_object(e: &CppCtx) -> &'static str {
    if e.text("slave") == "rtu" {
        "flpModbusRtu"
    } else {
        "flpModbus"
    }
}

fn looped(code: String) -> CppParts {
    CppParts {
        looped: code,
        ..Default::default()
    }
}

pub(crate) fn modbus_read(e: &CppCtx) -> CppParts {
    let (obj, t, reg) = (slave_object(e), table(e.text("table")), e.param("reg"));
    looped(match e.text("kind") {
        "float" => format!("{} = {obj}.readFloat({t}, {reg});", e.output("out_f")),
        "bool" => format!("{} = {obj}.readBool({t}, {reg});", e.output("out_b")),
        _ => format!(
            "{} = (int16_t){obj}.readInteger({t}, {reg});",
            e.output("out")
        ),
    })
}

pub(crate) fn modbus_write(e: &CppCtx) -> CppParts {
    let (obj, t, reg) = (slave_object(e), table(e.text("table")), e.param("reg"));
    looped(match e.text("kind") {
        "float" => format!("{obj}.saveFloat({}, {t}, {reg});", e.input("in_f")),
        "bool" => format!("{obj}.saveBool({}, {t}, {reg});", e.input("in_b")),
        _ => format!(
            "{obj}.saveInteger((uint16_t)({}), {t}, {reg});",
            e.input("in")
        ),
    })
}

/// The master object and the leading call argument: the TCP master has a server index first.
fn master_object(e: &CppCtx) -> (&'static str, &'static str) {
    if e.text("master") == "tcp" {
        ("flpMasterTcp", "0, ")
    } else {
        ("flpMaster", "")
    }
}

pub(crate) fn modbus_master_read(e: &CppCtx) -> CppParts {
    let (slave, t, reg) = (e.param("slave"), table(e.text("table")), e.param("reg"));
    let (obj, lead) = master_object(e);
    let mut l = Lines::default();
    l.line(match e.text("kind") {
        "float" => format!(
            "{} = {obj}.readFloat({lead}{slave}, {t}, {reg});",
            e.output("out_f")
        ),
        "bool" => format!(
            "{} = {obj}.readBool({lead}{slave}, {t}, {reg});",
            e.output("out_b")
        ),
        _ => format!(
            "{} = {obj}.readInteger({lead}{slave}, {t}, {reg});",
            e.output("out")
        ),
    })
    .line(format!(
        "{} = {obj}.slaveStatus({lead}{slave}, false);",
        e.output("ok")
    ));
    looped(l.finish())
}

pub(crate) fn modbus_master_write(e: &CppCtx) -> CppParts {
    let (slave, t, reg) = (e.param("slave"), table(e.text("table")), e.param("reg"));
    let (obj, lead) = master_object(e);
    looped(match e.text("kind") {
        "float" => format!(
            "{obj}.saveFloat({lead}{slave}, {}, {t}, {reg});",
            e.input("in_f")
        ),
        "bool" => format!(
            "{obj}.saveBool({lead}{slave}, {}, {t}, {reg});",
            e.input("in_b")
        ),
        _ => format!(
            "{obj}.saveInteger({lead}{slave}, (int16_t)({}), {t}, {reg});",
            e.input("in")
        ),
    })
}
