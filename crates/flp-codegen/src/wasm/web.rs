//! Web interface in modules (`specs/wasm-io/web.md`): the master serves the page (`FLP_WEB` in
//! `master.ino`); a module hands it the layout JSON and a table of its widgets once, then writes
//! the values to show and reads what the page set, through ops 17..22 of the `net` import.
//! The widgets of a sheet are numbered in execution order; the page reads them by that number.

use super::text::A;
use super::*;

/// Widgets the master holds, the layout it can keep (`g_webItems`, `g_webLayout`).
const MAX_WIDGETS: usize = 32;
const MAX_LAYOUT: usize = 8191;

/// What a widget looks like to the page and to the master's table.
struct Widget {
    /// Kind of the page (the block kind without `web_`; the text input is `textin`).
    kind: &'static str,
    unit: String,
    lo: f64,
    hi: f64,
    step: f64,
    dec: u8,
    /// Master table: 0 number, 1 text shown, 2 text input, 3 secret text input.
    table_kind: u8,
    /// The page may change the value.
    rw: bool,
    init: f64,
    /// Chart only: points kept by the master.
    points: u8,
}

fn json_text(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 32 => {}
            c => out.push(c),
        }
    }
}

impl Emitter<'_, '_> {
    /// A numeric parameter of a block that has it, 0 otherwise.
    fn web_num(&self, inst: &Instance, name: &str) -> f64 {
        if inst.lib.def.params.iter().any(|p| p.name == name) {
            self.named_param_f(inst, name)
        } else {
            0.0
        }
    }

    /// The widgets of the sheet in execution order.
    /// The page host block is switched on (the `enabled` parameter).
    pub(super) fn web_enabled(&self, ui: &Instance) -> bool {
        self.web_num(ui, "enabled") != 0.0
    }

