use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "qsh",
    version,
    about = "AI shell-command generator",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Generate a shell command from a natural-language description.
    Generate(GenerateArgs),
    /// Record an exec attempt for retry replay.
    Record(RecordArgs),
    /// Print shell init script (function definition + alias).
    Init(InitArgs),
    /// Manage the ~/.qsh_known list of detected tools.
    Known(KnownArgs),
    /// Manage the global qsh config at ~/.config/qsh/config.toml.
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print the effective config (TOML + env + defaults), with API keys redacted.
    Show,
    /// Open the config file in $EDITOR (creates a template if missing).
    Edit,
    /// Set a config value (e.g. `qsh config set providers.claude.api_key sk-ant-...`).
    Set(ConfigSetArgs),
}

#[derive(Args, Debug)]
pub struct ConfigSetArgs {
    /// Dotted key path. Allowed:
    ///   provider, mode,
    ///   providers.<gemini|openai|claude|ollama>.api_key,
    ///   providers.<gemini|openai|claude|ollama>.model,
    ///   providers.ollama.base_url
    pub key: String,
    /// New value. Omit and pipe via stdin for API keys.
    pub value: Option<String>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
}

impl Shell {
    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Fish => "fish",
            Shell::Zsh => "zsh",
        }
    }
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Target shell to print the wrapper and completion script for.
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Args, Debug)]
pub struct KnownArgs {
    /// Re-scan and overwrite ~/.qsh_known.
    #[arg(short = 'r', long = "refresh")]
    pub refresh: bool,
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Wrapper shell context (set by `qsh init`; affects exec/history semantics).
    #[arg(long, value_enum, default_value = "zsh")]
    pub shell: Shell,

    /// Use Google Gemini.
    #[arg(short = 'g', long = "gemini", visible_alias = "google")]
    pub gemini: bool,
    /// Use OpenAI (GPT).
    #[arg(short = 'o', long = "openai", visible_alias = "gpt")]
    pub openai: bool,
    /// Use Anthropic Claude.
    #[arg(short = 'c', long = "claude")]
    pub claude: bool,
    /// Use a local Ollama model.
    #[arg(short = 'l', long = "ollama", visible_aliases = ["local"])]
    pub ollama: bool,
    /// Provider name (gemini|openai|claude|ollama).
    #[arg(short = 'p', long = "provider")]
    pub provider: Option<String>,

    /// Override the model id for the selected provider.
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    /// Use a smarter (slower, larger-budget) model.
    #[arg(short = 's', long = "smart")]
    pub smart: bool,
    /// Force fast mode (cheap, low-latency).
    #[arg(short = 'f', long = "fast")]
    pub fast: bool,

    /// Skip the response cache for this run.
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    /// Delete the on-disk cache directory and exit.
    #[arg(long = "clear-cache")]
    pub clear_cache: bool,

    /// Omit cwd context (git branch, lang manifests, build tools).
    #[arg(long = "no-context")]
    pub no_context: bool,

    /// Stream N (1-8) alternative candidates and pick one via fzf.
    #[arg(short = 'a', long = "alts")]
    pub alts: Option<u32>,

    /// Print a one-line explanation alongside the command.
    #[arg(short = 'e', long = "explain")]
    pub explain: bool,
    /// Dump resolved provider/model/request body to stderr.
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Print full long-form help.
    #[arg(long = "full-help")]
    pub full_help: bool,

    /// Positional task words and ./file references.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub task: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RecordArgs {
    /// The command that was executed.
    #[arg(long)]
    pub cmd: String,
    /// Exit status from eval.
    #[arg(long)]
    pub status: i32,
    /// Path to captured stderr from the executed command.
    #[arg(long = "stderr-file")]
    pub stderr_file: Option<String>,
    /// Original-intent task (passed through from generate).
    #[arg(long = "original-task", default_value = "")]
    pub original_task: String,
}
