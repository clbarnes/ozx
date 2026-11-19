use std::collections::{BTreeMap, btree_map::Entry};

use anyhow::bail;
use clap::Subcommand;
mod list;
use list::ListCmd;
mod create;
use create::CreateCmd;
use storecfg::Values;
use streampath::StreamPathRead;

use crate::{GlobalArgs, config::Config};

pub trait Command {
    /// Execute the command.
    async fn run(self, global: GlobalArgs) -> anyhow::Result<()>;
}

#[derive(Debug, Subcommand, Clone)]
pub enum Commands {
    /// List the contents of a zarr hierarchy in the order they would be zipped.
    List(ListCmd),
    /// Create a new .ozx archive.
    Create(CreateCmd),
}

impl Command for Commands {
    async fn run(self, global: GlobalArgs) -> anyhow::Result<()> {
        match self {
            Commands::List(cmd) => cmd.run(global).await,
            Commands::Create(cmd) => cmd.run(global).await,
        }
    }
}

fn merge_config(
    base: &mut BTreeMap<String, Values>,
    other: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
) {
    for (key, value) in other {
        match base.entry(key.into()) {
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(Values::new(value.into()));
            }
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut().push(value.into()),
        }
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct OneStoreArgs {
    store: Option<String>,
    #[arg(short, long="option", value_parser = storecfg::cli::parse_key_value_clap)]
    options: Vec<(String, String)>,
    #[arg(short, long = "config")]
    configs: Vec<StreamPathRead>,
}

impl OneStoreArgs {
    pub fn select(self) -> anyhow::Result<opendal::Operator> {
        let mut coll = storecfg::StoreConfigCollector::default();
        for spr in self.configs {
            let contents = spr.read_all()?;
            let config: Config = serde_json::from_slice(&contents)?;
            for (name, kvs) in config.stores {
                for (k, v) in kvs {
                    coll.add(&name, &k, v);
                }
            }
        }
        coll.add_from_env("OZX_STORE")?;
        // for (k, v) in options {
        //     coll.add("", k, v);
        // }
        let cfg = if let Some(store) = self.store {
            if store.contains("://") {
                let mut cfg = storecfg::StoreConfig::new(store);
                let mut kvs = coll.name_cfg_values.remove("").unwrap_or_default();
                merge_config(&mut kvs, self.options);

                cfg.extend_options(kvs.into_iter().map(|(k, v)| (k, v.into_last())));
                cfg
            } else if let Some(mut map) = coll.name_cfg_values.remove(&store) {
                merge_config(&mut map, self.options);
                let Some(uri) = map.remove("uri") else {
                    bail!(
                        "Store configuration for '{store}' does not contain a 'uri' key. Please provide a store URI and options."
                    );
                };
                let mut cfg = storecfg::StoreConfig::new(uri.into_last());
                cfg.extend_options(map.into_iter().map(|(k, v)| (k, v.into_last())));
                cfg
            } else if let Ok(path) = std::fs::canonicalize(&store) {
                let mut map = coll.name_cfg_values.remove("").unwrap_or_default();
                merge_config(&mut map, self.options);
                map.remove("uri");
                let mut cfg =
                    storecfg::StoreConfig::new(format!("file://{}", path.to_string_lossy()));
                cfg.extend_options(map.into_iter().map(|(k, v)| (k, v.into_last())));
                cfg
            } else {
                bail!(
                    "Store configuration for '{store}' not found. Please provide a store URI and options."
                )
            }
        } else {
            let mut map = coll.name_cfg_values.remove("").unwrap_or_default();
            merge_config(&mut map, self.options);
            let Some(uri) = map.remove("uri") else {
                bail!("Store configuration not found. Please provide a store URI and options.");
            };
            let mut cfg = storecfg::StoreConfig::new(uri.into_last());
            cfg.extend_options(map.into_iter().map(|(k, v)| (k, v.into_last())));
            cfg
        };

        let op = opendal::Operator::from_uri(cfg.into_tuple())?
            .layer(
                opendal::layers::RetryLayer::new()
                    .with_max_times(3)
                    .with_factor(2.0)
                    .with_min_delay(std::time::Duration::from_millis(200)),
            )
            .layer(opendal::layers::TracingLayer::new());
        Ok(op)
    }
}
