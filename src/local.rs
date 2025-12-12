use crate::types::ZarrV3Metadata;
use std::{
    collections::VecDeque,
    fs::{self, File},
    path::{Path, PathBuf},
};

struct ZarrMetadataWalker {
    /// Canonicalized root path.
    root_dir: PathBuf,
    /// Directory paths relative to [Walker::root_dir].
    dirs_to_visit: VecDeque<PathBuf>,
}

impl ZarrMetadataWalker {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir
                .as_ref()
                .canonicalize()
                .expect("canonicalize root dir"),
            dirs_to_visit: VecDeque::from([PathBuf::default()]),
        }
    }

    fn is_array(&self, contents: &[u8]) -> bool {
        let metadata: ZarrV3Metadata =
            serde_json::from_slice(contents).expect("read/ deserialise zarr.json");
        metadata.is_array()
    }
}

impl Iterator for ZarrMetadataWalker {
    type Item = (PathBuf, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(local) = self.dirs_to_visit.pop_front() {
            log::debug!("Popped {local:?} from dirs_to_visit");
            let mut dirs = vec![];
            let mut to_return = None;
            let canonical = self.root_dir.join(&local);
            for res in fs::read_dir(&canonical).expect("read dir") {
                let entry = res.expect("read entry");
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.') {
                    log::debug!("Skipping hidden entry {name:?}");
                    continue;
                }
                log::debug!("Addressing entry {name:?}");

                let ft = entry.file_type().expect("file type");

                if ft.is_file() && name == "zarr.json" {
                    let contents = fs::read(canonical.join(&name)).expect("read zarr.json");
                    let return_now = self.is_array(&contents);
                    to_return = Some((local.join(&name), contents));
                    if return_now {
                        return to_return;
                    }
                }

                if ft.is_dir() {
                    dirs.push(local.join(&name));
                }
            }
            dirs.sort();
            self.dirs_to_visit.extend(dirs);
            if to_return.is_some() {
                return to_return;
            }
        }
        None
    }
}

/// Skips zarr.json entries.
struct DataWalker {
    root_dir: PathBuf,
    /// Relative to root_dir
    dirs_to_visit: Vec<PathBuf>,
    /// Relative to root_dir
    files_to_yield: Vec<PathBuf>,
    skip_zarr_json: bool,
}

impl DataWalker {
    pub fn new<P: AsRef<Path>>(root: P, skip_zarr_json: bool) -> Self {
        let root_dir = root
            .as_ref()
            .canonicalize()
            .expect("canonicalize root path");

        Self {
            root_dir,
            dirs_to_visit: Vec::from([PathBuf::default()]),
            files_to_yield: Vec::default(),
            skip_zarr_json,
        }
    }
}

impl Iterator for DataWalker {
    type Item = (PathBuf, Option<File>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(local) = self.files_to_yield.pop() {
                let canonical = self.root_dir.join(&local);
                match fs::OpenOptions::new().read(true).open(&canonical) {
                    Ok(f) => return Some((local, Some(f))),
                    Err(e) => {
                        log::warn!("Skipping {canonical:?} after error: {e}");
                        continue;
                    }
                }
            } else if let Some(local) = self.dirs_to_visit.pop() {
                // sort items into files and dirs
                let canonical = self.root_dir.join(&local);
                let readdir = match fs::read_dir(&canonical) {
                    Ok(rd) => rd,
                    Err(e) => {
                        log::warn!(
                            "Skipping {} after error listing directory: {e}",
                            canonical.display()
                        );
                        continue;
                    }
                };
                for res in readdir {
                    let entry = match res {
                        Ok(e) => e,
                        Err(e) => {
                            log::warn!("Skipping entry after stat error: {e}");
                            continue;
                        }
                    };
                    let ft = match entry.file_type() {
                        Ok(ft) => ft,
                        Err(e) => {
                            log::warn!(
                                "Skipping entry after stat error for {}: {e}",
                                entry.path().display()
                            );
                            continue;
                        }
                    };
                    if ft.is_dir() {
                        self.dirs_to_visit.push(local.join(entry.file_name()));
                    } else if ft.is_file() {
                        if !self.skip_zarr_json || entry.file_name() != "zarr.json" {
                            self.files_to_yield.push(local.join(entry.file_name()));
                        }
                    } else {
                        log::warn!(
                            "Skipping non-file, non-directory entry {}",
                            entry.path().display()
                        );
                    }
                }
                // skip empty (i.e. initial) directory
                if local.components().next().is_some() {
                    return Some((local, None));
                }
            } else {
                return None;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct HierarchyWalkers {
    root_dir: PathBuf,
}

impl HierarchyWalkers {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self {
            root_dir: root.into(),
        }
    }

    pub fn root_metadata(&self) -> Result<ZarrV3Metadata, String> {
        let p = self.root_dir.join("zarr.json");
        let contents = fs::read(&p).map_err(|e| e.to_string())?;
        serde_json::from_slice(&contents).map_err(|e| e.to_string())
    }

    pub fn walk_metadata(&self) -> impl Iterator<Item = (PathBuf, Vec<u8>)> {
        log::info!("Finding metadata");
        ZarrMetadataWalker::new(&self.root_dir)
    }

    pub fn walk_data(&self, skip_zarr_json: bool) -> impl Iterator<Item = (PathBuf, Option<File>)> {
        log::info!("Finding data");
        DataWalker::new(&self.root_dir, skip_zarr_json)
    }
}
