use std::process::ExitCode;

use crate::{
    GlobalArgs,
    commands::{Command, OneStoreArgs},
    odal::{KeyStream, ListOpts, is_metadata_path},
};
use futures::StreamExt;

#[derive(Debug, clap::Args, Clone)]
pub struct ListCmd {
    #[clap(flatten)]
    store: OneStoreArgs,
    #[arg(short, long)]
    eager: bool,
    #[arg(short = 'S', long)]
    no_sort_metadata: bool,
    #[arg(short = 'D')]
    no_data: bool,
}

impl Command for ListCmd {
    async fn run(self, _global: GlobalArgs) -> anyhow::Result<ExitCode> {
        let op = self.store.select()?;

        let mut keys = KeyStream::new(
            op,
            ListOpts {
                json_first: !self.no_sort_metadata,
                eager: self.eager,
            },
        );

        while let Some(path) = keys.next().await {
            if self.no_data && !is_metadata_path(&path) {
                if self.no_sort_metadata {
                    continue;
                } else {
                    keys.cancel();
                    return Ok(Default::default());
                }
            }
            println!("{path}");
        }
        keys.finish().await?;
        Ok(Default::default())
    }
}
