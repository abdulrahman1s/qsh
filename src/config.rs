pub const STDIN_CAP: usize = 32_768;
pub const FILE_CAP: usize = 32_768;
pub const ATTEMPTS_KEEP: usize = 3;
pub const RETRY_WINDOW_MIN: u64 = 10;
pub const ALTS_MIN: u32 = 1;
pub const ALTS_MAX: u32 = 8;
pub const TOKENS_FAST: u32 = 1000;
pub const TOKENS_SMART: u32 = 16_000;
pub const TOKENS_EXPLAIN_BONUS: u32 = 200;
pub const TOKENS_PER_ALT: u32 = 800;
pub const CLAUDE_SMART_MAX: u32 = 10_000;
pub const CLAUDE_SMART_BUDGET: u32 = 5_000;
pub const CURL_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const CURL_TIMEOUT_FAST_SECS: u64 = 60;
pub const CURL_TIMEOUT_SMART_SECS: u64 = 180;
pub const STDERR_CAP: usize = 4_096;
pub const INDICATOR_MAX: usize = 80;
pub const SPINNER_FRAMES: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
pub const SPINNER_SLEEP_MS: u64 = 80;
pub const TYPEWRITER_CHAR_MS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Fast,
    Smart,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Fast => "fast",
            Mode::Smart => "smart",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Gemini,
    Openai,
    Claude,
    Ollama,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Gemini => "gemini",
            Provider::Openai => "openai",
            Provider::Claude => "claude",
            Provider::Ollama => "ollama",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gemini" | "google" => Some(Provider::Gemini),
            "openai" | "chatgpt" | "gpt" => Some(Provider::Openai),
            "claude" | "anthropic" => Some(Provider::Claude),
            "ollama" | "local" => Some(Provider::Ollama),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Api,
    Cli,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Api => "api",
            Backend::Cli => "cli",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "api" | "http" => Some(Backend::Api),
            "cli" | "command" => Some(Backend::Cli),
            _ => None,
        }
    }

    pub fn supports(p: Provider, b: Backend) -> bool {
        matches!(
            (p, b),
            (_, Backend::Api) | (Provider::Claude, Backend::Cli) | (Provider::Openai, Backend::Cli)
        )
    }
}
