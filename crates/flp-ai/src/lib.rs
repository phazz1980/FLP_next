//! One command set for the AI: the chat in the editor and the MCP server both
//! call [`call`] with tools described by [`tools`], so the logic is written once.
//!
//! A [`Context`] gives the commands access to the project: the MCP server works
//! on the project folder and saves after each edit, the editor works on the open
//! document with undo.

pub mod agent;
pub mod folder;
pub mod provider;

use std::fs;

use flp_codegen::{ArduinoCpp, Backend, Program};
use flp_core::library::{BLOCK_FILE, DOCS_FILE, ESP32_CPP_TEMPLATE};
use flp_core::pins::{pin_uses, Chip};
use flp_core::{
    check_project, BlockPos, Library, LibraryBlock, LocalValues, ParamValue, PortRef, Project,
    Severity, Var, VarType,
};
use serde_json::{json, Map, Value};

/// Access to the project for the commands.
pub trait Context {
    fn project(&self) -> &Project;
    fn library(&self) -> &Library;
    /// Folder of user blocks, where new block types are written (`user_library/`).
    fn library_dir(&self) -> std::path::PathBuf;
    /// Applies an edit (one undo step in the editor; saved to disk by the MCP server).
    fn edit(
        &mut self,
        edit: &mut dyn FnMut(&mut Project) -> Result<(), String>,
    ) -> Result<(), String>;
    /// Generates, builds and optionally uploads the firmware; returns a short report.
    fn build(&mut self, upload: bool, port: Option<String>, debug: bool) -> Result<String, String>;
    /// Latest values from the board: (name, value, forced).
    fn live_values(&mut self) -> Result<Vec<(String, String, bool)>, String>;
    /// Called after a new block type was written to the library.
    fn reload_library(&mut self) -> Result<(), String>;
}

