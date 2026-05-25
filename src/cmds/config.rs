use super::cli::ConfigSetArgs;
use crate::config::{Backend, Mode, Provider};
use crate::providers as provider_helpers;
use crate::util::{
    cache,
    settings::{self, Settings},
};
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

const TEMPLATE: &str = r#"# qsh config
# Generated automatically. Edit with `qsh config edit` or `qsh config set`.
# All keys are optional. Precedence: CLI > qsh.toml > this file > env vars > defaults.

# ─── core ──────────────────────────────────────────────────────────────────
# Default provider when no CLI flag, env var, or qsh.toml says otherwise.
# provider = "claude"          # gemini | openai | claude | ollama

# Default mode. "fast" = cheap/low-latency; "smart" = larger budget + reasoning.
# mode = "fast"

# ─── providers ─────────────────────────────────────────────────────────────
# Each provider block is independent. `api_key` falls back to the matching
# env var (e.g. $ANTHROPIC_API_KEY). `model` falls back to the matching
# *_MODEL env var, then the built-in default for that provider.
#
# Use `[providers.<p>.models]` to pick a different model per mode. When set,
# `models.fast` / `models.smart` override the bare `model` field; the bare
# `model` is then just the fallback if either per-mode value is missing.

# [providers.gemini]
# api_key = "..."
# model   = "gemini-3.5-flash"
# [providers.gemini.models]
# fast  = "gemini-3.5-flash"
# smart = "gemini-3.5-pro"
# [providers.gemini.tokens]
# fast  = 1000                 # max output tokens in fast mode
# smart = 16000                # max output tokens in smart mode

# [providers.openai]
# api_key = "..."
# backend = "api"  # or "cli" to use Codex CLI
# model   = "gpt-5.4-mini"
# [providers.openai.models]
# fast  = "gpt-5.4-mini"
# smart = "gpt-5.4"
# [providers.openai.tokens]
# fast  = 1000
# smart = 16000

# [providers.claude]
# api_key = "..."
# backend = "api"  # or "cli" to use Claude Code CLI
# model   = "claude-sonnet-4-6"
# [providers.claude.models]
# fast  = "claude-haiku-4-5"
# smart = "claude-opus-4-7"
# [providers.claude.tokens]
# fast            = 1000
# smart           = 10000      # Claude's smart-mode max_tokens
# thinking_budget = 5000       # Claude-only: extended-thinking budget

# [providers.ollama]
# model    = "llama3"
# base_url = "http://127.0.0.1:11434"
# [providers.ollama.models]
# fast  = "llama3"
# smart = "llama3:70b"
# [providers.ollama.tokens]
# fast  = 1000
# smart = 16000

# ─── retry replay ──────────────────────────────────────────────────────────
# The shell wrapper feeds failed-command stderr back to the model so it can
# self-correct on the next `?` invocation.
# [retry]
# keep           = 3           # how many recent failures to keep in history
# window_minutes = 10          # drop attempts older than this on next run

# ─── stderr capture ────────────────────────────────────────────────────────
# Maximum bytes of stderr stored per failed attempt.
# [capture]
# stderr_bytes = 4096

# ─── network timeouts (seconds) ────────────────────────────────────────────
# [timeouts]
# connect_secs = 10
# fast_secs    = 60
# smart_secs   = 180
"#;

const PROVIDERS: [Provider; 4] = [
    Provider::Gemini,
    Provider::Openai,
    Provider::Claude,
    Provider::Ollama,
];

