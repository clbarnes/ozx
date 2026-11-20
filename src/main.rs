#![doc = include_str!("../README.md")]
pub mod zip_comment;

mod commands;
mod config;
mod odal;
mod zarr_metadata;

use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::commands::Command;

///
/// The STORE argument may be a URL (i.e. start with a scheme), a store name from a config file, or a path to a local directory.
/// Further configuration options may be specified with `--option key=value`, or in a config file `--config path/to/config.toml`, or with an environment variable `OZX_STORE__{key}=value` (note the double underscore!).
///
/// See further documetation at <https://docs.rs/crate/ozx/latest>.
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
struct GlobalArgs {}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
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
        return Ok(Default::default());
    };

    cmd.run(args.global).await
}
