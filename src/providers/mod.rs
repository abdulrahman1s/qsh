pub mod stream;

mod claude;
mod gemini;
mod ollama;
mod openai;

use crate::config::{Mode, Provider};
use crate::util::settings::Settings;
use serde_json::Value;
use std::env;

#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
    pub provider: Provider,
}

pub struct BuildArgs<'a> {
    pub provider: Provider,
    pub system: &'a str,
    pub task: &'a str,
    pub model: &'a str,
    pub mode: Mode,
    pub max_tok: u32,
    pub stop: Vec<String>,
    pub settings: &'a Settings,
}

#[derive(Debug, Clone, Copy)]
pub enum StreamKind {
    Gemini,
    Openai,
    Claude,
    Ollama,
}

pub fn api_key_env(p: Provider) -> Option<&'static str> {
    match p {
        Provider::Gemini => Some(gemini::API_KEY_ENV),
        Provider::Openai => Some(openai::API_KEY_ENV),
        Provider::Claude => Some(claude::API_KEY_ENV),
        Provider::Ollama => None,
    }
}

pub fn default_model(p: Provider) -> Option<&'static str> {
    match p {
        Provider::Gemini => Some(gemini::DEFAULT_MODEL),
        Provider::Openai => Some(openai::DEFAULT_MODEL),
        Provider::Claude => Some(claude::DEFAULT_MODEL),
        Provider::Ollama => None,
    }
}

pub fn model_env(p: Provider) -> &'static str {
    match p {
        Provider::Gemini => gemini::MODEL_ENV,
        Provider::Openai => openai::MODEL_ENV,
        Provider::Claude => claude::MODEL_ENV,
        Provider::Ollama => ollama::MODEL_ENV,
    }
}

pub fn default_ollama_model() -> Option<String> {
    ollama::default_model_from_installed()
}

pub fn resolve_provider(
    explicit: Option<Provider>,
    env_pref: Option<String>,
    model_hint: &mut Option<String>,
    settings: &Settings,
) -> Option<Provider> {
    if let Some(p) = explicit {
        return Some(p);
    }
    if let Some(s) = env_pref.as_deref()
        && let Some(p) = Provider::parse(s)
    {
        return Some(p);
    }
    if let Some(s) = settings.provider.as_deref()
        && let Some(p) = Provider::parse(s)
    {
        return Some(p);
    }
    let has_key = |p: Provider, env_key: &str| -> bool {
        settings.api_key(p).is_some() || env::var(env_key).map(|v| !v.is_empty()).unwrap_or(false)
    };
    if has_key(Provider::Gemini, gemini::API_KEY_ENV) {
        return Some(Provider::Gemini);
    }
    if has_key(Provider::Claude, claude::API_KEY_ENV) {
        return Some(Provider::Claude);
    }
    if has_key(Provider::Openai, openai::API_KEY_ENV) {
        return Some(Provider::Openai);
    }
    if settings.model(Provider::Ollama).is_some()
        || env::var(ollama::MODEL_ENV)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    {
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

pub fn require_key(p: Provider, settings: &Settings) -> Result<(), String> {
    if settings.api_key(p).is_some() {
        return Ok(());
    }
    if let Some(key) = api_key_env(p) {
        let v = env::var(key).unwrap_or_default();
        if v.is_empty() {
            return Err(format!(
                "no API key for {p}: set ${key} in your environment, or run:\n  echo $YOUR_KEY | qsh config set providers.{p}.api_key",
                p = p.as_str(),
                key = key,
            ));
        }
    }
    Ok(())
}

pub fn resolve_model(
    p: Provider,
    current: Option<String>,
    settings: &Settings,
) -> Result<String, String> {
    if let Some(m) = current.filter(|s| !s.is_empty()) {
        return Ok(m);
    }
    if let Some(m) = settings.model(p) {
        return Ok(m.to_string());
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

/// Extract the next text delta from a single SSE `data: {…}` JSON payload.
pub fn extract_delta(kind: StreamKind, payload: &str) -> Option<String> {
    if payload == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(payload).ok()?;
    match kind {
        StreamKind::Gemini => gemini::extract_delta(&v),
        StreamKind::Openai => openai::extract_delta(&v),
        StreamKind::Claude => claude::extract_delta(&v),
        StreamKind::Ollama => ollama::extract_delta(&v),
    }
}

pub fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    match args.provider {
        Provider::Gemini => gemini::build(args),
        Provider::Openai => openai::build(args),
        Provider::Claude => claude::build(args),
        Provider::Ollama => ollama::build(args),
    }
}

fn json_headers() -> Vec<(String, String)> {
    vec![("Content-Type".to_string(), "application/json".to_string())]
}

fn key_for(p: Provider, settings: &Settings) -> String {
    if let Some(k) = settings.api_key(p) {
        return k.to_string();
    }
    if let Some(env_key) = api_key_env(p) {
        return env::var(env_key).unwrap_or_default();
    }
    String::new()
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
        // Exercise the pure normalizer to avoid env-var mutation in tests.
        assert_eq!(
            ollama::normalize_url("127.0.0.1:11434"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            ollama::normalize_url("http://127.0.0.1:11434/"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            ollama::normalize_url("https://example.com/v1"),
            "https://example.com/v1/chat/completions"
        );
    }
}