pub fn show(settings: &Settings) -> i32 {
    let path = settings::config_file();
    let exists = path.exists();

    println!("qsh config (effective):");
    println!(
        "  path: {}{}",
        path.display(),
        if exists { "" } else { " (missing)" }
    );
    println!();

    let (val, src) = top_provider(settings);
    println!("  provider: {:<10} [{}]", val, src);

    let (val, src) = top_mode(settings);
    println!("  mode:     {:<10} [{}]", val, src);

    println!();
    println!("  providers:");
    for p in PROVIDERS {
        println!("    {}:", p.as_str());

        let (key_val, key_src) = api_key_source(settings, p);
        println!("      api_key:         {:<24} [{}]", key_val, key_src);

        let (m_val, m_src) = model_default_source(settings, p);
        println!("      model:           {:<24} [{}]", m_val, m_src);

        let (mf_val, mf_src) = model_mode_source(settings, p, Mode::Fast);
        println!("      models.fast:     {:<24} [{}]", mf_val, mf_src);

        let (ms_val, ms_src) = model_mode_source(settings, p, Mode::Smart);
        println!("      models.smart:    {:<24} [{}]", ms_val, ms_src);

        let (b_val, b_src) = backend_source(settings, p);
        println!("      backend:         {:<24} [{}]", b_val, b_src);

        if p == Provider::Ollama {
            let (u_val, u_src) = ollama_url_source(settings);
            println!("      base_url:        {:<24} [{}]", u_val, u_src);
        }

        let (tf, tf_src) = tokens_fast_source(settings, p);
        println!("      tokens.fast:     {:<24} [{}]", tf, tf_src);
        let (ts, ts_src) = tokens_smart_source(settings, p);
        println!("      tokens.smart:    {:<24} [{}]", ts, ts_src);
        if p == Provider::Claude {
            let (tb, tb_src) = claude_thinking_source(settings);
            println!("      thinking_budget: {:<24} [{}]", tb, tb_src);
        }
    }

    println!();
    println!("  retry:");
    let (rk, rk_src) = num_source_usize(
        settings.retry_keep(),
        settings.retry.as_ref().and_then(|r| r.keep),
    );
    println!("    keep:               {:<24} [{}]", rk, rk_src);
    let (rw, rw_src) = num_source_u64(
        settings.retry_window_min(),
        settings.retry.as_ref().and_then(|r| r.window_minutes),
    );
    println!("    window_minutes:     {:<24} [{}]", rw, rw_src);

    println!();
    println!("  capture:");
    let (cs, cs_src) = num_source_usize(
        settings.stderr_cap(),
        settings.capture.as_ref().and_then(|c| c.stderr_bytes),
    );
    println!("    stderr_bytes:       {:<24} [{}]", cs, cs_src);

    println!();
    println!("  timeouts (seconds):");
    let (tc, tc_src) = num_source_u64(
        settings.timeout_connect(),
        settings.timeouts.as_ref().and_then(|t| t.connect_secs),
    );
    println!("    connect:            {:<24} [{}]", tc, tc_src);
    let (tf, tf_src) = num_source_u64(
        settings.timeout_fast(),
        settings.timeouts.as_ref().and_then(|t| t.fast_secs),
    );
    println!("    fast:               {:<24} [{}]", tf, tf_src);
    let (ts2, ts2_src) = num_source_u64(
        settings.timeout_smart(),
        settings.timeouts.as_ref().and_then(|t| t.smart_secs),
    );
    println!("    smart:              {:<24} [{}]", ts2, ts2_src);
    0
}

fn tokens_fast_source(settings: &Settings, p: Provider) -> (String, String) {
    let val = settings.tokens_fast(p);
    let src = if settings
        .providers
        .get(p.as_str())
        .and_then(|s| s.tokens.as_ref())
        .and_then(|t| t.fast)
        .is_some()
    {
        "config.toml"
    } else {
        "default"
    };
    (val.to_string(), src.into())
}

fn tokens_smart_source(settings: &Settings, p: Provider) -> (String, String) {
    let val = settings.tokens_smart(p);
    let src = if settings
        .providers
        .get(p.as_str())
        .and_then(|s| s.tokens.as_ref())
        .and_then(|t| t.smart)
        .is_some()
    {
        "config.toml"
    } else {
        "default"
    };
    (val.to_string(), src.into())
}

fn claude_thinking_source(settings: &Settings) -> (String, String) {
    let val = settings.claude_thinking_budget();
    let src = if settings
        .providers
        .get(Provider::Claude.as_str())
        .and_then(|s| s.tokens.as_ref())
        .and_then(|t| t.thinking_budget)
        .is_some()
    {
        "config.toml"
    } else {
        "default"
    };
    (val.to_string(), src.into())
}

