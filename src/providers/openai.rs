use super::{BuildArgs, PreparedRequest, json_headers, key_for};
use crate::config::{Mode, Provider};
use serde_json::{Value, json};

pub(super) const API_KEY_ENV: &str = "OPENAI_API_KEY";
pub(super) const MODEL_ENV: &str = "OPENAI_MODEL";
pub(super) const DEFAULT_MODEL: &str = "gpt-5.4-mini";

pub(super) fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    let mut headers = json_headers();
    headers.push((
        "Authorization".into(),
        format!("Bearer {}", key_for(Provider::Openai, args.settings)),
    ));
    let effort = if args.mode == Mode::Smart {
        "high"
    } else {
        "low"
    };
    let body = json!({
        "model": args.model,
        "max_output_tokens": args.max_tok,
        "reasoning": {"effort": effort},
        "instructions": args.system,
        "input": args.task,
        "stream": true,
    });
    PreparedRequest {
        url: "https://api.openai.com/v1/responses".to_string(),
        headers,
        body,
        provider: Provider::Openai,
    }
}

pub(super) fn extract_delta(v: &Value) -> Option<String> {
    let t = v.get("type")?.as_str()?;
    if t != "response.output_text.delta" {
        return None;
    }
    v.get("delta").and_then(|d| d.as_str()).map(String::from)
}