/// A tool as offered to the model: name, description (Russian) and JSON schema.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn tools() -> Vec<Tool> {
    let id = json!({"type": "string", "description": "id блока"});
    let port = json!({"type": "string", "description": "порт в виде «блок.порт»"});
    vec![
        Tool {
            name: "get_schema",
            description: "Прочитать схему проекта: переменные, блоки с параметрами, связи, платы, занятые и свободные выводы (pins) и результат проверки.",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        Tool {
            name: "list_block_types",
            description: "Список типов блоков библиотеки с описаниями, входами, выходами и параметрами. Можно отфильтровать по словам.",
            input_schema: json!({"type": "object", "properties": {
                "filter": {"type": "string", "description": "слова для поиска в названии и описании"}
            }}),
        },
        Tool {
            name: "get_block_type",
            description: "Полное описание типа блока, включая документацию docs.md и шаблон кода.",
            input_schema: json!({"type": "object", "properties": {
                "type": {"type": "string"}
            }, "required": ["type"]}),
        },
        Tool {
            name: "add_block",
            description: "Добавить блок. Без id он придумывается сам; без координат блок ставится на свободное место платы.",
            input_schema: json!({"type": "object", "properties": {
                "type": {"type": "string", "description": "тип блока из библиотеки"},
                "id": {"type": "string", "description": "понятный id: буквы (можно русские), цифры, _; без пробелов и точек"},
                "params": {"type": "object", "description": "значения параметров"},
                "sheet": {"type": "integer", "description": "номер платы, по умолчанию 1"},
                "x": {"type": "number"}, "y": {"type": "number"}
            }, "required": ["type"]}),
        },
        Tool {
            name: "remove_block",
            description: "Удалить блок вместе с его связями.",
            input_schema: json!({"type": "object", "properties": {"id": id}, "required": ["id"]}),
        },
        Tool {
            name: "move_block",
            description: "Передвинуть блок на схеме (координаты и плата нужны только редактору).",
            input_schema: json!({"type": "object", "properties": {
                "id": id, "x": {"type": "number"}, "y": {"type": "number"},
                "sheet": {"type": "integer", "description": "номер платы, по умолчанию прежний"}
            }, "required": ["id", "x", "y"]}),
        },
        Tool {
            name: "rename_block",
            description: "Переименовать блок; связи обновятся.",
            input_schema: json!({"type": "object", "properties": {
                "id": id, "new_id": {"type": "string"}
            }, "required": ["id", "new_id"]}),
        },
        Tool {
            name: "set_params",
            description: "Задать параметры блока. null возвращает значение по умолчанию. Время — строкой: \"500ms\", \"5s\", \"2m\".",
            input_schema: json!({"type": "object", "properties": {
                "id": id, "params": {"type": "object"}
            }, "required": ["id", "params"]}),
        },
        Tool {
            name: "connect",
            description: "Соединить выход одного блока со входом другого. Связь в занятый вход заменяет старую.",
            input_schema: json!({"type": "object", "properties": {
                "from": port, "to": port
            }, "required": ["from", "to"]}),
        },
        Tool {
            name: "disconnect",
            description: "Убрать связь, ведущую во вход.",
            input_schema: json!({"type": "object", "properties": {"to": port}, "required": ["to"]}),
        },
        Tool {
            name: "set_input_const",
            description: "Поставить на вход константу вместо связи (связь во вход убирается) или убрать константу (value = null). Тип значения — как у входа: true/false, целое, число, строка.",
            input_schema: json!({"type": "object", "properties": {
                "input": port,
                "value": {"description": "значение константы; null убирает её"}
            }, "required": ["input", "value"]}),
        },
        Tool {
            name: "bind",
            description: "Привязать порт или параметр блока (текст, целое, число) к переменной проекта без блоков на листе: вход читает переменную, выход пишет в неё, параметр берётся из неё. variable = null снимает привязку. Так настраивают служебные блоки Wi-Fi/Ethernet.",
            input_schema: json!({"type": "object", "properties": {
                "block": {"type": "string", "description": "id блока"},
                "name": {"type": "string", "description": "имя порта или параметра"},
                "variable": {"description": "имя переменной; null снимает привязку"}
            }, "required": ["block", "name", "variable"]}),
        },
        Tool {
            name: "invert_port",
            description: "Инвертировать логический (bool) вход или выход блока, как кружок инверсии в FLProg. Инверсию выхода видят все его связи.",
            input_schema: json!({"type": "object", "properties": {
                "port": port,
                "output": {"type": "boolean", "description": "true — выход, false — вход"},
                "on": {"type": "boolean", "description": "false снимает инверсию; по умолчанию true"}
            }, "required": ["port", "output"]}),
        },
        Tool {
            name: "set_variable",
            description: "Создать или изменить переменную (тег). Переменную, которую никто не записывает, можно использовать как уставку.",
            input_schema: json!({"type": "object", "properties": {
                "name": {"type": "string"},
                "type": {"type": "string", "description": "тип Arduino: bool, char, byte, short, word, int, unsigned int, long, unsigned long, long long, unsigned long long, int8_t…uint64_t, float, double, String"},
                "init": {"description": "начальное значение"},
                "description": {"type": "string"},
                "retain": {"type": "boolean", "description": "сохранять во флеш (EEPROM): восстанавливается после перезагрузки"},
                "rename_to": {"type": "string"}
            }, "required": ["name", "type"]}),
        },
        Tool {
            name: "remove_variable",
            description: "Удалить переменную вместе с блоками чтения и записи.",
            input_schema: json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}),
        },
        Tool {
            name: "add_variable_block",
            description: "Поставить на плату блок чтения (var_get, выход out) или записи (var_set, вход in) переменной.",
            input_schema: json!({"type": "object", "properties": {
                "var": {"type": "string"},
                "write": {"type": "boolean"},
                "sheet": {"type": "integer"}
            }, "required": ["var", "write"]}),
        },
        Tool {
            name: "check",
            description: "Проверить схему: ошибки, предупреждения и порядок выполнения.",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        Tool {
            name: "generate",
            description: "Сгенерировать скетч Arduino C++ и вернуть его текст.",
            input_schema: json!({"type": "object", "properties": {
                "debug": {"type": "boolean", "description": "с модулем онлайн-отладки"}
            }}),
        },
        Tool {
            name: "build",
            description: "Собрать прошивку через arduino-cli. Может занять минуту.",
            input_schema: json!({"type": "object", "properties": {"debug": {"type": "boolean"}}}),
        },
        Tool {
            name: "upload",
            description: "Собрать и залить прошивку на плату.",
            input_schema: json!({"type": "object", "properties": {
                "port": {"type": "string", "description": "например COM3"},
                "debug": {"type": "boolean", "description": "с модулем онлайн-отладки"}
            }}),
        },
        Tool {
            name: "read_live_values",
            description: "Живые значения выходов блоков и переменных с платы (нужна отладочная прошивка).",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        Tool {
            name: "create_block_type",
            description: "Создать новый пользовательский тип блока (папка user_library/): block.toml, шаблон esp32.cpp.tmpl и docs.md. Встроенные блоки менять нельзя. Поле group задаёт раздел и папки библиотеки через «/», например «Насосы/Управление». Формат — как у существующих блоков (см. get_block_type).",
            input_schema: json!({"type": "object", "properties": {
                "type": {"type": "string"},
                "block_toml": {"type": "string"},
                "template": {"type": "string"},
                "docs": {"type": "string"}
            }, "required": ["type", "block_toml", "template"]}),
        },
    ]
}

