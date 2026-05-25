pub mod stream;

mod claude;
mod gemini;
mod ollama;
mod openai;

use crate::config::{Backend, Mode, Provider};
use crate::util::settings::Settings;
use serde_json::Value;
use std::env;

#[derive(Debug, Clone)]
pub enum PreparedInvocation {
    Http(PreparedRequest),
    Cli(PreparedCli),
}

#[derive(Debug, Clone)]
pub struct PreparedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
    pub provider: Provider,
}

#[derive(Debug, Clone)]
pub struct PreparedCli {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: String,
    pub stream_kind: StreamKind,
    pub provider: Provider,
}

pub struct BuildArgs<'a> {
    pub provider: Provider,
    pub backend: Backend,
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
    ClaudeCli,
    CodexCli,
}

pub trait BinaryProbe {
    fn has(&self, name: &str) -> bool;
}

pub struct RealProbe;

impl BinaryProbe for RealProbe {
    fn has(&self, name: &str) -> bool {
        which::which(name).is_ok()
    }
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
    env_pref: Option<&str>,
    qshrc_provider: Option<&str>,
    model_hint: &mut Option<String>,
    settings: &Settings,
    probe: &dyn BinaryProbe,
) -> Option<(Provider, Option<Backend>)> {
    resolve_provider_with_env(
        explicit,
        env_pref,
        qshrc_provider,
        model_hint,
        settings,
        probe,
        &real_env,
        &default_ollama_model,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_provider_with_env(
    explicit: Option<Provider>,
    env_pref: Option<&str>,
    qshrc_provider: Option<&str>,
    model_hint: &mut Option<String>,
    settings: &Settings,
    probe: &dyn BinaryProbe,
    env_value: &dyn Fn(&str) -> Option<String>,
    default_ollama: &dyn Fn() -> Option<String>,
) -> Option<(Provider, Option<Backend>)> {
    if let Some(p) = explicit {
        return Some((p, None));
    }
    if let Some(s) = env_pref
        && let Some(p) = Provider::parse(s)
    {
        return Some((p, None));
    }
    if let Some(s) = settings.provider.as_deref()
        && let Some(p) = Provider::parse(s)
    {
        return Some((p, None));
    }
    if let Some(s) = qshrc_provider
        && let Some(p) = Provider::parse(s)
    {
        return Some((p, None));
    }
    let has_key = |p: Provider, env_key: &str| -> bool {
        settings.api_key(p).is_some() || env_value(env_key).is_some()
    };
    if has_key(Provider::Gemini, gemini::API_KEY_ENV) {
        return Some((Provider::Gemini, None));
    }
    if has_key(Provider::Claude, claude::API_KEY_ENV) {
        return Some((Provider::Claude, None));
    }
    if has_key(Provider::Openai, openai::API_KEY_ENV) {
        return Some((Provider::Openai, None));
    }
    if settings.model(Provider::Ollama).is_some() || env_value(ollama::MODEL_ENV).is_some() {
        return Some((Provider::Ollama, None));
    }
    if let Some(m) = default_ollama() {
        if model_hint.is_none() {
            *model_hint = Some(m);
        }
        return Some((Provider::Ollama, None));
    }
    if probe.has("claude") {
        return Some((Provider::Claude, Some(Backend::Cli)));
    }
    if probe.has("codex") {
        return Some((Provider::Openai, Some(Backend::Cli)));
    }
    None
}

pub fn resolve_backend(
    p: Provider,
    explicit: Option<Backend>,
    env_pref: Option<&str>,
    settings: &Settings,
    qshrc_backend: Option<&str>,
    detected: Option<Backend>,
) -> Backend {
    let chosen = explicit
        .or_else(|| env_pref.and_then(Backend::parse))
        .or_else(|| settings.backend(p))
        .or_else(|| qshrc_backend.and_then(Backend::parse))
        .or(detected)
        .unwrap_or(Backend::Api);

    if Backend::supports(p, chosen) {
        chosen
    } else {
        Backend::Api
    }
}

pub fn require_auth(
    p: Provider,
    b: Backend,
    settings: &Settings,
    probe: &dyn BinaryProbe,
) -> Result<(), String> {
    require_auth_with_env(p, b, settings, probe, &real_env)
}

fn require_auth_with_env(
    p: Provider,
    b: Backend,
    settings: &Settings,
    probe: &dyn BinaryProbe,
    env_value: &dyn Fn(&str) -> Option<String>,
) -> Result<(), String> {
    match b {
        Backend::Api => require_api_key_with_env(p, settings, env_value),
        Backend::Cli => match p {
            Provider::Claude => probe.has("claude").then_some(()).ok_or_else(|| {
                "claude CLI not found. Install Claude Code, then run: claude /login".into()
            }),
            Provider::Openai => probe
                .has("codex")
                .then_some(())
                .ok_or_else(|| "codex CLI not found. Install Codex, then run: codex login".into()),
            _ => Err(format!("CLI backend not supported for {}", p.as_str())),
        },
    }
}

fn require_api_key_with_env(
    p: Provider,
    settings: &Settings,
    env_value: &dyn Fn(&str) -> Option<String>,
) -> Result<(), String> {
    if settings.api_key(p).is_some() {
        return Ok(());
    }
    if let Some(key) = api_key_env(p)
        && env_value(key).is_none()
    {
        return Err(format!(
            "no API key for {p}: set ${key} in your environment, or run:\n  echo $YOUR_KEY | qsh config set providers.{p}.api_key",
            p = p.as_str(),
            key = key,
        ));
    }
    Ok(())
}

fn real_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
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
        StreamKind::ClaudeCli => {
            let event = v.pointer("/event").unwrap_or(&v);
            claude::extract_delta(event)
        }
        StreamKind::CodexCli => {
            if v.get("type").and_then(|t| t.as_str()) != Some("item.completed") {
                return None;
            }
            if v.pointer("/item/type").and_then(|t| t.as_str()) != Some("agent_message") {
                return None;
            }
            v.pointer("/item/text")
                .and_then(|t| t.as_str())
                .map(String::from)
        }
    }
}

