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
mod known;
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
        cli::Command::Known(args) => run_known(args),
    };
    ExitCode::from(rc as u8)
}

fn run_known(args: cli::KnownArgs) -> i32 {
    let list = if args.refresh {
        known::refresh()
    } else {
        known::load_or_refresh()
    };
    eprintln!(
        "qsh: {} ({} programs)",
        known::known_path().display(),
        list.len()
    );
    for name in &list {
        println!("{name}");
    }
    0
}