/// Runs one tool. Errors are user-facing text the model can act on.
pub fn call(ctx: &mut dyn Context, name: &str, args: &Value) -> Result<Value, String> {
    let empty = Map::new();
    let args = args.as_object().unwrap_or(&empty);
    let s = |key: &str| -> Result<String, String> {
        args.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("нужен строковый аргумент «{key}»"))
    };
    let port = |key: &str| -> Result<PortRef, String> {
        s(key)?
            .parse()
            .map_err(|e: flp_core::PortRefError| e.to_string())
    };
    let lib = ctx.library().clone();
    match name {
        "get_schema" => {
            let p = ctx.project();
            Ok(json!({
                "project": p.config.project.name,
                "board": p.config.board.fqbn,
                "logic": p.logic,
                "sheets": p.layout.sheets,
                "placement": p.layout.blocks,
                "view": block_view(p, &lib),
                "pins": pins(p, &lib),
                "check": report(p, &lib),
            }))
        }
        "list_block_types" => {
            let filter = args
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            let list: Vec<Value> = lib
                .iter()
                .filter(|b| {
                    let i = &b.def.info;
                    filter.is_empty()
                        || filter.split_whitespace().any(|w| {
                            format!("{} {} {} {}", i.kind, i.name, i.group, i.description)
                                .to_lowercase()
                                .contains(w)
                        })
                })
                .map(block_summary)
                .collect();
            Ok(json!(list))
        }
        "get_block_type" => {
            let kind = s("type")?;
            let b = lib
                .get(&kind)
                .ok_or_else(|| format!("нет типа блока «{kind}»"))?;
            let mut v = block_summary(b);
            let docs = b.docs_path().and_then(|p| fs::read_to_string(p).ok());
            let template = b.template(ESP32_CPP_TEMPLATE).and_then(Result::ok);
            v["docs"] = json!(docs);
            v["template"] = json!(template);
            Ok(v)
        }
        "add_block" => {
            let kind = s("type")?;
            let wanted = args.get("id").and_then(Value::as_str).map(str::to_owned);
            let params = params_arg(args.get("params"))?;
            let sheet = args.get("sheet").and_then(Value::as_u64).unwrap_or(1) as u32;
            let xy = (
                args.get("x").and_then(Value::as_f64),
                args.get("y").and_then(Value::as_f64),
            );
            let mut new_id = String::new();
            ctx.edit(&mut |p| {
                let pos = match xy {
                    (Some(x), Some(y)) => BlockPos {
                        sheet,
                        x: x as f32,
                        y: y as f32,
                    },
                    _ => p.free_position(sheet),
                };
                let mut id = p.add_block(&lib, &kind, pos).map_err(|e| e.to_string())?;
                if let Some(w) = &wanted {
                    p.rename_block(&id, w).map_err(|e| e.to_string())?;
                    id = w.clone();
                }
                for (name, value) in &params {
                    p.set_param(&lib, &id, name, value.clone())
                        .map_err(|e| e.to_string())?;
                }
                new_id = id;
                Ok(())
            })?;
            Ok(json!({"id": new_id, "check": report(ctx.project(), &lib)}))
        }
        "remove_block" => {
            let id = s("id")?;
            ctx.edit(&mut |p| {
                p.remove_block(&id).map_err(|e| e.to_string())?;
                p.release_unused_services(&lib).map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "move_block" => {
            let id = s("id")?;
            let x = args
                .get("x")
                .and_then(Value::as_f64)
                .ok_or("нужен числовой x")? as f32;
            let y = args
                .get("y")
                .and_then(Value::as_f64)
                .ok_or("нужен числовой y")? as f32;
            let sheet = args.get("sheet").and_then(Value::as_u64).map(|n| n as u32);
            ctx.edit(&mut |p| {
                if !p.logic.blocks.iter().any(|b| b.id == id) {
                    return Err(format!("нет блока «{id}»"));
                }
                let old = p.layout.blocks.get(&id).map_or(1, |b| b.sheet);
                p.layout.blocks.insert(
                    id.clone(),
                    BlockPos {
                        sheet: sheet.unwrap_or(old),
                        x,
                        y,
                    },
                );
                Ok(())
            })?;
            Ok(ok(ctx, &lib))
        }
        "rename_block" => {
            let (id, new_id) = (s("id")?, s("new_id")?);
            ctx.edit(&mut |p| p.rename_block(&id, &new_id).map_err(|e| e.to_string()))?;
            Ok(ok(ctx, &lib))
        }
        "set_params" => {
            let id = s("id")?;
            let params = params_arg(args.get("params"))?;
            ctx.edit(&mut |p| {
                for (name, value) in &params {
                    p.set_param(&lib, &id, name, value.clone())
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            })?;
            Ok(ok(ctx, &lib))
        }
        "connect" => {
            let (from, to) = (port("from")?, port("to")?);
            ctx.edit(&mut |p| {
                p.connect(&lib, from.clone(), to.clone())
                    .map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "disconnect" => {
            let to = port("to")?;
            ctx.edit(&mut |p| {
                if p.disconnect(&to) {
                    Ok(())
                } else {
                    Err(format!("во вход {to} ничего не подано"))
                }
            })?;
            Ok(ok(ctx, &lib))
        }
        "set_input_const" => {
            let input = port("input")?;
            let value = match args.get("value") {
                None => return Err("нужен аргумент «value» (null убирает константу)".to_owned()),
                Some(Value::Null) => None,
                Some(v) => Some(
                    serde_json::from_value::<ParamValue>(v.clone())
                        .map_err(|e| format!("value: {e}"))?,
                ),
            };
            ctx.edit(&mut |p| {
                p.set_input_const(&lib, &input, value.clone())
                    .map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "bind" => {
            let text = |key: &str| {
                args.get(key)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("нужен аргумент «{key}»"))
            };
            let (block, name) = (text("block")?, text("name")?);
            let var = match args.get("variable") {
                Some(Value::Null) => None,
                Some(Value::String(v)) => Some(v.clone()),
                _ => return Err("нужен аргумент «variable» (null снимает привязку)".to_owned()),
            };
            ctx.edit(&mut |p| {
                p.bind(&block, &name, var.as_deref())
                    .map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "invert_port" => {
            let target = port("port")?;
            let output = args
                .get("output")
                .and_then(Value::as_bool)
                .ok_or("нужен аргумент «output»: true — выход, false — вход")?;
            let on = args.get("on").and_then(Value::as_bool).unwrap_or(true);
            ctx.edit(&mut |p| {
                p.set_port_inverted(&lib, &target, output, on)
                    .map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "set_variable" => {
            let name = s("name")?;
            let ty: VarType = serde_json::from_value(args.get("type").cloned().unwrap_or_default())
                .map_err(|_| {
                    "type: тип Arduino (bool, byte, int, long, float, double, String…)".to_owned()
                })?;
            let init = match args.get("init") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    serde_json::from_value::<ParamValue>(v.clone()).map_err(|e| e.to_string())?,
                ),
            };
            let var = Var {
                name: args
                    .get("rename_to")
                    .and_then(Value::as_str)
                    .unwrap_or(&name)
                    .to_owned(),
                ty,
                init,
                description: args
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                retain: args.get("retain").and_then(Value::as_bool).unwrap_or(false),
                service: false,
            };
            let retain = args.get("retain").and_then(Value::as_bool);
            ctx.edit(&mut |p| {
                let result = if let Some(old) = p.logic.var(&name) {
                    // Omitted `retain` keeps the current setting.
                    let var = Var {
                        retain: retain.unwrap_or(old.retain),
                        service: old.service,
                        ..var.clone()
                    };
                    p.update_var(&name, var)
                } else {
                    p.add_var(var.clone())
                };
                result.map_err(|e| e.to_string())
            })?;
            Ok(ok(ctx, &lib))
        }
        "remove_variable" => {
            let name = s("name")?;
            ctx.edit(&mut |p| p.remove_var(&name).map_err(|e| e.to_string()))?;
            Ok(ok(ctx, &lib))
        }
        "add_variable_block" => {
            let var = s("var")?;
            let write = args.get("write").and_then(Value::as_bool).unwrap_or(false);
            let sheet = args.get("sheet").and_then(Value::as_u64).unwrap_or(1) as u32;
            let mut new_id = String::new();
            ctx.edit(&mut |p| {
                let pos = p.free_position(sheet);
                new_id = p
                    .add_var_block(&var, write, pos)
                    .map_err(|e| e.to_string())?;
                Ok(())
            })?;
            Ok(json!({"id": new_id, "check": report(ctx.project(), &lib)}))
        }
        "check" => Ok(report(ctx.project(), &lib)),
        "generate" => {
            let debug = args.get("debug").and_then(Value::as_bool).unwrap_or(false);
            let store = LocalValues::open_default().map_err(|e| e.to_string())?;
            let project = ctx.project().with_local(&lib, &store);
            let program = Program::build(&project, &lib).map_err(|e| e.to_string())?;
            let code = ArduinoCpp { debug }
                .generate(&program)
                .map_err(|e| e.to_string())?;
            Ok(json!({"sketch": code}))
        }
        "build" => {
            let debug = args.get("debug").and_then(Value::as_bool).unwrap_or(false);
            Ok(json!({"result": ctx.build(false, None, debug)?}))
        }
        "upload" => {
            let debug = args.get("debug").and_then(Value::as_bool).unwrap_or(false);
            let port = args.get("port").and_then(Value::as_str).map(str::to_owned);
            Ok(json!({"result": ctx.build(true, port, debug)?}))
        }
        "read_live_values" => {
            let values: Vec<Value> = ctx
                .live_values()?
                .into_iter()
                .map(
                    |(name, value, forced)| json!({"name": name, "value": value, "forced": forced}),
                )
                .collect();
            Ok(json!(values))
        }
        "create_block_type" => {
            let kind = s("type")?;
            create_block_type(
                ctx,
                &kind,
                &s("block_toml")?,
                &s("template")?,
                args.get("docs").and_then(Value::as_str),
            )?;
            Ok(json!({"created": kind}))
        }
        other => Err(format!("неизвестная команда «{other}»")),
    }
}

fn ok(ctx: &dyn Context, lib: &Library) -> Value {
    json!({"ok": true, "check": report(ctx.project(), lib)})
}

/// Pins taken by blocks and, for a known chip, the free ones with their notes.
fn pins(project: &Project, lib: &Library) -> Value {
    let uses = pin_uses(&project.logic, lib);
    let used: Vec<Value> = uses
        .iter()
        .map(|u| json!({"gpio": u.gpio, "block": u.block, "param": u.param}))
        .collect();
    let Some(chip) = Chip::of_board(&project.config.board) else {
        return json!({"used": used});
    };
    let free: Vec<Value> = chip
        .pins()
        .into_iter()
        .filter(|p| !p.flash && !uses.iter().any(|u| u.gpio == p.gpio))
        .map(|p| json!({"gpio": p.gpio, "notes": p.notes()}))
        .collect();
    json!({"chip": chip.name(), "used": used, "free": free})
}

/// Check result in a compact form for the model.
pub fn report(project: &Project, lib: &Library) -> Value {
    let r = check_project(project, lib);
    let list = |sev: Severity| -> Vec<String> {
        r.diagnostics
            .iter()
            .filter(|d| d.severity == sev)
            .map(|d| d.message.clone())
            .collect()
    };
    let order: Vec<&str> = if r.has_errors() {
        Vec::new()
    } else {
        r.order
            .iter()
            .map(|&i| project.logic.blocks[i].id.as_str())
            .collect()
    };
    json!({"errors": list(Severity::Error), "warnings": list(Severity::Warning), "order": order})
}

/// Ports and parameters that are present on each block instance (conditions
/// `when` evaluated), for drawing the block: id → {inputs, outputs, params}.
fn block_view(p: &Project, lib: &Library) -> Value {
    let mut view = serde_json::Map::new();
    for b in &p.logic.blocks {
        let Some(def) = lib.get(&b.kind).map(|l| &l.def) else {
            continue;
        };
        let port = |d: &flp_core::PortDef| json!({"name": d.name, "label": d.label, "type": d.ty, "default": d.default});
        view.insert(
            b.id.clone(),
            json!({
                "inputs": def.active_inputs(&b.params).map(port).collect::<Vec<_>>(),
                "outputs": def.active_outputs(&b.params).map(port).collect::<Vec<_>>(),
                "params": def.active_params(&b.params).map(|d| &d.name).collect::<Vec<_>>(),
            }),
        );
    }
    Value::Object(view)
}

fn block_summary(b: &LibraryBlock) -> Value {
    let d = &b.def;
    let ports = |ps: &Vec<flp_core::PortDef>| -> Vec<Value> {
        ps.iter()
            .map(|p| json!({"name": p.name, "label": p.label, "type": p.ty, "default": p.default, "description": p.description}))
            .collect()
    };
    json!({
        "type": d.info.kind,
        "name": d.info.name,
        "group": d.info.group,
        "description": d.info.description,
        "inputs": ports(&d.inputs),
        "outputs": ports(&d.outputs),
        "params": d.params.iter().map(|p| json!({
            "name": p.name, "label": p.label, "kind": p.kind, "default": p.default,
            "min": p.min, "max": p.max, "description": p.description, "choices": p.choices
        })).collect::<Vec<_>>(),
    })
}

/// `{"pin": 4, "time": "5s", "b": null}` → parameter edits (`None` = default).
fn params_arg(v: Option<&Value>) -> Result<Vec<(String, Option<ParamValue>)>, String> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let obj = v.as_object().ok_or("params должен быть объектом")?;
    obj.iter()
        .map(|(k, v)| {
            let value = if v.is_null() {
                None
            } else {
                Some(
                    serde_json::from_value::<ParamValue>(v.clone())
                        .map_err(|e| format!("{k}: {e}"))?,
                )
            };
            Ok((k.clone(), value))
        })
        .collect()
}

fn create_block_type(
    ctx: &mut dyn Context,
    kind: &str,
    block_toml: &str,
    template: &str,
    docs: Option<&str>,
) -> Result<(), String> {
    let lib_dir = ctx.library_dir();
    if !flp_core::ident::is_valid_ident(kind) {
        return Err(format!("недопустимое имя типа «{kind}»"));
    }
    if ctx.library().is_builtin(kind) {
        return Err(format!(
            "тип «{kind}» занят встроенным блоком, встроенные блоки менять нельзя; выберите другое имя"
        ));
    }
    if ctx.library().is_user(kind) {
        return Err(format!("тип «{kind}» уже есть; выберите другое имя"));
    }
    // Validate in memory first, so a broken block never lands in the library.
    let def: flp_core::BlockDef =
        toml::from_str(block_toml).map_err(|e| format!("block.toml: {e}"))?;
    def.validate(kind).map_err(|e| format!("block.toml: {e}"))?;
    // The group becomes the section and folders of the user library
    // (`Насосы/Управление`); an unusable group puts the block in the root.
    let parts: Vec<&str> = def.info.group.split('/').map(str::trim).collect();
    let folder = if parts
        .iter()
        .all(|p| flp_core::library::is_valid_folder_name(p))
    {
        parts.iter().fold(lib_dir.clone(), |d, p| d.join(p))
    } else {
        lib_dir.clone()
    };
    let dir = folder.join(kind);
    if dir.exists() {
        return Err(format!("тип «{kind}» уже есть; выберите другое имя"));
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let write = |file: &str, text: &str| fs::write(dir.join(file), text).map_err(|e| e.to_string());
    write(BLOCK_FILE, block_toml)?;
    write(ESP32_CPP_TEMPLATE, template)?;
    if let Some(docs) = docs {
        write(DOCS_FILE, docs)?;
    }
    ctx.reload_library()
}
