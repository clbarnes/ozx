pub mod types;

mod commands;
mod config;
mod odal;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::commands::Command;

#[derive(Debug, Parser)]
#[command(version, about, long_about)]
struct Args {
    #[command(flatten)]
    global: GlobalArgs,
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Option<commands::Commands>,
}

#[derive(Debug, clap::Args)]
struct GlobalArgs {
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let indicatif_layer = tracing_indicatif::IndicatifLayer::new();
    tracing_subscriber::registry()
        .with(args.verbose.tracing_level_filter())
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();
    tracing::debug!("Parsed args: {:?}", args);
    let Some(cmd) = args.command else {
        eprintln!("No command given");
        return Ok(());
    };

    cmd.run(args.global).await
}