fn num_source_usize(effective: usize, set: Option<usize>) -> (String, String) {
    (
        effective.to_string(),
        if set.is_some() {
            "config.toml".into()
        } else {
            "default".into()
        },
    )
}

fn num_source_u64(effective: u64, set: Option<u64>) -> (String, String) {
    (
        effective.to_string(),
        if set.is_some() {
            "config.toml".into()
        } else {
            "default".into()
        },
    )
}

fn top_provider(settings: &Settings) -> (String, String) {
    if let Ok(v) = env::var("QSH_PROVIDER")
        && !v.is_empty()
    {
        return (v, "env QSH_PROVIDER".into());
    }
    if let Some(v) = settings.provider.as_deref() {
        return (v.into(), "config.toml".into());
    }
    ("(auto-detect)".into(), "default".into())
}

fn top_mode(settings: &Settings) -> (String, String) {
    if let Some(v) = settings.mode.as_deref() {
        return (v.into(), "config.toml".into());
    }
    if let Ok(v) = env::var("QSH_MODE")
        && !v.is_empty()
    {
        return (v, "env QSH_MODE".into());
    }
    ("fast".into(), "default".into())
}

fn api_key_source(settings: &Settings, p: Provider) -> (String, String) {
    if let Some(k) = settings.api_key(p) {
        return (redact(k), "config.toml".into());
    }
    if let Some(env_var) = provider_helpers::api_key_env(p)
        && let Ok(v) = env::var(env_var)
        && !v.is_empty()
    {
        return (redact(&v), format!("env {}", env_var));
    }
    if provider_helpers::api_key_env(p).is_none() {
        return ("(n/a)".into(), "no auth required".into());
    }
    ("(not set)".into(), "missing".into())
}

fn model_default_source(settings: &Settings, p: Provider) -> (String, String) {
    if let Some(m) = settings.model_default(p) {
        return (m.into(), "config.toml".into());
    }
    let env_var = provider_helpers::model_env(p);
    if let Ok(v) = env::var(env_var)
        && !v.is_empty()
    {
        return (v, format!("env {}", env_var));
    }
    if let Some(d) = provider_helpers::default_model(p) {
        return (d.into(), "default".into());
    }
    ("(auto-detect)".into(), "ollama list".into())
}

fn model_mode_source(settings: &Settings, p: Provider, mode: Mode) -> (String, String) {
    if let Some(m) = settings.model_mode(p, mode) {
        return (m.into(), "config.toml".into());
    }
    if let Some(m) = settings.model_default(p) {
        return (m.into(), "config.toml (fallback)".into());
    }
    let env_var = provider_helpers::model_env(p);
    if let Ok(v) = env::var(env_var)
        && !v.is_empty()
    {
        return (v, format!("env {}", env_var));
    }
    if let Some(d) = provider_helpers::default_model(p) {
        return (d.into(), "default".into());
    }
    ("(auto-detect)".into(), "ollama list".into())
}

fn backend_source(settings: &Settings, p: Provider) -> (String, String) {
    if let Ok(v) = env::var("QSH_BACKEND")
        && let Some(b) = Backend::parse(&v)
        && Backend::supports(p, b)
    {
        return (b.as_str().into(), "env QSH_BACKEND".into());
    }
    if let Some(b) = settings.backend(p)
        && Backend::supports(p, b)
    {
        return (b.as_str().into(), "config.toml".into());
    }
    ("api".into(), "default".into())
}

fn ollama_url_source(settings: &Settings) -> (String, String) {
    if let Some(u) = settings.ollama_base_url() {
        return (u.into(), "config.toml".into());
    }
    if let Ok(v) = env::var("OLLAMA_BASE_URL")
        && !v.is_empty()
    {
        return (v, "env OLLAMA_BASE_URL".into());
    }
    if let Ok(v) = env::var("OLLAMA_HOST")
        && !v.is_empty()
    {
        return (v, "env OLLAMA_HOST".into());
    }
    ("http://127.0.0.1:11434".into(), "default".into())
}

fn redact(s: &str) -> String {
    if s.len() >= 8 {
        format!("{}...{}", &s[..3], &s[s.len() - 3..])
    } else {
        "***".into()
    }
}

