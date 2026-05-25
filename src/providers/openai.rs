use super::{BuildArgs, PreparedCli, PreparedRequest, StreamKind, json_headers, key_for};
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

pub(super) fn build_cli(args: &BuildArgs<'_>) -> PreparedCli {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into());
    let effort = if args.mode == Mode::Smart {
        "high"
    } else {
        "low"
    };
    PreparedCli {
        program: "codex".into(),
        args: vec![
            "exec".into(),
            "-m".into(),
            args.model.into(),
            "-c".into(),
            format!("model_reasoning_effort={}", effort),
            "-C".into(),
            cwd,
            "--ephemeral".into(),
            "--skip-git-repo-check".into(),
            "--ignore-user-config".into(),
            "-s".into(),
            "read-only".into(),
            "--json".into(),
            "-".into(),
        ],
        stdin: format!("{}\n\n{}", args.system, args.task),
        stream_kind: StreamKind::CodexCli,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Backend, Mode, Provider};
    use crate::util::settings::Settings;

    fn args_for(mode: Mode) -> BuildArgs<'static> {
        BuildArgs {
            provider: Provider::Openai,
            backend: Backend::Cli,
            system: "sys",
            task: "task",
            model: "gpt-5.4",
            mode,
            max_tok: 1000,
            stop: Vec::new(),
            settings: Box::leak(Box::new(Settings::default())),
        }
    }

    #[test]
    fn cli_passes_low_effort_in_fast_mode() {
        let cli = build_cli(&args_for(Mode::Fast));
        let i = cli.args.iter().position(|a| a == "-c").expect("-c arg");
        assert_eq!(cli.args[i + 1], "model_reasoning_effort=low");
    }

    #[test]
    fn cli_passes_high_effort_in_smart_mode() {
        let cli = build_cli(&args_for(Mode::Smart));
        let i = cli.args.iter().position(|a| a == "-c").expect("-c arg");
        assert_eq!(cli.args[i + 1], "model_reasoning_effort=high");
    }
}