pub fn build(args: &BuildArgs<'_>) -> PreparedInvocation {
    match (args.provider, args.backend) {
        (Provider::Gemini, Backend::Api) => PreparedInvocation::Http(gemini::build(args)),
        (Provider::Openai, Backend::Api) => PreparedInvocation::Http(openai::build(args)),
        (Provider::Claude, Backend::Api) => PreparedInvocation::Http(claude::build(args)),
        (Provider::Ollama, Backend::Api) => PreparedInvocation::Http(ollama::build(args)),
        (Provider::Claude, Backend::Cli) => PreparedInvocation::Cli(claude::build_cli(args)),
        (Provider::Openai, Backend::Cli) => PreparedInvocation::Cli(openai::build_cli(args)),
        (_, Backend::Cli) => unreachable!("unsupported CLI backend passed validation"),
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
    use crate::util::settings::ProviderSettings;

    struct FakeProbe {
        bins: &'static [&'static str],
    }

    impl BinaryProbe for FakeProbe {
        fn has(&self, name: &str) -> bool {
            self.bins.contains(&name)
        }
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    fn settings_with_key(p: Provider) -> Settings {
        let mut settings = Settings::default();
        settings.providers.insert(
            p.as_str().into(),
            ProviderSettings {
                api_key: Some("test-key".into()),
                ..Default::default()
            },
        );
        settings
    }

    #[test]
    fn backend_config_parses() {
        let src = r#"
[providers.claude]
backend = "cli"
"#;
        let settings: Settings = toml::from_str(src).unwrap();
        assert_eq!(settings.backend(Provider::Claude), Some(Backend::Cli));
    }

    #[test]
    fn backend_defaults_to_api() {
        let settings = Settings::default();
        assert_eq!(
            resolve_backend(Provider::Claude, None, None, &settings, None, None),
            Backend::Api
        );
    }

    #[test]
    fn auto_detect_prefers_api_over_cli() {
        let settings = settings_with_key(Provider::Claude);
        let probe = FakeProbe { bins: &["claude"] };
        let mut model = None;
        let resolved = resolve_provider_with_env(
            None,
            None,
            None,
            &mut model,
            &settings,
            &probe,
            &no_env,
            &|| None,
        );
        assert_eq!(resolved, Some((Provider::Claude, None)));
        assert_eq!(
            resolve_backend(
                Provider::Claude,
                None,
                None,
                &settings,
                None,
                resolved.and_then(|(_, b)| b),
            ),
            Backend::Api
        );
    }

    #[test]
    fn auto_detect_falls_back_to_claude_cli() {
        let settings = Settings::default();
        let probe = FakeProbe { bins: &["claude"] };
        let mut model = None;
        let resolved = resolve_provider_with_env(
            None,
            None,
            None,
            &mut model,
            &settings,
            &probe,
            &no_env,
            &|| None,
        );
        assert_eq!(resolved, Some((Provider::Claude, Some(Backend::Cli))));
        assert_eq!(
            resolve_backend(
                Provider::Claude,
                None,
                None,
                &settings,
                None,
                resolved.and_then(|(_, b)| b),
            ),
            Backend::Cli
        );
    }

    #[test]
    fn require_auth_cli_missing_binary() {
        let settings = Settings::default();
        let probe = FakeProbe { bins: &[] };
        let err = require_auth_with_env(Provider::Claude, Backend::Cli, &settings, &probe, &no_env)
            .unwrap_err();
        assert!(err.contains("claude /login"));
    }

    #[test]
    fn require_auth_api_no_key_errors() {
        let settings = Settings::default();
        let probe = FakeProbe { bins: &[] };
        let err = require_auth_with_env(Provider::Claude, Backend::Api, &settings, &probe, &no_env)
            .unwrap_err();
        assert!(err.contains("no API key"));
    }

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
    fn extract_claude_cli_delta() {
        let p = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"echo hi"}},"session_id":"abc","parent_tool_use_id":null,"uuid":"x"}"#;
        assert_eq!(
            extract_delta(StreamKind::ClaudeCli, p).as_deref(),
            Some("echo hi")
        );
        let non_delta = r#"{"type":"stream_event","event":{"type":"message_start","message":{}}}"#;
        assert!(extract_delta(StreamKind::ClaudeCli, non_delta).is_none());
        let assistant =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"echo hi"}]}}"#;
        assert!(extract_delta(StreamKind::ClaudeCli, assistant).is_none());
    }

    #[test]
    fn extract_codex_cli_delta() {
        let p = r#"{"type":"item.completed","item":{"type":"agent_message","text":"git status"}}"#;
        assert_eq!(
            extract_delta(StreamKind::CodexCli, p).as_deref(),
            Some("git status")
        );
        let other = r#"{"type":"item.completed","item":{"type":"tool_call","text":"nope"}}"#;
        assert!(extract_delta(StreamKind::CodexCli, other).is_none());
    }

    #[test]
    fn backend_unsupported_for_gemini() {
        assert!(!Backend::supports(Provider::Gemini, Backend::Cli));
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
