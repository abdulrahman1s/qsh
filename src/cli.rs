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
    /// Force shell wrapper integration mode (default: zsh).
    #[arg(long, value_enum, default_value = "zsh")]
    pub shell: Shell,

    // Provider flags
    #[arg(short = 'g', long = "gemini", visible_alias = "google")]
    pub gemini: bool,
    #[arg(short = 'o', long = "openai", visible_alias = "gpt")]
    pub openai: bool,
    #[arg(short = 'c', long = "claude")]
    pub claude: bool,
    #[arg(short = 'l', long = "ollama", visible_aliases = ["local"])]
    pub ollama: bool,
    #[arg(short = 'p', long = "provider")]
    pub provider: Option<String>,

    // Model
    #[arg(short = 'm', long = "model")]
    pub model: Option<String>,

    // Mode
    #[arg(short = 's', long = "smart")]
    pub smart: bool,
    #[arg(short = 'f', long = "fast")]
    pub fast: bool,

    // Cache
    #[arg(long = "no-cache")]
    pub no_cache: bool,
    #[arg(long = "clear-cache")]
    pub clear_cache: bool,

    // Context
    #[arg(long = "no-context")]
    pub no_context: bool,

    // Alternatives
    #[arg(short = 'a', long = "alts")]
    pub alts: Option<u32>,

    // Output controls
    #[arg(short = 'e', long = "explain")]
    pub explain: bool,
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