pub fn edit() -> i32 {
    let path = settings::config_file();
    if !path.exists() {
        let Some(parent) = path.parent() else {
            eprintln!("qsh: config path has no parent: {}", path.display());
            return 1;
        };
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("qsh: failed to create {}: {}", parent.display(), e);
            return 1;
        }
        if let Err(e) = fs::write(&path, TEMPLATE) {
            eprintln!("qsh: failed to write {}: {}", path.display(), e);
            return 1;
        }
        chmod_600(&path);
    }

    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".into());

    let status = Command::new(&editor).arg(&path).status();
    eprintln!("saved: {}", path.display());
    match status {
        Ok(s) if s.success() => 0,
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("qsh: failed to launch {}: {}", editor, e);
            1
        }
    }
}

pub fn set(args: ConfigSetArgs) -> i32 {
    let key = match parse_key(&args.key) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("qsh: {}", e);
            eprintln!();
            print_allowed_keys();
            return 2;
        }
    };

    let value = match resolve_value(&key, args.value) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("qsh: {}", e);
            return 2;
        }
    };

    if let Err(e) = validate_value(&key, &value) {
        eprintln!("qsh: {}", e);
        return 2;
    }

    let path = settings::config_file();
    // Seed the file with the commented template on first write so users
    // can see every option without running `qsh config edit`.
    let doc_src = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => TEMPLATE.to_string(),
    };
    let mut doc: toml_edit::DocumentMut = match doc_src.parse() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("qsh: failed to parse {}: {}", path.display(), e);
            return 1;
        }
    };

    apply(&mut doc, &key, &value);

    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("qsh: failed to create {}: {}", parent.display(), e);
        return 1;
    }
    if let Err(e) = cache::save_atomic(&path, doc.to_string().as_bytes()) {
        eprintln!("qsh: failed to write {}: {}", path.display(), e);
        return 1;
    }
    chmod_600(&path);
    eprintln!("saved: {}", path.display());
    0
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum Key {
    Provider,
    Mode,
    ApiKey(Provider),
    Backend(Provider),
    Model(Provider),
    ModelFast(Provider),
    ModelSmart(Provider),
    OllamaBaseUrl,
    TokensFast(Provider),
    TokensSmart(Provider),
    ClaudeThinkingBudget,
    RetryKeep,
    RetryWindowMinutes,
    CaptureStderrBytes,
    TimeoutConnect,
    TimeoutFast,
    TimeoutSmart,
}

