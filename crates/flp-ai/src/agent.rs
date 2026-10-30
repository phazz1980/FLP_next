//! The chat loop: send the conversation, run the tools the model asks for, send
//! the results back, until the model answers in plain text.

use serde_json::{json, Value};

use crate::provider::Provider;
use crate::{call, tools, Context};

pub const SYSTEM_PROMPT: &str = "Ты — помощник в CrabLogic, программе визуального \
программирования микроконтроллеров ESP32 на языке FBD (как FLProg). Пользователь — \
разработчик устройств; отвечай по-русски, коротко и по делу.

Схема состоит из блоков с понятными id (pressure, pump) и связей «блок.порт» → «блок.порт». \
Переменные (теги) читаются блоками var_get и записываются блоками var_set; переменная, \
которую никто не записывает, служит уставкой. Время в параметрах пишется строкой: \"5s\", \"500ms\". \
Если выводов платы не хватает, цифровые входы и выходы ставят на расширитель PCF по I²C: \
set_expander создаёт его, place_on_expander ставит блок на вывод (занятые видны в expanders).

Как работать:
- перед правками прочитай схему (get_schema) и подходящие типы блоков (list_block_types: без фильтра — короткий список, \
порты видны при поиске по словам; не запрашивай описание блоков, которые не нужны);
- давай блокам осмысленные id и связывай всё, что добавил; после каждой правки смотри поле check \
и исправляй ошибки, пока их не останется;
- не собирай и не заливай прошивку, пока пользователь об этом не попросил;
- если подходящего блока нет, предложи создать новый тип (create_block_type) и опиши, что он будет делать;
- в конце кратко перечисли, что изменил в схеме.";

/// What happens during a turn, for the chat view.
#[derive(Debug, Clone)]
pub enum Event {
    Text(String),
    ToolCall {
        name: String,
        input: Value,
    },
    ToolResult {
        name: String,
        ok: bool,
        text: String,
    },
}

/// Upper bound on model calls per user message, against runaway loops.
const MAX_STEPS: usize = 40;

/// Tool results older than the last `KEEP_RECENT_RESULTS` are cut to
/// `OLD_RESULT_CHARS`: the whole history is re-sent on every step, and free
/// tiers cap the request size.
const KEEP_RECENT_RESULTS: usize = 4;
const OLD_RESULT_CHARS: usize = 400;

/// Shortens old tool results in place, keeping every message and `tool_use_id`.
fn trim_old_results(history: &mut [Value]) {
    let mut seen = 0;
    for msg in history.iter_mut().rev() {
        let Some(blocks) = msg["content"].as_array_mut() else {
            continue;
        };
        for block in blocks.iter_mut().rev() {
            if block["type"] != "tool_result" {
                continue;
            }
            seen += 1;
            if seen <= KEEP_RECENT_RESULTS {
                continue;
            }
            let Some(text) = block["content"].as_str() else {
                continue;
            };
            if text.chars().count() > OLD_RESULT_CHARS {
                let cut: String = text.chars().take(OLD_RESULT_CHARS).collect();
                block["content"] = json!(format!("{cut}… [результат сокращён]"));
            }
        }
    }
}

