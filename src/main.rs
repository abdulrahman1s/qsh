use clap::Parser;
use std::process::ExitCode;

mod cmds;
mod config;
mod providers;
mod util;

use cmds::cli::{Command, ConfigAction};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let cli = cmds::cli::Cli::parse();
    let rc = match cli.command {
        Command::Generate(args) => cmds::generate::run(args).await,
        Command::Record(args) => cmds::record::run(args),
        Command::Init(args) => cmds::init::run(args),
        Command::Known(args) => cmds::known::run(args),
        Command::Config(args) => match args.action {
            ConfigAction::Show => cmds::config::show(&util::settings::load()),
            ConfigAction::Edit => cmds::config::edit(),
            ConfigAction::Set(set_args) => cmds::config::set(set_args),
        },
    };
    ExitCode::from(rc as u8)
}
