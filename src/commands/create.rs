use std::fs::OpenOptions;

use super::Command;
use crate::{GlobalArgs, commands::OneStoreArgs, odal::download_and_write};
use anyhow::bail;
use streampath::StreamPath;

#[derive(Debug, clap::Args, Clone)]
pub struct CreateCmd {
    /// Path to the archive to be written,
    /// which SHOULD end with .ozx and MUST NOT exist (unless --force is given);
    /// MAY be `-` to write to stdout.
    archive: StreamPath,
    #[clap(flatten)]
    store: OneStoreArgs,
    /// Fetch all object keys and sort locally rather than fetching directory-by-directory,
    /// which may be very inefficient on certain stores.
    #[arg(short, long)]
    eager: bool,
    /// Truncate an existing file rather than failing.
    #[arg(short, long)]
    force: bool,
    /// By default, zarr.json files are sorted in breadth-first order at the start of the central directory;
    /// this flag disables that behaviour.
    #[arg(short = 'S', long)]
    no_sort_metadata: bool,
    /// Number of concurrent downloaders to use when creating the archive.
    /// Too many downloaders may overwhelm the store or the network, or could result in high memory usage.
    /// Too few will leave the writer waiting for the next chunks.
    #[arg(short = 'd', long, default_value = "100")]
    downloaders: usize,
}

impl Command for CreateCmd {
    async fn run(self, _global: GlobalArgs) -> anyhow::Result<()> {
        if !self.force && self.archive.path().is_some_and(|p| p.exists()) {
            bail!("Archive already exists. Delete it or use --force.");
        }
        let op = self.store.select()?;

        let list_opts = crate::odal::ListOpts {
            json_first: !self.no_sort_metadata,
            eager: self.eager,
        };
        match self.archive {
            StreamPath::Path(p) => {
                if p.extension().is_none_or(|ext| ext != "ozx") {
                    tracing::warn!("Archive extension should end with .ozx");
                }
                let f = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&p)?;
                download_and_write(op, list_opts, f, self.downloaders).await?;
            }
            StreamPath::Stream => {
                let stdout = std::io::stdout().lock();
                let w = zip::write::StreamWriter::new(stdout);
                download_and_write(op, list_opts, w, self.downloaders).await?;
            }
        }
        Ok(())
    }
}
