use super::{BuildArgs, PreparedRequest, json_headers, key_for};
use crate::config::{Mode, Provider};
use serde_json::{Value, json};

pub(super) const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
pub(super) const MODEL_ENV: &str = "ANTHROPIC_MODEL";
pub(super) const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

pub(super) fn build(args: &BuildArgs<'_>) -> PreparedRequest {
    let mut headers = json_headers();
    headers.push(("x-api-key".into(), key_for(Provider::Claude, args.settings)));
    headers.push(("anthropic-version".into(), "2023-06-01".into()));
    let body = if args.mode == Mode::Smart {
        json!({
            "model": args.model,
            "max_tokens": args.settings.claude_smart_max(),
            "thinking": {"type": "enabled", "budget_tokens": args.settings.claude_thinking_budget()},
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
    PreparedRequest {
        url: "https://api.anthropic.com/v1/messages".to_string(),
        headers,
        body,
        provider: Provider::Claude,
    }
}

pub(super) fn extract_delta(v: &Value) -> Option<String> {
    let t = v.get("type")?.as_str()?;
    if t != "content_block_delta" {
        return None;
    }
    let delta_type = v.pointer("/delta/type")?.as_str()?;
    if delta_type != "text_delta" {
        return None;
    }
    v.pointer("/delta/text")
        .and_then(|t| t.as_str())
        .map(String::from)
}