    fn web_widgets(&self) -> Vec<&Instance<'_>> {
        let program = self.program;
        self.module
            .blocks
            .iter()
            .map(|&bi| &program.blocks[bi])
            .filter(|i| {
                i.block.kind.starts_with("web_")
                    && i.block.kind != "web_ui"
                    && i.block.tag.is_none()
            })
            .collect()
    }

    fn web_widget(&self, inst: &Instance) -> Result<Widget, WasmError> {
        let kind = inst.block.kind.as_str();
        let mut w = Widget {
            kind: "",
            unit: String::new(),
            lo: 0.0,
            hi: 0.0,
            step: 0.0,
            dec: 0,
            table_kind: 0,
            rw: false,
            init: 0.0,
            points: 0,
        };
        let unit = self.text_param(inst, "unit");
        match kind {
            "web_button" => {
                w.kind = "button";
                w.rw = true;
            }
            "web_switch" => {
                w.kind = "switch";
                w.rw = true;
                w.init = self.web_num(inst, "initial");
            }
            "web_label" => w.kind = "label",
            "web_clock" => {
                w.kind = "clock";
                w.lo = self.web_num(inst, "seconds");
                w.hi = self.web_num(inst, "date");
            }
            "web_led" => w.kind = "led",
            "web_text" => {
                w.kind = "text";
                w.table_kind = 1;
            }
            "web_icon" => {
                w.kind = "icon";
                w.lo = self.web_num(inst, "icon");
            }
            "web_image" => {
                w.kind = "image";
                w.unit = self.text_param(inst, "url");
                w.lo = self.web_num(inst, "fit");
            }
            "web_shape" => {
                w.kind = "shape";
                w.lo = self.web_num(inst, "shape");
                w.hi = self.web_num(inst, "thickness");
            }
            "web_gauge" => {
                w.kind = "gauge";
                w.unit = unit;
                w.lo = self.web_num(inst, "min");
                w.hi = self.web_num(inst, "max");
                w.dec = 1;
            }
            "web_tank" => {
                w.kind = "tank";
                w.unit = unit;
                w.lo = self.web_num(inst, "min");
                w.hi = self.web_num(inst, "max");
                w.dec = self.web_num(inst, "decimals").clamp(0.0, 9.0) as u8;
            }
            "web_chart" => {
                w.kind = "chart";
                w.unit = unit;
                w.lo = self.web_num(inst, "min");
                w.hi = self.web_num(inst, "max");
                w.step = self.web_num(inst, "period").max(1.0);
                w.init = w.step;
                w.dec = 1;
                w.table_kind = 4;
                w.points = self.web_num(inst, "points").clamp(10.0, 120.0) as u8;
            }
            "web_value" => {
                w.kind = "value";
                w.unit = unit;
                w.dec = self.web_num(inst, "decimals").clamp(0.0, 9.0) as u8;
            }
            "web_slider" | "web_input" => {
                w.kind = if kind == "web_slider" {
                    "slider"
                } else {
                    "input"
                };
                w.unit = unit;
                w.rw = true;
                w.init = self.web_num(inst, "initial");
                w.lo = self.web_num(inst, "min");
                w.hi = self.web_num(inst, "max");
                w.step = self.web_num(inst, "step");
                w.dec = if self.web_num(inst, "integer") != 0.0 {
                    0
                } else {
                    3
                };
            }
            "web_net" => {
                // The form is a text widget for the page; the master keeps the settings.
                w.kind = "net";
                w.table_kind = 1;
            }
            "web_text_input" => {
                w.kind = "textin";
                w.rw = true;
                let secret = self.web_num(inst, "secret") != 0.0;
                w.lo = secret as u8 as f64;
                w.table_kind = 2 + secret as u8;
            }
            _ => {
                return Err(self.unsupported(inst, "этот виджет пока не работает в модулях"));
            }
        }
        Ok(w)
    }

    /// Network interfaces the page is served on: the network blocks of the project
    /// (1 Wi-Fi, 2 Ethernet).
    fn web_mask(&self) -> i32 {
        let mut mask = 0;
        for inst in &self.program.blocks {
            let bit = match inst.block.kind.as_str() {
                "wifi" => 1,
                "ethernet" => 2,
                _ => continue,
            };
            mask |= bit;
        }
        mask
    }

    /// Hands the master the layout, the table, the interfaces and the password (once, before the
    /// first block of the step runs).
    pub(super) fn web_setup(&mut self, ui: &Instance) -> Result<(), WasmError> {
        let ids: Vec<(String, String)> = self
            .web_widgets()
            .iter()
            .map(|i| (i.block.id.clone(), i.block.kind.clone()))
            .collect();
        if ids.len() > MAX_WIDGETS {
            return Err(self.unsupported(
                ui,
                &format!("на листе больше {MAX_WIDGETS} виджетов — столько держит мастер"),
            ));
        }
        let mut mask = self.web_mask();
        if mask == 0 {
            return Err(self.unsupported(
                ui,
                "нужен блок Wi-Fi или Ethernet с включённой веб-страницей",
            ));
        }
        let system = self.web_num(ui, "system") != 0.0;
        if system {
            // Bit 4 of the interfaces: the master serves /ui/info.
            mask |= 4;
        }
        let mut layout = String::from("{\"t\":\"");
        json_text(&mut layout, &self.text_param(ui, "title"));
        layout.push_str("\",\"p\":\"");
        json_text(&mut layout, &self.text_param(ui, "pages"));
        layout.push_str("\",\"bg\":\"");
        json_text(&mut layout, &self.text_param(ui, "bg"));
        layout.push_str(&format!("\",\"y\":{},\"i\":[", system as u8));
        let mut table = Vec::new();
        let mut texts: Vec<(usize, String)> = Vec::new();
        let widgets: Vec<&Instance> = self.web_widgets();
        for (n, inst) in widgets.iter().enumerate() {
            let w = self.web_widget(inst)?;
            if n > 0 {
                layout.push(',');
            }
            layout.push_str(&format!("{{\"k\":\"{}\",\"l\":\"", w.kind));
            json_text(&mut layout, &self.text_param(inst, "caption"));
            layout.push_str("\",\"u\":\"");
            json_text(&mut layout, &w.unit);
            layout.push_str(&format!(
                "\",\"c\":{},\"r\":{},\"w\":{},\"h\":{},\"g\":{},\"d\":{},\"a\":{:.3},\"b\":{:.3},\"s\":{:.3},\"o\":\"",
                self.web_num(inst, "col") as i32,
                self.web_num(inst, "row") as i32,
                self.web_num(inst, "w") as i32,
                self.web_num(inst, "h") as i32,
                self.web_num(inst, "page") as i32,
                w.dec,
                w.lo,
                w.hi,
                w.step,
            ));
            json_text(&mut layout, &self.text_param(inst, "color"));
            layout.push_str("\",\"q\":\"");
            json_text(&mut layout, &self.text_param(inst, "back"));
            layout.push_str("\"}");
            table.extend_from_slice(&[w.table_kind, w.rw as u8, w.dec, w.points]);
            for x in [w.lo, w.hi, w.init] {
                table.extend_from_slice(&(x as f32).to_le_bytes());
            }
            if w.table_kind >= 2 {
                let initial = self.text_param(inst, "initial");
                if !initial.is_empty() {
                    texts.push((n, initial));
                }
            }
        }
        layout.push_str("]}");
        if layout.len() > MAX_LAYOUT {
            return Err(self.unsupported(
                ui,
                &format!(
                    "раскладка страницы {} байт — больше {MAX_LAYOUT}",
                    layout.len()
                ),
            ));
        }
        let password = self.text_param(ui, "password");

        let nets: Vec<usize> = widgets
            .iter()
            .enumerate()
            .filter(|(_, i)| i.block.kind == "web_net")
            .map(|(n, _)| n)
            .collect();
        if nets.len() > 1 {
            return Err(self.unsupported(
                widgets[nets[1]],
                "форма сети на странице может быть только одна",
            ));
        }
        let mut net_init: Option<(String, i32)> = None;
        if let Some(&n) = nets.first() {
            let inst = widgets[n];
            let text = |name: &str| self.text_param(inst, name).replace('\n', " ");
            let lines = [
                text("init_ssid"),
                text("init_password"),
                ((self.web_num(inst, "init_dhcp") != 0.0) as u8).to_string(),
                text("init_ip"),
                text("init_gateway"),
                text("init_subnet"),
                text("init_ap_ssid"),
                text("init_ap_password"),
                (self.web_num(inst, "init_mode") as i32).to_string(),
                (self.web_num(inst, "init_ap_delay") as i32).to_string(),
            ];
            net_init = Some((lines.join("\n"), n as i32));
        }
        let done = self.state_i();
        self.op(I::GlobalGet(done));
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        let s = self.const_str(layout.as_bytes());
        self.net_call4(17, [A::C(1), s.ptr(), s.len(), A::C(0)]);
        self.op(I::Drop);
        if !ids.is_empty() {
            let t = self.const_str(&table);
            self.net_call4(17, [A::C(2), t.ptr(), A::C(ids.len() as i32), A::C(0)]);
            self.op(I::Drop);
        }
        if let Some((payload, n)) = net_init {
            let t = self.const_str(payload.as_bytes());
            self.net_call4(17, [A::C(5), t.ptr(), t.len(), A::C(n)]);
            self.op(I::Drop);
        }
        self.net_call4(17, [A::C(3), A::C(mask), A::C(0), A::C(0)]);
        self.op(I::Drop);
        if !password.is_empty() {
            let s = self.const_str(password.as_bytes());
            self.net_call4(17, [A::C(4), s.ptr(), s.len(), A::C(0)]);
            self.op(I::Drop);
        }
        for (n, text) in texts {
            let s = self.const_str(text.as_bytes());
            self.net_call4(20, [A::C(n as i32), s.ptr(), s.len(), A::C(0)]);
            self.op(I::Drop);
        }
        self.net_call4(17, [A::C(63), A::C(0), A::C(0), A::C(0)]);
        self.op(I::Drop);
        self.c_i(1);
        self.op(I::GlobalSet(done));
        self.op(I::End);
        Ok(())
    }

    pub(super) fn web_block(&mut self, inst: &Instance) -> Result<bool, WasmError> {
        let kind = inst.block.kind.as_str();
        if !kind.starts_with("web_") {
            return Ok(false);
        }
        if kind == "web_ui" {
            // Set up before the first block, see `step_body`.
            return Ok(true);
        }
        let program = self.program;
        let ui = self
            .module
            .blocks
            .iter()
            .map(|&bi| &program.blocks[bi])
            .find(|i| i.block.kind == "web_ui");
        // A switched-off page keeps its widgets in the project but builds nothing.
        if ui.is_some_and(|ui| !self.web_enabled(ui)) {
            return Ok(true);
        }
        if ui.is_none() {
            return Err(self.unsupported(inst, "нужен блок «Веб-интерфейс» на этом же листе"));
        }
        // An image of a widget on the sheet works with the widget itself.
        let widget = inst.block.tag.as_deref().unwrap_or(&inst.block.id);
        let idx = self
            .web_widgets()
            .iter()
            .position(|i| i.block.id == widget)
            .unwrap_or(0) as i32;
        // Fails for a widget the master cannot show yet.
        self.web_widget(inst)?;
        // A widget with nothing linked (its images on the sheets carry the links) shows
        // nothing of its own: it must not overwrite what an image wrote.
        if inst.inputs.is_empty()
            && matches!(
                kind,
                "web_led"
                    | "web_icon"
                    | "web_gauge"
                    | "web_tank"
                    | "web_value"
                    | "web_chart"
                    | "web_text"
            )
        {
            return Ok(true);
        }
        match kind {
            "web_led" | "web_icon" | "web_gauge" | "web_tank" | "web_value" | "web_chart" => {
                let x = self.tmp_f();
                self.input_as(inst, 0, Ty::F);
                self.op(I::LocalSet(x));
                self.net_call4(19, [A::C(idx), A::F(x), A::C(0), A::C(0)]);
                self.op(I::Drop);
            }
            "web_text" => {
                let s = self.str_in(inst, "in")?;
                self.net_call4(20, [A::C(idx), s.ptr(), s.len(), A::C(0)]);
                self.op(I::Drop);
            }
            "web_button" | "web_switch" => {
                self.net_call4(21, [A::C(idx), A::C(0), A::C(0), A::C(0)]);
                self.op(I::F32ReinterpretI32);
                self.c_f(0.0);
                self.op(I::F32Ne);
                self.set_named(inst, "out", Ty::I);
            }
            "web_slider" | "web_input" => {
                let x = self.tmp_f();
                self.net_call4(21, [A::C(idx), A::C(0), A::C(0), A::C(0)]);
                self.op(I::F32ReinterpretI32);
                self.op(I::LocalSet(x));
                if inst.outputs.contains_key("out") {
                    self.op(I::LocalGet(x));
                    self.set_named(inst, "out", Ty::F);
                }
                if inst.outputs.contains_key("out_i") {
                    self.op(I::LocalGet(x));
                    self.op(I::F32Nearest);
                    self.op(I::I32TruncSatF32S);
                    self.set_named(inst, "out_i", Ty::I);
                }
            }
            "web_net" => {
                // Fields of the form as text: ssid, password, ip, gateway, subnet.
                for (name, field) in [
                    ("ssid", 0),
                    ("password", 1),
                    ("ip", 3),
                    ("gateway", 4),
                    ("subnet", 5),
                ] {
                    if let Some(buf) = self.out_buf(inst, name) {
                        self.net_call4(23, [A::C(field), buf.ptr(), buf.cap(), A::C(0)]);
                        self.op(I::GlobalSet(buf.len));
                    }
                }
                for (name, field) in [("dhcp", 11), ("mode", 12), ("changed", 10)] {
                    if inst.outputs.contains_key(name) {
                        self.net_call4(23, [A::C(field), A::C(0), A::C(0), A::C(0)]);
                        self.set_named(inst, name, Ty::I);
                    }
                }
            }
            "web_text_input" => {
                // The page may write while `write` is on; without a link it always may.
                if inst.inputs.contains_key("write") {
                    let on = self.tmp_i();
                    self.flag(inst, 0);
                    self.op(I::LocalSet(on));
                    self.net_call4(24, [A::C(idx), A::L(on), A::C(0), A::C(0)]);
                    self.op(I::Drop);
                }
                if let Some(buf) = self.out_buf(inst, "out") {
                    self.net_call4(22, [A::C(idx), buf.ptr(), buf.cap(), A::C(0)]);
                    self.op(I::GlobalSet(buf.len));
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
