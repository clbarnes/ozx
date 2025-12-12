pub mod local;
pub mod types;
pub mod zipper;
use anyhow::bail;

use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};

use crate::zipper::OzxCreator;

fn use_stdio<P: AsRef<Path>>(p: P) -> bool {
    p.as_ref() == "-"
}

#[derive(Parser)]
#[command(version, about, long_about)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new .ozx archive from a local zarr hierarchy
    Create {
        /// Path to the archive to be written,
        /// which SHOULD end with .ozx and MUST NOT exist (unless --force is given);
        /// MAY be `-` to write to stdout.
        archive: PathBuf,
        /// Path to the root of the zarr hierarchy to be written;
        /// MUST contain zarr.json
        root: PathBuf,
        /// Truncate an existing file rather than failing.
        #[arg(short, long)]
        force: bool,
        /// By default, zarr.json files are sorted in breadth-first order at the start of the central directory;
        /// this flag disables that behaviour.
        #[arg(short = 'S', long)]
        no_sort_metadata: bool,
    },
}

fn create(
    archive: PathBuf,
    root: PathBuf,
    force: bool,
    no_sort_metadata: bool,
) -> anyhow::Result<()> {
    if !force && archive.try_exists()? {
        bail!("Archive already exists. Delete it or use --force.");
    }
    let res = if use_stdio(&archive) {
        let stdout = std::io::stdout().lock();
        let w = zipper::StreamWriter::new(stdout);
        OzxCreator::new(w, root, !no_sort_metadata).create()
    } else {
        if archive.extension().is_none_or(|ext| ext != ".ozx") {
            log::warn!("Archive extension should end with .ozx");
        }
        let f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&archive)?;
        OzxCreator::new(f, root, !no_sort_metadata).create()
    };
    res.map_err(anyhow::Error::msg)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::Builder::new()
        .filter_level(args.verbose.into())
        .init();
    let Some(cmd) = args.command else {
        eprintln!("No command given");
        return Ok(());
    };
    match cmd {
        Commands::Create {
            archive,
            root,
            force,
            no_sort_metadata,
        } => create(archive, root, force, no_sort_metadata),
    }
}
