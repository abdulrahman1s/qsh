use super::{BuildArgs, PreparedRequest, json_headers, key_for};
use crate::config::Provider;
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
    let thinking_level = args
        .settings
        .effort(Provider::Gemini, args.model, args.mode);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Backend, Mode};
    use crate::util::settings::Settings;

    #[test]
    fn configured_effort_sets_thinking_level() {
        let mut settings = Settings::default();
        settings
            .providers
            .entry("gemini".into())
            .or_default()
            .effort
            .insert("gemini-3.5-flash".into(), "medium".into());
        let args = BuildArgs {
            provider: Provider::Gemini,
            backend: Backend::Api,
            system: "sys",
            task: "task",
            model: "gemini-3.5-flash",
            mode: Mode::Fast,
            max_tok: 1000,
            stop: Vec::new(),
            settings: &settings,
        };
        assert_eq!(
            build(&args).body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "medium"
        );
    }
}
