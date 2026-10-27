//! Provider for any OpenAI-compatible chat-completions endpoint (cheap routers,
//! OpenRouter, local servers). The history stays in Messages API shape; it is
//! converted to chat-completions messages here and the reply is converted back.

use std::time::Duration;

use serde_json::{json, Value};

use crate::provider::{Provider, Reply};
use crate::Tool;

pub struct OpenAiCompat {
    api_key: String,
    pub model: String,
    /// Up to and including the version segment, e.g. `https://host/v1`.
    base_url: String,
}

impl OpenAiCompat {
    /// Reads `FLP_AI_BASE_URL`, `FLP_AI_KEY` and `FLP_AI_MODEL`. Returns `None`
    /// when no base URL is set, i.e. the OpenAI-compatible provider is not wanted.
    pub fn from_env() -> Option<Result<Self, String>> {
        let base_url = non_empty("FLP_AI_BASE_URL")?;
        Some(Self::with_settings(
            base_url,
            non_empty("FLP_AI_KEY"),
            non_empty("FLP_AI_MODEL"),
        ))
    }

    fn with_settings(
        base_url: String,
        key: Option<String>,
        model: Option<String>,
    ) -> Result<Self, String> {
        let api_key = key.ok_or(
            "не задан ключ: установите переменную окружения FLP_AI_KEY и перезапустите программу",
        )?;
        let model = model.ok_or("не задана модель: установите переменную окружения FLP_AI_MODEL (например, имя модели из личного кабинета роутера)")?;
        Ok(Self {
            api_key,
            model,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub(crate) fn request_body(&self, system: &str, messages: &[Value], tools: &[Tool]) -> Value {
        let mut out = vec![json!({"role": "system", "content": system})];
        for m in messages {
            convert_message(m, &mut out);
        }
        let mut body = json!({
            "model": self.model,
            "messages": out,
            "max_tokens": 8000,
        });
        if !tools.is_empty() {
            body["tools"] = tools
                .iter()
                .map(|t| {
                    json!({"type": "function", "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }})
                })
                .collect();
        }
        body
    }
}

fn non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// One Messages API message becomes one or more chat-completions messages.
fn convert_message(m: &Value, out: &mut Vec<Value>) {
    let role = m["role"].as_str().unwrap_or("user");
    let Some(blocks) = m["content"].as_array() else {
        out.push(json!({"role": role, "content": m["content"]}));
        return;
    };
    let mut text = String::new();
    let mut calls = Vec::new();
    for b in blocks {
        match b["type"].as_str() {
            Some("text") => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(b["text"].as_str().unwrap_or(""));
            }
            Some("tool_use") => calls.push(json!({
                "id": b["id"],
                "type": "function",
                "function": {
                    "name": b["name"],
                    "arguments": serde_json::to_string(&b["input"]).unwrap_or_else(|_| "{}".into()),
                },
            })),
            Some("tool_result") => out.push(json!({
                "role": "tool",
                "tool_call_id": b["tool_use_id"],
                "content": b["content"].as_str().map_or_else(|| b["content"].to_string(), str::to_owned),
            })),
            _ => {}
        }
    }
    if role == "assistant" {
        let mut msg = json!({"role": "assistant", "content": text});
        if !calls.is_empty() {
            msg["tool_calls"] = Value::Array(calls);
            if text.is_empty() {
                msg["content"] = Value::Null;
            }
        }
        out.push(msg);
    } else if !text.is_empty() {
        out.push(json!({"role": role, "content": text}));
    }
}

pub(crate) fn parse_reply(body: &Value) -> Result<Reply, String> {
    let choice = &body["choices"][0];
    let message = &choice["message"];
    if !message.is_object() {
        let why = body["error"]["message"].as_str().unwrap_or("нет choices");
        return Err(format!("неожиданный ответ модели: {why}"));
    }
    let mut content = Vec::new();
    if let Some(t) = message["content"].as_str().filter(|t| !t.trim().is_empty()) {
        content.push(json!({"type": "text", "text": t}));
    }
    let mut has_calls = false;
    for (i, call) in message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        has_calls = true;
        let raw = call["function"]["arguments"].as_str().unwrap_or("");
        let input = if raw.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str::<Value>(raw)
                .map_err(|e| format!("модель прислала некорректные аргументы вызова: {e}"))?
        };
        content.push(json!({
            "type": "tool_use",
            "id": call["id"].as_str().map_or_else(|| format!("call_{i}"), str::to_owned),
            "name": call["function"]["name"],
            "input": input,
        }));
    }
    let stop_reason = match choice["finish_reason"].as_str() {
        _ if has_calls => "tool_use",
        Some("length") => "max_tokens",
        Some("content_filter") => return Err("модель отказалась отвечать на этот запрос".into()),
        _ => "end_turn",
    };
    Ok(Reply {
        content,
        stop_reason: stop_reason.to_owned(),
    })
}

impl Provider for OpenAiCompat {
    fn name(&self) -> &str {
        &self.model
    }

    fn complete(&self, system: &str, messages: &[Value], tools: &[Tool]) -> Result<Reply, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(600))
            .build();
        let response = agent
            .post(&format!("{}/chat/completions", self.base_url))
            .set("authorization", &format!("Bearer {}", self.api_key))
            .set("content-type", "application/json")
            .send_json(self.request_body(system, messages, tools));
        let body: Value = match response {
            Ok(r) => r
                .into_json()
                .map_err(|e| format!("ответ модели не прочитан: {e}"))?,
            Err(ureq::Error::Status(code, r)) => {
                let text = r.into_string().unwrap_or_default();
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                    .unwrap_or(text);
                return Err(match code {
                    401 | 403 => {
                        format!("сервис отклонил ключ ({code}): проверьте FLP_AI_KEY. {message}")
                    }
                    404 => format!(
                        "не найден адрес или модель (404): проверьте FLP_AI_BASE_URL и FLP_AI_MODEL. {message}"
                    ),
                    429 => format!("слишком много запросов (429), попробуйте позже: {message}"),
                    _ => format!("ошибка сервиса {code}: {message}"),
                });
            }
            Err(e) => return Err(format!("нет связи с сервисом: {e}")),
        };
        parse_reply(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> OpenAiCompat {
        OpenAiCompat::with_settings("https://h/v1/".into(), Some("k".into()), Some("m".into()))
            .unwrap()
    }

    #[test]
    fn settings_are_required() {
        assert!(OpenAiCompat::with_settings("u".into(), None, Some("m".into())).is_err());
        assert!(OpenAiCompat::with_settings("u".into(), Some("k".into()), None).is_err());
        assert_eq!(provider().base_url, "https://h/v1");
    }

    #[test]
    fn history_is_converted() {
        let history = [
            json!({"role": "user", "content": "привет"}),
            json!({"role": "assistant", "content": [
                {"type": "thinking", "thinking": "..."},
                {"type": "text", "text": "смотрю"},
                {"type": "tool_use", "id": "t1", "name": "get_schema", "input": {"a": 1}},
            ]}),
            json!({"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "t1", "content": "{}", "is_error": false},
            ]}),
        ];
        let body = provider().request_body("sys", &history, &crate::tools());
        let m = body["messages"].as_array().unwrap();
        assert_eq!(m.len(), 4);
        assert_eq!(m[0]["role"], "system");
        assert_eq!(m[2]["content"], "смотрю");
        let call = &m[2]["tool_calls"][0];
        assert_eq!(call["id"], "t1");
        assert_eq!(call["function"]["arguments"], "{\"a\":1}");
        assert_eq!(m[3]["role"], "tool");
        assert_eq!(m[3]["tool_call_id"], "t1");
        assert_eq!(body["tools"][0]["type"], "function");
        assert!(body["tools"][0]["function"]["parameters"].is_object());
    }

    #[test]
    fn tool_call_reply_becomes_tool_use() {
        let r = parse_reply(
            &json!({"choices": [{"finish_reason": "tool_calls", "message": {
                "content": null,
                "tool_calls": [{"id": "c1", "type": "function",
                    "function": {"name": "check", "arguments": "{\"x\": 2}"}}],
            }}]}),
        )
        .unwrap();
        assert_eq!(r.stop_reason, "tool_use");
        assert_eq!(r.content[0]["type"], "tool_use");
        assert_eq!(r.content[0]["input"]["x"], 2);
    }

    #[test]
    fn plain_and_truncated_replies() {
        let r = parse_reply(
            &json!({"choices": [{"finish_reason": "stop", "message": {"content": "готово"}}]}),
        )
        .unwrap();
        assert_eq!(r.stop_reason, "end_turn");
        assert_eq!(r.content[0]["text"], "готово");
        let r = parse_reply(
            &json!({"choices": [{"finish_reason": "length", "message": {"content": "до"}}]}),
        )
        .unwrap();
        assert_eq!(r.stop_reason, "max_tokens");
        assert!(parse_reply(&json!({"error": {"message": "bad"}})).is_err());
    }

    #[test]
    fn bad_arguments_are_an_error() {
        let r = parse_reply(
            &json!({"choices": [{"finish_reason": "tool_calls", "message": {
                "tool_calls": [{"function": {"name": "check", "arguments": "{oops"}}],
            }}]}),
        );
        assert!(r.is_err());
    }
}
