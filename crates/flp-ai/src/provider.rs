//! AI providers behind one trait; Claude is the default. Others (OpenAI, Ollama)
//! can be added as more implementations of [`Provider`].

use std::time::Duration;

use serde_json::{json, Value};

use crate::Tool;

/// One model call: the conversation so far in, the assistant message out.
///
/// Messages use the Messages API shape (`role`, `content` blocks); providers with
/// another wire format convert at their boundary.
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn complete(&self, system: &str, messages: &[Value], tools: &[Tool]) -> Result<Reply, String>;
}

/// Assistant reply: content blocks to append to the history as they are, and why
/// the model stopped (`end_turn`, `tool_use`, `max_tokens`, `refusal`, ...).
#[derive(Debug, Clone)]
pub struct Reply {
    pub content: Vec<Value>,
    pub stop_reason: String,
}

/// Picks the provider from the environment: an OpenAI-compatible endpoint when
/// `FLP_AI_BASE_URL` is set, Claude otherwise.
pub fn from_env() -> Result<Box<dyn Provider>, String> {
    match crate::openai::OpenAiCompat::from_env() {
        Some(p) => Ok(Box::new(p?)),
        None => Ok(Box::new(Claude::from_env()?)),
    }
}

/// Claude over the Messages API (raw HTTP: there is no official Rust SDK).
pub struct Claude {
    api_key: String,
    pub model: String,
    base_url: String,
}

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";

impl Claude {
    /// Reads the key from `ANTHROPIC_API_KEY`; the model from `FLP_AI_MODEL` if set.
    pub fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .ok_or("не задан ключ Claude: установите переменную окружения ANTHROPIC_API_KEY и перезапустите программу")?;
        Ok(Self {
            api_key,
            model: std::env::var("FLP_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned()),
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_owned()),
        })
    }

    pub(crate) fn request_body(&self, system: &str, messages: &[Value], tools: &[Tool]) -> Value {
        let mut tools: Vec<Value> = tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema}))
            .collect();
        // Prompt caching: tools and system are identical on every step, and the
        // history grows by appending, so the last block is a moving breakpoint.
        if let Some(last) = tools.last_mut() {
            last["cache_control"] = json!({"type": "ephemeral"});
        }
        let mut messages = messages.to_vec();
        if let Some(block) = messages
            .last_mut()
            .and_then(|m| m["content"].as_array_mut())
            .and_then(|blocks| blocks.last_mut())
        {
            block["cache_control"] = json!({"type": "ephemeral"});
        }
        json!({
            "model": self.model,
            "max_tokens": 16000,
            "system": [{"type": "text", "text": system, "cache_control": {"type": "ephemeral"}}],
            "messages": messages,
            "tools": tools,
            // Every tool step pays this latency again; `FLP_AI_EFFORT=high` for harder schemas.
            "output_config": {"effort": std::env::var("FLP_AI_EFFORT").unwrap_or_else(|_| "medium".to_owned())},
            // On a safety decline the API retries on a fallback model in the same call.
            "fallbacks": "default",
        })
    }
}

impl Provider for Claude {
    fn name(&self) -> &str {
        &self.model
    }

    fn complete(&self, system: &str, messages: &[Value], tools: &[Tool]) -> Result<Reply, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(600))
            .build();
        let response = agent
            .post(&format!("{}/v1/messages", self.base_url))
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", "2023-06-01")
            .set("anthropic-beta", "server-side-fallback-2026-07-01")
            .set("content-type", "application/json")
            .send_json(self.request_body(system, messages, tools));
        let body: Value = match response {
            Ok(r) => r
                .into_json()
                .map_err(|e| format!("ответ Claude не прочитан: {e}"))?,
            Err(ureq::Error::Status(code, r)) => {
                let text = r.into_string().unwrap_or_default();
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                    .unwrap_or(text);
                return Err(match code {
                    401 => "Claude отклонил ключ (401): проверьте ANTHROPIC_API_KEY".to_owned(),
                    429 => format!(
                        "слишком много запросов к Claude (429), попробуйте позже: {message}"
                    ),
                    _ => format!("ошибка Claude {code}: {message}"),
                });
            }
            Err(e) => return Err(format!("нет связи с Claude: {e}")),
        };
        parse_reply(&body)
    }
}

pub(crate) fn parse_reply(body: &Value) -> Result<Reply, String> {
    let stop_reason = body["stop_reason"].as_str().unwrap_or("").to_owned();
    if stop_reason == "refusal" {
        let why = body["stop_details"]["explanation"].as_str().unwrap_or("");
        return Err(format!("Claude отказался отвечать на этот запрос. {why}"));
    }
    let content = body["content"]
        .as_array()
        .cloned()
        .ok_or("в ответе Claude нет content")?;
    Ok(Reply {
        content,
        stop_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_shape() {
        let c = Claude {
            api_key: "k".into(),
            model: DEFAULT_MODEL.into(),
            base_url: "x".into(),
        };
        let body = c.request_body(
            "sys",
            &[json!({"role": "user", "content": "hi"})],
            &crate::tools(),
        );
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        assert!(body.get("thinking").is_none());
        assert!(body["tools"][0]["input_schema"].is_object());
    }

    #[test]
    fn refusal_is_an_error() {
        let r = parse_reply(
            &json!({"stop_reason": "refusal", "stop_details": {"explanation": "нет"}, "content": []}),
        );
        assert!(r.is_err());
        let r = parse_reply(
            &json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": "ok"}]}),
        )
        .unwrap();
        assert_eq!(r.content.len(), 1);
    }
}
