use crate::config::{
    ATTEMPTS_KEEP, CLAUDE_SMART_BUDGET, CLAUDE_SMART_MAX, CURL_CONNECT_TIMEOUT_SECS,
    CURL_TIMEOUT_FAST_SECS, CURL_TIMEOUT_SMART_SECS, Provider, RETRY_WINDOW_MIN, STDERR_CAP,
    TOKENS_FAST, TOKENS_SMART,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub provider: Option<String>,
    pub mode: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderSettings>,
    pub retry: Option<RetrySettings>,
    pub capture: Option<CaptureSettings>,
    pub timeouts: Option<TimeoutSettings>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub tokens: Option<TokenSettings>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TokenSettings {
    pub fast: Option<u32>,
    pub smart: Option<u32>,
    /// Claude-only: extended-thinking budget. Ignored by other providers.
    pub thinking_budget: Option<u32>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RetrySettings {
    pub keep: Option<usize>,
    pub window_minutes: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    pub stderr_bytes: Option<usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TimeoutSettings {
    pub connect_secs: Option<u64>,
    pub fast_secs: Option<u64>,
    pub smart_secs: Option<u64>,
}

pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("qsh");
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".config").join("qsh")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load() -> Settings {
    let path = config_file();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    match toml::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("qsh: {} parse error: {}", path.display(), e);
            Settings::default()
        }
    }
}

impl Settings {
    pub fn api_key(&self, p: Provider) -> Option<&str> {
        self.providers
            .get(p.as_str())
            .and_then(|s| s.api_key.as_deref())
            .filter(|s| !s.is_empty())
    }

    pub fn model(&self, p: Provider) -> Option<&str> {
        self.providers
            .get(p.as_str())
            .and_then(|s| s.model.as_deref())
            .filter(|s| !s.is_empty())
    }

    pub fn ollama_base_url(&self) -> Option<&str> {
        self.providers
            .get(Provider::Ollama.as_str())
            .and_then(|s| s.base_url.as_deref())
            .filter(|s| !s.is_empty())
    }

    fn tokens(&self, p: Provider) -> Option<&TokenSettings> {
        self.providers
            .get(p.as_str())
            .and_then(|s| s.tokens.as_ref())
    }

    pub fn tokens_fast(&self, p: Provider) -> u32 {
        self.tokens(p).and_then(|t| t.fast).unwrap_or(TOKENS_FAST)
    }

    pub fn tokens_smart(&self, p: Provider) -> u32 {
        self.tokens(p).and_then(|t| t.smart).unwrap_or(TOKENS_SMART)
    }

    pub fn claude_smart_max(&self) -> u32 {
        self.tokens(Provider::Claude)
            .and_then(|t| t.smart)
            .unwrap_or(CLAUDE_SMART_MAX)
    }

    pub fn claude_thinking_budget(&self) -> u32 {
        self.tokens(Provider::Claude)
            .and_then(|t| t.thinking_budget)
            .unwrap_or(CLAUDE_SMART_BUDGET)
    }

    pub fn retry_keep(&self) -> usize {
        self.retry
            .as_ref()
            .and_then(|r| r.keep)
            .unwrap_or(ATTEMPTS_KEEP)
    }

    pub fn retry_window_min(&self) -> u64 {
        self.retry
            .as_ref()
            .and_then(|r| r.window_minutes)
            .unwrap_or(RETRY_WINDOW_MIN)
    }

    pub fn stderr_cap(&self) -> usize {
        self.capture
            .as_ref()
            .and_then(|c| c.stderr_bytes)
            .unwrap_or(STDERR_CAP)
    }

    pub fn timeout_connect(&self) -> u64 {
        self.timeouts
            .as_ref()
            .and_then(|t| t.connect_secs)
            .unwrap_or(CURL_CONNECT_TIMEOUT_SECS)
    }

    pub fn timeout_fast(&self) -> u64 {
        self.timeouts
            .as_ref()
            .and_then(|t| t.fast_secs)
            .unwrap_or(CURL_TIMEOUT_FAST_SECS)
    }

    pub fn timeout_smart(&self) -> u64 {
        self.timeouts
            .as_ref()
            .and_then(|t| t.smart_secs)
            .unwrap_or(CURL_TIMEOUT_SMART_SECS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let src = r#"
provider = "claude"
mode = "smart"

[providers.claude]
api_key = "sk-ant-test"
model = "claude-sonnet-4-6"

[providers.claude.tokens]
fast = 800
smart = 20000
thinking_budget = 7000

[providers.ollama]
base_url = "http://localhost:11434"
model = "llama3"

[retry]
keep = 5
window_minutes = 30

[capture]
stderr_bytes = 8192

[timeouts]
connect_secs = 15
fast_secs = 90
smart_secs = 240
"#;
        let s: Settings = toml::from_str(src).unwrap();
        assert_eq!(s.provider.as_deref(), Some("claude"));
        assert_eq!(s.mode.as_deref(), Some("smart"));
        assert_eq!(s.api_key(Provider::Claude), Some("sk-ant-test"));
        assert_eq!(s.model(Provider::Claude), Some("claude-sonnet-4-6"));
        assert_eq!(s.ollama_base_url(), Some("http://localhost:11434"));
        assert_eq!(s.tokens_fast(Provider::Claude), 800);
        assert_eq!(s.tokens_smart(Provider::Claude), 20000);
        assert_eq!(s.claude_smart_max(), 20000);
        assert_eq!(s.claude_thinking_budget(), 7000);
        assert_eq!(s.retry_keep(), 5);
        assert_eq!(s.retry_window_min(), 30);
        assert_eq!(s.stderr_cap(), 8192);
        assert_eq!(s.timeout_connect(), 15);
        assert_eq!(s.timeout_fast(), 90);
        assert_eq!(s.timeout_smart(), 240);
    }

    #[test]
    fn parses_partial_config() {
        let src = r#"provider = "openai""#;
        let s: Settings = toml::from_str(src).unwrap();
        assert_eq!(s.provider.as_deref(), Some("openai"));
        assert!(s.mode.is_none());
        assert!(s.api_key(Provider::Openai).is_none());
        assert!(s.model(Provider::Openai).is_none());
        // Defaults come through when unset.
        assert_eq!(s.tokens_fast(Provider::Openai), TOKENS_FAST);
        assert_eq!(s.tokens_smart(Provider::Openai), TOKENS_SMART);
        assert_eq!(s.retry_keep(), ATTEMPTS_KEEP);
        assert_eq!(s.retry_window_min(), RETRY_WINDOW_MIN);
        assert_eq!(s.stderr_cap(), STDERR_CAP);
    }

    #[test]
    fn empty_strings_treated_as_unset() {
        let src = r#"
[providers.claude]
api_key = ""
model = ""
"#;
        let s: Settings = toml::from_str(src).unwrap();
        assert!(s.api_key(Provider::Claude).is_none());
        assert!(s.model(Provider::Claude).is_none());
    }

    #[test]
    fn missing_provider_section_is_none() {
        let s: Settings = toml::from_str("").unwrap();
        assert!(s.api_key(Provider::Gemini).is_none());
        assert!(s.model(Provider::Gemini).is_none());
        assert!(s.ollama_base_url().is_none());
    }
}