/// Runs one user turn. `history` holds the whole conversation in Messages API
/// shape and grows with every assistant reply and tool result.
pub fn run_turn(
    provider: &dyn Provider,
    ctx: &mut dyn Context,
    history: &mut Vec<Value>,
    user_text: &str,
    on_event: &mut dyn FnMut(Event),
) -> Result<(), String> {
    history.push(json!({"role": "user", "content": user_text}));
    let tools = tools();
    for _ in 0..MAX_STEPS {
        trim_old_results(history);
        let reply = provider.complete(SYSTEM_PROMPT, history, &tools)?;
        // Append the content as returned: thinking blocks must go back unchanged.
        history.push(json!({"role": "assistant", "content": reply.content}));
        let mut results = Vec::new();
        for block in &reply.content {
            match block["type"].as_str() {
                Some("text") => {
                    let text = block["text"].as_str().unwrap_or("").trim();
                    if !text.is_empty() {
                        on_event(Event::Text(text.to_owned()));
                    }
                }
                Some("tool_use") => {
                    let name = block["name"].as_str().unwrap_or("").to_owned();
                    let input = block["input"].clone();
                    on_event(Event::ToolCall {
                        name: name.clone(),
                        input: input.clone(),
                    });
                    let (text, ok) = match call(ctx, &name, &input) {
                        Ok(v) => (serde_json::to_string(&v).unwrap_or_default(), true),
                        Err(e) => (e, false),
                    };
                    on_event(Event::ToolResult {
                        name,
                        ok,
                        text: text.clone(),
                    });
                    results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": block["id"],
                        "content": text,
                        "is_error": !ok,
                    }));
                }
                _ => {}
            }
        }
        match reply.stop_reason.as_str() {
            "tool_use" => {
                // All results of one reply go back in a single message.
                history.push(json!({"role": "user", "content": results}));
            }
            "max_tokens" => {
                return Err("ответ оборвался по лимиту длины; попросите продолжить".into())
            }
            _ => return Ok(()),
        }
    }
    Err(format!("остановлено после {MAX_STEPS} шагов"))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::provider::Reply;
    use crate::Tool;

    /// Replays scripted replies and records what it was sent.
    struct Script(Mutex<Vec<Reply>>, Mutex<Vec<Vec<Value>>>);

    impl Provider for Script {
        fn name(&self) -> &str {
            "script"
        }
        fn complete(&self, _: &str, messages: &[Value], _: &[Tool]) -> Result<Reply, String> {
            self.1.lock().unwrap().push(messages.to_vec());
            Ok(self.0.lock().unwrap().remove(0))
        }
    }

    struct NoCtx(flp_core::Project, flp_core::Library);

    impl Context for NoCtx {
        fn project(&self) -> &flp_core::Project {
            &self.0
        }
        fn library(&self) -> &flp_core::Library {
            &self.1
        }
        fn library_dir(&self) -> std::path::PathBuf {
            Default::default()
        }
        fn edit(
            &mut self,
            _: &mut dyn FnMut(&mut flp_core::Project) -> Result<(), String>,
        ) -> Result<(), String> {
            Err("только чтение".into())
        }
        fn build(&mut self, _: bool, _: Option<String>, _: bool) -> Result<String, String> {
            Err("нет".into())
        }
        fn live_values(&mut self) -> Result<Vec<(String, String, bool)>, String> {
            Ok(Vec::new())
        }
        fn reload_library(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn old_tool_results_are_trimmed() {
        let long = "я".repeat(OLD_RESULT_CHARS * 3);
        let mut history = Vec::new();
        for i in 0..KEEP_RECENT_RESULTS + 2 {
            history.push(json!({"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": format!("t{i}"), "content": long, "is_error": false}
            ]}));
        }
        trim_old_results(&mut history);
        let len = |i: usize| history[i]["content"][0]["content"].as_str().unwrap().chars().count();
        assert!(len(0) < OLD_RESULT_CHARS + 40);
        assert!(len(1) < OLD_RESULT_CHARS + 40);
        assert_eq!(len(2), OLD_RESULT_CHARS * 3);
        assert_eq!(history[0]["content"][0]["tool_use_id"], "t0");
    }

    #[test]
    fn tool_loop_appends_results_and_keeps_blocks() {
        let script = Script(
            Mutex::new(vec![
                Reply {
                    content: vec![
                        json!({"type": "thinking", "thinking": "", "signature": "sig"}),
                        json!({"type": "tool_use", "id": "t1", "name": "check", "input": {}}),
                    ],
                    stop_reason: "tool_use".into(),
                },
                Reply {
                    content: vec![json!({"type": "text", "text": "Схема в порядке."})],
                    stop_reason: "end_turn".into(),
                },
            ]),
            Mutex::new(Vec::new()),
        );
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ctx = NoCtx(
            flp_core::Project::load(root.join("examples/button")).unwrap(),
            flp_core::Library::load(root.join("library")).unwrap().0,
        );
        let mut history = Vec::new();
        let mut events = Vec::new();
        run_turn(
            &script,
            &mut ctx,
            &mut history,
            "проверь схему",
            &mut |e| events.push(e),
        )
        .unwrap();

        // user, assistant (thinking + tool_use), user (tool_result), assistant (text)
        assert_eq!(history.len(), 4);
        assert_eq!(history[1]["content"][0]["signature"], "sig");
        assert_eq!(history[2]["content"][0]["tool_use_id"], "t1");
        assert_eq!(history[2]["content"][0]["is_error"], false);
        assert!(matches!(events.last(), Some(Event::Text(t)) if t == "Схема в порядке."));
        // The second call saw the tool result.
        assert_eq!(script.1.lock().unwrap()[1].len(), 3);
    }
}
