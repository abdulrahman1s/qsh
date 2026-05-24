use clap::Parser;
use std::process::ExitCode;

mod alts;
mod cache;
mod clean;
mod cli;
mod config;
mod context;
mod env_detect;
mod files;
mod generate;
mod prompt;
mod provider;
mod qshrc;
mod record;
mod retry;
mod shell;
mod stream;
mod ui;
mod xml_escape;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let rc = match cli.command {
        cli::Command::Generate(args) => generate::run(args).await,
        cli::Command::Record(args) => record::run(args),
        cli::Command::Init(args) => shell::init(args),
    };
    ExitCode::from(rc as u8)
}
