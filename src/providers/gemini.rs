use super::{BuildArgs, PreparedRequest, json_headers, key_for};
use crate::config::{Mode, Provider};
use serde_json::{Value, json};

pub(super) const API_KEY_ENV: &str = "GEMINI_API_KEY";
pub(super) const MODEL_ENV: &str = "GEMINI_MODEL";
pub(super) const DEFAULT_MODEL: &str = "gemini-3.5-flash";

pub(super) fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    let mut headers = json_headers();
    headers.push((
        "x-goog-api-key".into(),
        key_for(Provider::Gemini, args.settings),
    ));
    let thinking_level = if args.mode == Mode::Smart {
        "high"
    } else {
        "low"
    };
    let body = json!({
        "system_instruction": {"parts": [{"text": args.system}]},
        "contents": [{"parts": [{"text": args.task}]}],
        "generationConfig": {
            "temperature": 0.2,
            "maxOutputTokens": args.max_tok,
            "stopSequences": args.stop,
            "thinkingConfig": {"thinkingLevel": thinking_level},
        }
    });
    PreparedRequest {
        url: format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            args.model
        ),
        headers,
        body,
        provider: Provider::Gemini,
    }
}

pub(super) fn extract_delta(v: &Value) -> Option<String> {
    let parts = v.pointer("/candidates/0/content/parts")?.as_array()?;
    let mut out = String::new();
    for p in parts {
        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
            out.push_str(t);
        }
    }
    (!out.is_empty()).then_some(out)
}