fn parse_key(s: &str) -> Result<Key, String> {
    match s {
        "provider" => return Ok(Key::Provider),
        "mode" => return Ok(Key::Mode),
        "retry.keep" => return Ok(Key::RetryKeep),
        "retry.window_minutes" => return Ok(Key::RetryWindowMinutes),
        "capture.stderr_bytes" => return Ok(Key::CaptureStderrBytes),
        "timeouts.connect_secs" => return Ok(Key::TimeoutConnect),
        "timeouts.fast_secs" => return Ok(Key::TimeoutFast),
        "timeouts.smart_secs" => return Ok(Key::TimeoutSmart),
        _ => {}
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() == 3 && parts[0] == "providers" {
        let p =
            Provider::parse(parts[1]).ok_or_else(|| format!("unknown provider: {}", parts[1]))?;
        match parts[2] {
            "api_key" => return Ok(Key::ApiKey(p)),
            "backend" if p == Provider::Claude || p == Provider::Openai => {
                return Ok(Key::Backend(p));
            }
            "backend" => {
                return Err(format!(
                    "backend is only supported for claude and openai, not {}",
                    p.as_str()
                ));
            }
            "model" => return Ok(Key::Model(p)),
            "base_url" if p == Provider::Ollama => return Ok(Key::OllamaBaseUrl),
            "base_url" => {
                return Err(format!(
                    "base_url is only supported for ollama, not {}",
                    p.as_str()
                ));
            }
            _ => return Err(format!("unknown leaf key: {}", parts[2])),
        }
    }
    if parts.len() == 4 && parts[0] == "providers" && parts[2] == "tokens" {
        let p =
            Provider::parse(parts[1]).ok_or_else(|| format!("unknown provider: {}", parts[1]))?;
        match parts[3] {
            "fast" => return Ok(Key::TokensFast(p)),
            "smart" => return Ok(Key::TokensSmart(p)),
            "thinking_budget" if p == Provider::Claude => return Ok(Key::ClaudeThinkingBudget),
            "thinking_budget" => {
                return Err("thinking_budget is only supported for claude".into());
            }
            _ => return Err(format!("unknown tokens key: {}", parts[3])),
        }
    }
    if parts.len() == 4 && parts[0] == "providers" && parts[2] == "models" {
        let p =
            Provider::parse(parts[1]).ok_or_else(|| format!("unknown provider: {}", parts[1]))?;
        match parts[3] {
            "fast" => return Ok(Key::ModelFast(p)),
            "smart" => return Ok(Key::ModelSmart(p)),
            _ => return Err(format!("unknown models key: {}", parts[3])),
        }
    }
    Err(format!("unknown key: {}", s))
}

fn print_allowed_keys() {
    eprintln!("allowed keys:");
    eprintln!("  provider                              (gemini|openai|claude|ollama)");
    eprintln!("  mode                                  (fast|smart)");
    for p in PROVIDERS {
        eprintln!("  providers.{}.api_key", p.as_str());
        eprintln!("  providers.{}.model", p.as_str());
        eprintln!("  providers.{}.models.fast", p.as_str());
        eprintln!("  providers.{}.models.smart", p.as_str());
        eprintln!(
            "  providers.{}.tokens.fast            (positive integer)",
            p.as_str()
        );
        eprintln!(
            "  providers.{}.tokens.smart           (positive integer)",
            p.as_str()
        );
    }
    eprintln!("  providers.claude.backend             (api|cli)");
    eprintln!("  providers.openai.backend             (api|cli)");
    eprintln!("  providers.ollama.base_url");
    eprintln!("  providers.claude.tokens.thinking_budget (positive integer)");
    eprintln!("  retry.keep                            (positive integer)");
    eprintln!("  retry.window_minutes                  (positive integer)");
    eprintln!("  capture.stderr_bytes                  (positive integer)");
    eprintln!("  timeouts.connect_secs                 (positive integer)");
    eprintln!("  timeouts.fast_secs                    (positive integer)");
    eprintln!("  timeouts.smart_secs                   (positive integer)");
}

fn resolve_value(key: &Key, given: Option<String>) -> Result<String, String> {
    if let Some(v) = given {
        return Ok(v);
    }
    if matches!(key, Key::ApiKey(_)) {
        if atty::is(atty::Stream::Stdin) {
            return Err(
                "pass the value as an argument or pipe it via stdin (e.g. `echo $KEY | qsh config set ...`)"
                    .into(),
            );
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("failed to read stdin: {}", e))?;
        return Ok(buf.trim().to_string());
    }
    Err("missing value".into())
}

fn validate_value(key: &Key, value: &str) -> Result<(), String> {
    match key {
        Key::Provider => {
            Provider::parse(value).ok_or_else(|| {
                format!(
                    "invalid provider: {} (expected gemini|openai|claude|ollama)",
                    value
                )
            })?;
        }
        Key::Mode => match value {
            "fast" | "smart" => {}
            _ => return Err(format!("invalid mode: {} (expected fast|smart)", value)),
        },
        Key::ApiKey(_)
        | Key::Model(_)
        | Key::ModelFast(_)
        | Key::ModelSmart(_)
        | Key::OllamaBaseUrl => {
            if value.is_empty() {
                return Err("value cannot be empty".into());
            }
        }
        Key::Backend(p) => {
            if *p != Provider::Claude && *p != Provider::Openai {
                return Err(format!(
                    "backend is only supported for claude and openai, not {}",
                    p.as_str()
                ));
            }
            let b = Backend::parse(value)
                .ok_or_else(|| format!("invalid backend: {} (expected api|cli)", value))?;
            if !Backend::supports(*p, b) {
                return Err(format!(
                    "backend {} is not supported for {}",
                    b.as_str(),
                    p.as_str()
                ));
            }
        }
        Key::TokensFast(_)
        | Key::TokensSmart(_)
        | Key::ClaudeThinkingBudget
        | Key::RetryKeep
        | Key::RetryWindowMinutes
        | Key::CaptureStderrBytes
        | Key::TimeoutConnect
        | Key::TimeoutFast
        | Key::TimeoutSmart => {
            let n: u64 = value
                .parse()
                .map_err(|_| format!("expected a positive integer, got: {}", value))?;
            if n == 0 {
                return Err("value must be greater than 0".into());
            }
        }
    }
    Ok(())
}

fn apply(doc: &mut toml_edit::DocumentMut, key: &Key, value: &str) {
    use toml_edit::{Item, Table, value as tv};
    let int = |s: &str| -> i64 { s.parse::<i64>().expect("validated earlier") };

    let provider_leaf = |doc: &mut toml_edit::DocumentMut, p: Provider, leaf: &str, v: Item| {
        let providers = doc
            .entry("providers")
            .or_insert_with(|| Item::Table(Table::new()))
            .as_table_mut()
            .expect("providers is a table");
        providers.set_implicit(true);
        let sub = providers
            .entry(p.as_str())
            .or_insert_with(|| Item::Table(Table::new()))
            .as_table_mut()
            .expect("provider entry is a table");
        sub[leaf] = v;
    };

    let provider_sub_leaf =
        |doc: &mut toml_edit::DocumentMut, p: Provider, sub_table: &str, leaf: &str, v: Item| {
            let providers = doc
                .entry("providers")
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_mut()
                .expect("providers is a table");
            providers.set_implicit(true);
            let sub = providers
                .entry(p.as_str())
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_mut()
                .expect("provider entry is a table");
            sub.set_implicit(true);
            let nested = sub
                .entry(sub_table)
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_mut()
                .expect("nested entry is a table");
            nested[leaf] = v;
        };

    let section_leaf = |doc: &mut toml_edit::DocumentMut, section: &str, leaf: &str, v: Item| {
        let sec = doc
            .entry(section)
            .or_insert_with(|| Item::Table(Table::new()))
            .as_table_mut()
            .expect("section is a table");
        sec[leaf] = v;
    };

    match key {
        Key::Provider => doc["provider"] = tv(value),
        Key::Mode => doc["mode"] = tv(value),
        Key::ApiKey(p) => provider_leaf(doc, *p, "api_key", tv(value)),
        Key::Backend(p) => provider_leaf(doc, *p, "backend", tv(value)),
        Key::Model(p) => provider_leaf(doc, *p, "model", tv(value)),
        Key::OllamaBaseUrl => provider_leaf(doc, Provider::Ollama, "base_url", tv(value)),
        Key::TokensFast(p) => provider_sub_leaf(doc, *p, "tokens", "fast", tv(int(value))),
        Key::TokensSmart(p) => provider_sub_leaf(doc, *p, "tokens", "smart", tv(int(value))),
        Key::ClaudeThinkingBudget => provider_sub_leaf(
            doc,
            Provider::Claude,
            "tokens",
            "thinking_budget",
            tv(int(value)),
        ),
        Key::ModelFast(p) => provider_sub_leaf(doc, *p, "models", "fast", tv(value)),
        Key::ModelSmart(p) => provider_sub_leaf(doc, *p, "models", "smart", tv(value)),
        Key::RetryKeep => section_leaf(doc, "retry", "keep", tv(int(value))),
        Key::RetryWindowMinutes => section_leaf(doc, "retry", "window_minutes", tv(int(value))),
        Key::CaptureStderrBytes => section_leaf(doc, "capture", "stderr_bytes", tv(int(value))),
        Key::TimeoutConnect => section_leaf(doc, "timeouts", "connect_secs", tv(int(value))),
        Key::TimeoutFast => section_leaf(doc, "timeouts", "fast_secs", tv(int(value))),
        Key::TimeoutSmart => section_leaf(doc, "timeouts", "smart_secs", tv(int(value))),
    }
}

fn chmod_600(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_top_level_keys() {
        assert_eq!(parse_key("provider").unwrap(), Key::Provider);
        assert_eq!(parse_key("mode").unwrap(), Key::Mode);
    }

    #[test]
    fn parse_provider_keys() {
        assert_eq!(
            parse_key("providers.claude.api_key").unwrap(),
            Key::ApiKey(Provider::Claude)
        );
        assert_eq!(
            parse_key("providers.openai.model").unwrap(),
            Key::Model(Provider::Openai)
        );
        assert_eq!(
            parse_key("providers.openai.backend").unwrap(),
            Key::Backend(Provider::Openai)
        );
        assert_eq!(
            parse_key("providers.ollama.base_url").unwrap(),
            Key::OllamaBaseUrl
        );
    }

    #[test]
    fn parse_rejects_base_url_on_non_ollama() {
        assert!(parse_key("providers.claude.base_url").is_err());
    }

    #[test]
    fn parse_rejects_backend_on_unsupported_provider() {
        assert!(parse_key("providers.gemini.backend").is_err());
        assert!(parse_key("providers.ollama.backend").is_err());
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(parse_key("foo").is_err());
        assert!(parse_key("providers.foo.api_key").is_err());
        assert!(parse_key("providers.claude.unknown").is_err());
    }

    #[test]
    fn validate_provider() {
        assert!(validate_value(&Key::Provider, "claude").is_ok());
        assert!(validate_value(&Key::Provider, "bogus").is_err());
    }

    #[test]
    fn validate_mode() {
        assert!(validate_value(&Key::Mode, "fast").is_ok());
        assert!(validate_value(&Key::Mode, "smart").is_ok());
        assert!(validate_value(&Key::Mode, "warp").is_err());
    }

    #[test]
    fn validate_backend() {
        assert!(validate_value(&Key::Backend(Provider::Claude), "cli").is_ok());
        assert!(validate_value(&Key::Backend(Provider::Openai), "api").is_ok());
        assert!(validate_value(&Key::Backend(Provider::Claude), "bogus").is_err());
        assert!(validate_value(&Key::Backend(Provider::Gemini), "cli").is_err());
    }

    #[test]
    fn redact_short_and_long() {
        assert_eq!(redact("abc"), "***");
        assert_eq!(redact("abcdefgh"), "abc...fgh");
        assert_eq!(redact("sk-ant-1234567890xyz"), "sk-...xyz");
    }

    #[test]
    fn apply_writes_top_and_nested_keys() {
        let mut doc: toml_edit::DocumentMut = "".parse().unwrap();
        apply(&mut doc, &Key::Provider, "claude");
        apply(&mut doc, &Key::Mode, "smart");
        apply(&mut doc, &Key::ApiKey(Provider::Claude), "sk-ant-test");
        apply(&mut doc, &Key::Backend(Provider::Claude), "cli");
        apply(&mut doc, &Key::Model(Provider::Openai), "gpt-foo");
        apply(&mut doc, &Key::OllamaBaseUrl, "http://localhost:11434");

        let parsed: Settings = toml::from_str(&doc.to_string()).unwrap();
        assert_eq!(parsed.provider.as_deref(), Some("claude"));
        assert_eq!(parsed.mode.as_deref(), Some("smart"));
        assert_eq!(parsed.api_key(Provider::Claude), Some("sk-ant-test"));
        assert_eq!(parsed.backend(Provider::Claude), Some(Backend::Cli));
        assert_eq!(parsed.model_default(Provider::Openai), Some("gpt-foo"));
        assert_eq!(parsed.ollama_base_url(), Some("http://localhost:11434"));
    }

    #[test]
    fn apply_preserves_comments() {
        let original = "# top comment\nprovider = \"claude\"\n\n[providers.claude]\n# inner\nmodel = \"old\"\n";
        let mut doc: toml_edit::DocumentMut = original.parse().unwrap();
        apply(&mut doc, &Key::Model(Provider::Claude), "new");
        let out = doc.to_string();
        assert!(out.contains("# top comment"));
        assert!(out.contains("# inner"));
        assert!(out.contains("\"new\""));
        assert!(!out.contains("\"old\""));
    }

    #[test]
    fn parses_numeric_keys() {
        assert_eq!(
            parse_key("providers.claude.tokens.fast").unwrap(),
            Key::TokensFast(Provider::Claude)
        );
        assert_eq!(
            parse_key("providers.openai.tokens.smart").unwrap(),
            Key::TokensSmart(Provider::Openai)
        );
        assert_eq!(
            parse_key("providers.claude.tokens.thinking_budget").unwrap(),
            Key::ClaudeThinkingBudget
        );
        assert_eq!(parse_key("retry.keep").unwrap(), Key::RetryKeep);
        assert_eq!(
            parse_key("retry.window_minutes").unwrap(),
            Key::RetryWindowMinutes
        );
        assert_eq!(
            parse_key("capture.stderr_bytes").unwrap(),
            Key::CaptureStderrBytes
        );
        assert_eq!(
            parse_key("timeouts.connect_secs").unwrap(),
            Key::TimeoutConnect
        );
    }

    #[test]
    fn rejects_thinking_budget_on_non_claude() {
        assert!(parse_key("providers.openai.tokens.thinking_budget").is_err());
    }

    #[test]
    fn parse_per_mode_model_keys() {
        assert_eq!(
            parse_key("providers.claude.models.fast").unwrap(),
            Key::ModelFast(Provider::Claude)
        );
        assert_eq!(
            parse_key("providers.openai.models.smart").unwrap(),
            Key::ModelSmart(Provider::Openai)
        );
        assert!(parse_key("providers.claude.models.weird").is_err());
    }

    #[test]
    fn apply_writes_per_mode_models() {
        let mut doc: toml_edit::DocumentMut = "".parse().unwrap();
        apply(
            &mut doc,
            &Key::ModelFast(Provider::Claude),
            "claude-haiku-4-5",
        );
        apply(
            &mut doc,
            &Key::ModelSmart(Provider::Claude),
            "claude-opus-4-7",
        );
        let parsed: Settings = toml::from_str(&doc.to_string()).unwrap();
        assert_eq!(
            parsed.model(Provider::Claude, Mode::Fast),
            Some("claude-haiku-4-5")
        );
        assert_eq!(
            parsed.model(Provider::Claude, Mode::Smart),
            Some("claude-opus-4-7")
        );
    }

    #[test]
    fn validate_numeric_keys() {
        assert!(validate_value(&Key::RetryKeep, "5").is_ok());
        assert!(validate_value(&Key::RetryKeep, "0").is_err());
        assert!(validate_value(&Key::RetryKeep, "-1").is_err());
        assert!(validate_value(&Key::RetryKeep, "foo").is_err());
        assert!(validate_value(&Key::TokensFast(Provider::Openai), "1000").is_ok());
    }

    #[test]
    fn apply_writes_numeric_keys() {
        let mut doc: toml_edit::DocumentMut = "".parse().unwrap();
        apply(&mut doc, &Key::TokensFast(Provider::Claude), "750");
        apply(&mut doc, &Key::TokensSmart(Provider::Claude), "12000");
        apply(&mut doc, &Key::ClaudeThinkingBudget, "6000");
        apply(&mut doc, &Key::RetryKeep, "5");
        apply(&mut doc, &Key::RetryWindowMinutes, "30");
        apply(&mut doc, &Key::CaptureStderrBytes, "8192");
        apply(&mut doc, &Key::TimeoutConnect, "15");
        apply(&mut doc, &Key::TimeoutFast, "90");
        apply(&mut doc, &Key::TimeoutSmart, "240");

        let parsed: Settings = toml::from_str(&doc.to_string()).unwrap();
        assert_eq!(parsed.tokens_fast(Provider::Claude), 750);
        assert_eq!(parsed.claude_smart_max(), 12000);
        assert_eq!(parsed.claude_thinking_budget(), 6000);
        assert_eq!(parsed.retry_keep(), 5);
        assert_eq!(parsed.retry_window_min(), 30);
        assert_eq!(parsed.stderr_cap(), 8192);
        assert_eq!(parsed.timeout_connect(), 15);
        assert_eq!(parsed.timeout_fast(), 90);
        assert_eq!(parsed.timeout_smart(), 240);
    }
}
