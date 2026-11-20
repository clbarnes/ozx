use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    time::Duration,
};

use anyhow::Context;
use futures::{
    Stream, StreamExt,
    stream::{FuturesOrdered, FuturesUnordered},
};
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use opendal::{Buffer, Operator, options::ListOptions};
use tokio::{
    sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tracing::instrument;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{zarr_metadata::ZarrMetadata, zip_comment::ZipComment};

#[derive(Debug, Clone, Default)]
pub struct DirContents {
    metadata: Option<String>,
    dirs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct KeyProvider {
    op: Operator,
    metadata_sender: UnboundedSender<Option<String>>,
    other_sender: UnboundedSender<Option<String>>,
}

pub(crate) fn is_metadata_path(path: &str) -> bool {
    path == "zarr.json" || path.ends_with("/zarr.json")
}

impl KeyProvider {
    pub fn new(
        op: Operator,
        metadata_sender: UnboundedSender<Option<String>>,
        other_sender: UnboundedSender<Option<String>>,
    ) -> Self {
        Self {
            op,
            metadata_sender,
            other_sender,
        }
    }

    async fn get_dir_contents(&self, path: String) -> anyhow::Result<DirContents> {
        tracing::debug!("Getting directory contents for path {}", path);
        let opts = ListOptions {
            recursive: false,
            ..Default::default()
        };
        let mut lister = self.op.lister_options(&path, opts).await?;
        let mut dirs = Vec::new();
        let mut metadata = None;
        while let Some(entry) = lister.next().await.transpose()? {
            let (name, meta) = entry.into_parts();
            if name == path {
                tracing::debug!(
                    "Skipping entry for path {} because it is the same as the requested path",
                    path
                );
                continue;
            }
            match meta.mode() {
                opendal::EntryMode::FILE => {
                    if is_metadata_path(&name) {
                        metadata = Some(name);
                    } else {
                        self.other_sender.send(Some(name))?;
                    }
                }
                opendal::EntryMode::DIR => {
                    dirs.push(name);
                }
                opendal::EntryMode::Unknown => {
                    tracing::debug!("Unknown type for path {name}")
                }
            }
        }
        let contents = DirContents { metadata, dirs };
        tracing::debug!("Got directory contents for path {path}: {contents:?}");
        Ok(contents)
    }

    /// Stream paths directory by directory.
    ///
    /// Lower RAM cost, but may be extremely inefficient on certain stores.
    #[instrument]
    pub async fn handle_keys(self, prefix: &str, json_first: bool) -> anyhow::Result<()> {
        if json_first {
            self.handle_keys_jsonfirst(prefix).await
        } else {
            self.handle_keys_unordered(prefix).await
        }
    }

    async fn handle_keys_unordered(self, prefix: &str) -> anyhow::Result<()> {
        self.metadata_sender.send(None)?;
        let path = prefix;
        let opts = ListOptions {
            recursive: true,
            ..Default::default()
        };
        let mut lister = self.op.lister_options(path, opts).await?;
        while let Some(entry) = lister.next().await.transpose()? {
            let (name, meta) = entry.into_parts();
            if meta.mode() != opendal::EntryMode::FILE {
                continue;
            }
            self.other_sender.send(Some(name))?;
        }
        Ok(())
    }

    async fn handle_data_files(&self, prefix: String) -> anyhow::Result<()> {
        let opts = ListOptions {
            recursive: true,
            ..Default::default()
        };
        let mut lister = self.op.lister_options(&prefix, opts).await?;
        while let Some(entry) = lister.next().await.transpose()? {
            if entry.metadata().mode() == opendal::EntryMode::FILE {
                let p = entry.path();
                if p == prefix {
                    continue;
                }
                self.other_sender.send(Some(p.to_string()))?;
            }
        }
        Ok(())
    }

    /// Fetch all the keys and sort metadata files locally.
    pub async fn handle_keys_eager(
        self,
        prefix: impl Into<String>,
        json_first: bool,
    ) -> anyhow::Result<()> {
        if !json_first {
            self.metadata_sender.send(None)?;
        }
        let path = prefix;
        let opts = ListOptions {
            recursive: true,
            ..Default::default()
        };
        let pref = path.into();
        let mut lister = self.op.lister_options(&pref, opts).await?;
        let mut by_slash: BTreeMap<usize, Vec<String>> = Default::default();
        while let Some(entry) = lister.next().await.transpose()? {
            let (name, meta) = entry.into_parts();
            if meta.mode() == opendal::EntryMode::FILE {
                if json_first && is_metadata_path(&name) {
                    let slash_count = name.chars().filter(|&c| c == '/').count();
                    by_slash.entry(slash_count).or_default().push(name);
                } else {
                    self.other_sender.send(Some(name))?;
                }
            }
        }
        self.other_sender.send(None)?;

        // Will have 0 items if json_first is false.
        for s in by_slash.into_values() {
            for p in s {
                self.metadata_sender.send(Some(p))?;
            }
        }
        if json_first {
            self.metadata_sender.send(None)?;
        }

        Ok(())
    }

    async fn handle_keys_jsonfirst(self, prefix: impl Into<String>) -> anyhow::Result<()> {
        let mut futs = FuturesOrdered::default();
        futs.push_back(self.get_dir_contents(prefix.into()));
        let mut filedir_queue: VecDeque<String> = VecDeque::default();
        while let Some(contents) = futs.next().await.transpose()? {
            if let Some(metadata_path) = contents.metadata {
                // has metadata, so it's a node, so eagerly get child directories (which may be nodes)
                self.metadata_sender.send(Some(metadata_path))?;
                for d in contents.dirs {
                    futs.push_back(self.get_dir_contents(d));
                }
            } else {
                // delay processing non-node directories until after all nodes have been processed
                filedir_queue.extend(contents.dirs);
            };
        }

        self.metadata_sender.send(None)?;

        let mut unordered_futs = FuturesUnordered::default();
        for path in filedir_queue {
            unordered_futs.push(self.handle_data_files(path));
        }
        while let Some(res) = unordered_futs.next().await {
            res?;
        }
        self.other_sender.send(None)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListOpts {
    pub json_first: bool,
    pub eager: bool,
}

#[derive(Debug)]
pub struct KeyStream {
    meta_recv: Option<UnboundedReceiver<Option<String>>>,
    other_recv: Option<UnboundedReceiver<Option<String>>>,
    handle: JoinHandle<anyhow::Result<()>>,
}

impl KeyStream {
    pub fn new(op: Operator, opts: ListOpts) -> Self {
        let (meta_send, meta_recv) = tokio::sync::mpsc::unbounded_channel();
        let (other_send, other_recv) = tokio::sync::mpsc::unbounded_channel();
        let provider = KeyProvider::new(op, meta_send, other_send);

        let handle = if opts.eager {
            tokio::spawn(provider.handle_keys_eager("/", opts.json_first))
        } else {
            tokio::spawn(provider.handle_keys("/", opts.json_first))
        };

        Self {
            meta_recv: Some(meta_recv),
            other_recv: Some(other_recv),
            handle,
        }
    }

    pub fn cancel(self) {
        self.handle.abort();
    }

    pub async fn finish(self) -> anyhow::Result<()> {
        self.handle.await?
    }
}

impl Stream for KeyStream {
    type Item = String;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        let selfmut = self.get_mut();
        if let Some(meta_recv) = &mut selfmut.meta_recv {
            match meta_recv.poll_recv(cx) {
                Poll::Ready(Some(Some(path))) => {
                    tracing::trace!("Discovered metadata object: {path}");
                    return Poll::Ready(Some(path));
                }
                Poll::Ready(Some(None)) | Poll::Ready(None) => {
                    selfmut.meta_recv = None;
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
        if let Some(other_recv) = &mut selfmut.other_recv {
            match other_recv.poll_recv(cx) {
                Poll::Ready(Some(Some(path))) => {
                    tracing::trace!("Discovered object: {path}");
                    Poll::Ready(Some(path))
                }
                Poll::Ready(Some(None)) | Poll::Ready(None) => {
                    selfmut.other_recv = None;
                    Poll::Ready(None)
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            Poll::Ready(None)
        }
    }
}

type Handle = JoinHandle<opendal::Result<(String, Buffer)>>;

#[instrument(skip(op, list_opts, value_send))]
pub async fn download_worker(
    op: Operator,
    list_opts: ListOpts,
    value_send: Sender<Option<Handle>>,
) -> anyhow::Result<()> {
    let mut keys = KeyStream::new(op.clone(), list_opts);

    while let Some(path) = keys.next().await {
        let this_op = op.clone();
        let handle = tokio::spawn(async move {
            let data = this_op.read(&path).await?;
            tracing::trace!("Downloaded object: {path} ({} B)", data.len());
            Ok((path, data))
        });
        value_send.send(Some(handle)).await?;
    }

    value_send.send(None).await?;
    keys.finish().await?;
    Ok(())
}

#[instrument(skip(zipper, buf, spinner, options))]
fn write_to_zip(
    zipper: &mut ZipWriter<impl Write + std::io::Seek>,
    path: &str,
    buf: Buffer,
    options: SimpleFileOptions,
    spinner: &indicatif::ProgressBar,
) -> anyhow::Result<u64> {
    let mut out = 0;
    zipper.start_file(path, options)?;
    for chunk in buf {
        let len = chunk.len() as u64;
        zipper.write_all(chunk.as_ref())?;
        spinner.inc(len);
        out += len
    }
    Ok(out)
}

pub async fn download_and_write(
    op: Operator,
    list_opts: ListOpts,
    writer: impl Write + std::io::Seek,
    downloaders: usize,
) -> anyhow::Result<()> {
    let zarr_json = op
        .read("zarr.json")
        .await
        .with_context(|| "could not read root zarr.json")?;
    let oz_meta: ZarrMetadata =
        serde_json::from_reader(zarr_json).with_context(|| "invalid root zarr.json")?;
    let mut zipper = ZipWriter::new(writer);

    let (sender, mut value_recv) = tokio::sync::mpsc::channel(downloaders);

    let download_handle = tokio::spawn(download_worker(op, list_opts, sender));

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);

    let multispinner = indicatif::MultiProgress::new();
    let status = multispinner.add(
        ProgressBar::new_spinner()
            .with_style(
                ProgressStyle::default_spinner()
                    .template("[{elapsed_precise}] {msg}")
                    .expect("timer template should be valid"),
            )
            .with_message("Fetching object list"),
    );
    status.enable_steady_tick(Duration::from_millis(100));

    let objects_spinner = multispinner.add(
        ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {human_pos} files written{msg}")
                .expect("objects template should be valid"),
        ),
    );
    let bytes_spinner = multispinner.add(
        ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {binary_bytes} written ({binary_bytes_per_sec}){msg}")
                .expect("bytes template should be valid"),
        ),
    );
    let mut bytes_out = 0;
    let mut meta_out = 0;
    let mut other_out = 0;

    let rt = tokio::runtime::Handle::current();
    tokio::task::block_in_place(|| -> anyhow::Result<()> {
        while let Some(Some(h)) = value_recv.blocking_recv() {
            let (path, buf) = rt.block_on(h)??;
            status.set_message(format!("Writing {path} ({})", HumanBytes(buf.len() as u64)));
            let wrote = write_to_zip(&mut zipper, &path, buf, options, &bytes_spinner)?;
            if is_metadata_path(&path) {
                meta_out += 1;
            } else {
                other_out += 1;
            }
            bytes_out += wrote;
            objects_spinner.inc(1);
        }
        Ok(())
    })?;

    let comment = ZipComment::new(oz_meta.version().unwrap_or("?"), true);
    let s = serde_json::to_string(&comment)?;

    status.set_message("Setting zip comment");
    zipper.set_comment(s)?;

    objects_spinner.finish();
    bytes_spinner.finish();
    status.finish();
    zipper.flush()?;
    zipper.finish()?;

    download_handle.await??;

    eprintln!(
        "Wrote {} metadata files, {} other files; total {}",
        indicatif::HumanCount(meta_out),
        indicatif::HumanCount(other_out),
        indicatif::HumanBytes(bytes_out)
    );
    Ok(())
}
