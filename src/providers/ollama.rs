use super::{BuildArgs, PreparedRequest, json_headers};
use crate::config::Provider;
use crate::util::settings::Settings;
use serde_json::{Value, json};
use std::env;
use std::process::Command;

pub(super) const MODEL_ENV: &str = "OLLAMA_MODEL";

pub(super) fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    let body = json!({
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
    PreparedRequest {
        url: url(args.settings),
        headers: json_headers(),
        body,
        provider: Provider::Ollama,
    }
}

pub(super) fn url(settings: &Settings) -> String {
    let base = settings
        .ollama_base_url()
        .map(|s| s.to_string())
        .or_else(|| env::var("OLLAMA_BASE_URL").ok().filter(|s| !s.is_empty()))
        .or_else(|| env::var("OLLAMA_HOST").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "http://127.0.0.1:11434".into());
    normalize_url(&base)
}

pub(super) fn normalize_url(base: &str) -> String {
    let mut base = if base.starts_with("http://") || base.starts_with("https://") {
        base.to_string()
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

pub(super) fn default_model_from_installed() -> Option<String> {
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

pub(super) fn extract_delta(v: &Value) -> Option<String> {
    v.pointer("/choices/0/delta/content")
        .and_then(|t| t.as_str())
        .map(String::from)
}
