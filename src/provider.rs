use crate::config::{CLAUDE_SMART_BUDGET, CLAUDE_SMART_MAX, Mode, Provider};
use serde_json::{Value, json};
use std::env;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
    pub provider: Provider,
}

pub fn api_key_env(p: Provider) -> Option<&'static str> {
    match p {
        Provider::Gemini => Some("GEMINI_API_KEY"),
        Provider::Openai => Some("OPENAI_API_KEY"),
        Provider::Claude => Some("ANTHROPIC_API_KEY"),
        Provider::Ollama => None,
    }
}

pub fn default_model(p: Provider) -> Option<&'static str> {
    match p {
        Provider::Gemini => Some("gemini-3.5-flash"),
        Provider::Openai => Some("gpt-5.4-mini"),
        Provider::Claude => Some("claude-sonnet-4-6"),
        Provider::Ollama => None,
    }
}

pub fn model_env(p: Provider) -> &'static str {
    match p {
        Provider::Gemini => "GEMINI_MODEL",
        Provider::Openai => "OPENAI_MODEL",
        Provider::Claude => "ANTHROPIC_MODEL",
        Provider::Ollama => "OLLAMA_MODEL",
    }
}

pub fn ollama_url() -> String {
    let base = env::var("OLLAMA_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| env::var("OLLAMA_HOST").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "http://127.0.0.1:11434".into());
    let mut base = if base.starts_with("http://") || base.starts_with("https://") {
        base
    } else {
        format!("http://{}", base)
    };
    while base.ends_with('/') {
        base.pop();
    }
    if base.ends_with("/v1") {
        format!("{}/chat/completions", base)
    } else {
        format!("{}/v1/chat/completions", base)
    }
}

pub fn default_ollama_model() -> Option<String> {
    if which::which("ollama").is_err() {
        return None;
    }
    let out = Command::new("ollama").arg("list").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for (i, line) in text.lines().enumerate() {
        if i == 0 {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if !first.is_empty() {
            return Some(first.to_string());
        }
    }
    None
}

pub fn resolve_provider(
    explicit: Option<Provider>,
    env_pref: Option<String>,
    model_hint: &mut Option<String>,
) -> Option<Provider> {
    if let Some(p) = explicit {
        return Some(p);
    }
    if let Some(s) = env_pref.as_deref()
        && let Some(p) = Provider::parse(s)
    {
        return Some(p);
    }
    let has = |k: &str| env::var(k).map(|v| !v.is_empty()).unwrap_or(false);
    if has("GEMINI_API_KEY") {
        return Some(Provider::Gemini);
    }
    if has("ANTHROPIC_API_KEY") {
        return Some(Provider::Claude);
    }
    if has("OPENAI_API_KEY") {
        return Some(Provider::Openai);
    }
    if has("OLLAMA_MODEL") {
        return Some(Provider::Ollama);
    }
    if let Some(m) = default_ollama_model() {
        if model_hint.is_none() {
            *model_hint = Some(m);
        }
        return Some(Provider::Ollama);
    }
    None
}

pub fn require_key(p: Provider) -> Result<(), String> {
    if let Some(key) = api_key_env(p) {
        let v = env::var(key).unwrap_or_default();
        if v.is_empty() {
            return Err(format!("{} not set", key));
        }
    }
    Ok(())
}

pub fn resolve_model(p: Provider, current: Option<String>) -> Result<String, String> {
    if let Some(m) = current.filter(|s| !s.is_empty()) {
        return Ok(m);
    }
    let env_var = model_env(p);
    if let Ok(v) = env::var(env_var)
        && !v.is_empty()
    {
        return Ok(v);
    }
    if let Some(d) = default_model(p) {
        return Ok(d.to_string());
    }
    if p == Provider::Ollama {
        if let Some(m) = default_ollama_model() {
            return Ok(m);
        }
        return Err("no Ollama model found. Pass -m MODEL or set OLLAMA_MODEL.".into());
    }
    Err("no default model".into())
}

pub fn stream_filter_kind(p: Provider) -> StreamKind {
    match p {
        Provider::Gemini => StreamKind::Gemini,
        Provider::Openai => StreamKind::Openai,
        Provider::Claude => StreamKind::Claude,
        Provider::Ollama => StreamKind::Ollama,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StreamKind {
    Gemini,
    Openai,
    Claude,
    Ollama,
}

/// Extract the next text delta from a single SSE `data: {…}` JSON payload.
pub fn extract_delta(kind: StreamKind, payload: &str) -> Option<String> {
    if payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    match kind {
        StreamKind::Gemini => {
            let parts = v.pointer("/candidates/0/content/parts")?.as_array()?;
            let mut out = String::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    out.push_str(t);
                }
            }
            (!out.is_empty()).then_some(out)
        }
        StreamKind::Openai => {
            let t = v.get("type")?.as_str()?;
            if t != "response.output_text.delta" {
                return None;
            }
            v.get("delta").and_then(|d| d.as_str()).map(String::from)
        }
        StreamKind::Claude => {
            let t = v.get("type")?.as_str()?;
            if t != "content_block_delta" {
                return None;
            }
            let dt = v.pointer("/delta/type")?.as_str()?;
            if dt != "text_delta" {
                return None;
            }
            v.pointer("/delta/text")
                .and_then(|t| t.as_str())
                .map(String::from)
        }
        StreamKind::Ollama => v
            .pointer("/choices/0/delta/content")
            .and_then(|t| t.as_str())
            .map(String::from),
    }
}

pub struct BuildArgs<'a> {
    pub provider: Provider,
    pub system: &'a str,
    pub task: &'a str,
    pub model: &'a str,
    pub mode: Mode,
    pub max_tok: u32,
    pub stop: Vec<String>,
}

pub fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    let url;
    let body;
    match args.provider {
        Provider::Gemini => {
            url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
                args.model
            );
            if let Some(k) = api_key_env(args.provider) {
                let v = env::var(k).unwrap_or_default();
                headers.push(("x-goog-api-key".into(), v));
            }
            let lvl = if args.mode == Mode::Smart {
                "high"
            } else {
                "low"
            };
            body = json!({
                "system_instruction": {"parts": [{"text": args.system}]},
                "contents": [{"parts": [{"text": args.task}]}],
                "generationConfig": {
                    "temperature": 0.2,
                    "maxOutputTokens": args.max_tok,
                    "stopSequences": args.stop,
                    "thinkingConfig": {"thinkingLevel": lvl},
                }
            });
        }
        Provider::Openai => {
            url = "https://api.openai.com/v1/responses".to_string();
            if let Some(k) = api_key_env(args.provider) {
                let v = env::var(k).unwrap_or_default();
                headers.push(("Authorization".into(), format!("Bearer {}", v)));
            }
            let effort = if args.mode == Mode::Smart {
                "high"
            } else {
                "low"
            };
            body = json!({
                "model": args.model,
                "max_output_tokens": args.max_tok,
                "reasoning": {"effort": effort},
                "instructions": args.system,
                "input": args.task,
                "stream": true,
            });
        }
        Provider::Claude => {
            url = "https://api.anthropic.com/v1/messages".to_string();
            if let Some(k) = api_key_env(args.provider) {
                let v = env::var(k).unwrap_or_default();
                headers.push(("x-api-key".into(), v));
            }
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
            body = if args.mode == Mode::Smart {
                json!({
                    "model": args.model,
                    "max_tokens": CLAUDE_SMART_MAX,
                    "thinking": {"type": "enabled", "budget_tokens": CLAUDE_SMART_BUDGET},
                    "system": args.system,
                    "messages": [{"role": "user", "content": args.task}],
                    "stream": true,
                })
            } else {
                json!({
                    "model": args.model,
                    "max_tokens": args.max_tok,
                    "temperature": 0.2,
                    "stop_sequences": args.stop,
                    "system": args.system,
                    "messages": [{"role": "user", "content": args.task}],
                    "stream": true,
                })
            };
        }
        Provider::Ollama => {
            url = ollama_url();
            body = json!({
                "model": args.model,
                "messages": [
                    {"role": "system", "content": args.system},
                    {"role": "user", "content": args.task},
                ],
                "stream": true,
                "temperature": 0.2,
                "max_tokens": args.max_tok,
                "stop": args.stop,
            });
        }
    }
    PreparedRequest {
        url,
        headers,
        body,
        provider: args.provider,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_gemini_delta() {
        let p = r#"{"candidates":[{"content":{"parts":[{"text":"hi "},{"text":"there"}]}}]}"#;
        assert_eq!(
            extract_delta(StreamKind::Gemini, p).as_deref(),
            Some("hi there")
        );
    }

    #[test]
    fn extract_openai_delta() {
        let p = r#"{"type":"response.output_text.delta","delta":"ls"}"#;
        assert_eq!(extract_delta(StreamKind::Openai, p).as_deref(), Some("ls"));
        let other = r#"{"type":"response.created"}"#;
        assert!(extract_delta(StreamKind::Openai, other).is_none());
    }

    #[test]
    fn extract_claude_delta() {
        let p = r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"echo hi"}}"#;
        assert_eq!(
            extract_delta(StreamKind::Claude, p).as_deref(),
            Some("echo hi")
        );
        let thinking =
            r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"..."}}"#;
        assert!(extract_delta(StreamKind::Claude, thinking).is_none());
    }

    #[test]
    fn extract_ollama_delta() {
        let p = r#"{"choices":[{"delta":{"content":"find"}}]}"#;
        assert_eq!(
            extract_delta(StreamKind::Ollama, p).as_deref(),
            Some("find")
        );
    }

    #[test]
    fn ollama_url_normalises() {
        // Cannot mutate env in tests safely without locks; just check parse logic via direct call shape.
        // The default with no env vars should produce a v1 chat endpoint.
        unsafe { std::env::remove_var("OLLAMA_BASE_URL") };
        unsafe { std::env::remove_var("OLLAMA_HOST") };
        let u = ollama_url();
        assert!(u.ends_with("/v1/chat/completions"));
    }
}
