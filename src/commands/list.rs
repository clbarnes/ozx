use crate::{
    GlobalArgs,
    commands::{Command, OneStoreArgs},
    odal::{KeyStream, ListOpts},
};
use futures::StreamExt;

#[derive(Debug, clap::Args, Clone)]
pub struct ListCmd {
    #[clap(flatten)]
    store: OneStoreArgs,
    #[arg(short, long)]
    eager: bool,
    #[arg(short, long, default_value = "10")]
    threads: usize,
    #[arg(short = 'S', long)]
    no_sort_metadata: bool,
}

impl Command for ListCmd {
    async fn run(self, _global: GlobalArgs) -> anyhow::Result<()> {
        let op = self.store.select()?;

        let mut keys = KeyStream::new(
            op,
            ListOpts {
                json_first: !self.no_sort_metadata,
                eager: self.eager,
            },
        );

        while let Some(path) = keys.next().await {
            println!("{path}");
        }
        keys.finish().await?;
        Ok(())
    }
}
