use std::{
    collections::BTreeMap,
    io::{Read, Seek},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    process::ExitCode,
};

use futures::{StreamExt, stream::FuturesOrdered};
use zip::{CompressionMethod, ZipArchive};

use crate::{zarr_metadata::ZarrMetadata, zip_comment::ZipComment};

#[derive(Debug, clap::Args, Clone)]
pub struct ValidateCmd {
    /// Local OZX archives to validate.
    #[arg(required = true, value_name = "PATH", value_hint = clap::ValueHint::FilePath)]
    paths: Vec<PathBuf>,
    /// Validation warnings will be treated as errors.
    #[arg(short, long)]
    strict: bool,
    /// Quit at the first error.
    #[arg(short, long)]
    fail_fast: bool,
    /// Print the validation results, but return exit code 0.
    #[arg(short = 'C', long)]
    no_exit_code: bool,
}

#[derive(Debug, thiserror::Error)]
enum Report {
    #[error("could not read archive: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not parse archive: {0}")]
    Zip(zip::result::ZipError),
    #[error("missing OZX zip comment")]
    MissingComment,
    #[error("invalid OZX zip comment: {0}")]
    InvalidComment(String),
    #[error("found compression method {0}")]
    Compression(CompressionMethod),
    #[error("metadata files not in breadth-first order (unexpected: {unexpected})")]
    MetadataNotBreadthFirst { unexpected: bool },
    #[error("metadata file after non-metadata files (unexpected: {unexpected})")]
    MetadataAfterNonMetadata { unexpected: bool },
    #[error("archive is missing root zarr.json")]
    NoRootZarrJson,
    #[error("zarr.json at root is invalid: {0}")]
    InvalidRootZarrJson(serde_json::Error),
    #[error("zarr.json at root is missing OME-Zarr metadata or version")]
    MissingRootOmeZarrVersion,
    #[error(
        "zip comment has version {comment} where zarr.json at root has OME-Zarr version {zarr_json}"
    )]
    MismatchRootOmeZarrVersion { zarr_json: String, comment: String },
    #[error("Path should end with .ozx")]
    InvalidExtension,
    #[error("contains a .ozx entry")]
    ContainsOzxEntry,
}

impl From<zip::result::ZipError> for Report {
    fn from(value: zip::result::ZipError) -> Self {
        match value {
            zip::result::ZipError::Io(error) => Self::Io(error),
            e => Self::Zip(e),
        }
    }
}

#[derive(Debug)]
pub struct Reports {
    path: PathBuf,
    items: Vec<Report>,
}

impl std::fmt::Display for Reports {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} validation errors for {}",
            self.items.len(),
            self.path.display()
        )?;
        Ok(())
    }
}

impl std::error::Error for Reports {}

impl Report {
    fn level(&self) -> ReportLevel {
        use ReportLevel::*;
        match self {
            Report::Io(_) => Critical,
            Report::Zip(e) => match e {
                zip::result::ZipError::FileNotFound => Error,
                _ => Critical,
            },
            Report::MetadataAfterNonMetadata { unexpected, .. } => {
                if *unexpected {
                    Error
                } else {
                    Info
                }
            }
            Report::MetadataNotBreadthFirst { unexpected, .. } => {
                if *unexpected {
                    Error
                } else {
                    Info
                }
            }
            Report::MissingComment => Warn,
            Report::InvalidComment(_) => Warn,
            Report::Compression(_) => Warn,
            Report::NoRootZarrJson => Error,
            Report::InvalidRootZarrJson(_) => Warn,
            Report::MissingRootOmeZarrVersion => Warn,
            Report::MismatchRootOmeZarrVersion { .. } => Error,
            Report::InvalidExtension => Warn,
            Report::ContainsOzxEntry => Error,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ReportLevel {
    Info,
    Warn,
    Fail,
    Error,
    Critical,
}

impl ReportLevel {
    fn as_str(&self) -> &'static str {
        match self {
            ReportLevel::Info => "INFO",
            ReportLevel::Warn => "WARN",
            ReportLevel::Fail => "FAIL",
            ReportLevel::Error => "ERROR",
            ReportLevel::Critical => "CRITICAL",
        }
    }
}

impl std::fmt::Display for ReportLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug)]
struct Validator {
    path: PathBuf,
    strict: bool,
    fail_fast: bool,
    errors: Vec<Report>,
    all_zarr_json: bool,
    max_slash: usize,
    breadthfirst_flagged: bool,
    jsonfirst_flagged: bool,
    json_first: Option<bool>,
    compression_methods: Vec<CompressionMethod>,
}

fn get_ext(name: &[u8]) -> Option<&[u8]> {
    name.rsplit(|&c| c == b'.').next()
}

impl Validator {
    fn new(path: PathBuf, strict: bool, fail_fast: bool) -> Self {
        Self {
            path,
            strict,
            fail_fast,
            errors: Vec::new(),
            all_zarr_json: true,
            max_slash: 0,
            breadthfirst_flagged: false,
            jsonfirst_flagged: false,
            json_first: None,
            compression_methods: vec![],
        }
    }

    fn raise_at(&self) -> ReportLevel {
        if self.fail_fast {
            if self.strict {
                ReportLevel::Warn
            } else {
                ReportLevel::Fail
            }
        } else {
            ReportLevel::Critical
        }
    }

    fn drain_into_error(&mut self) -> Reports {
        Reports {
            path: std::mem::take(&mut self.path),
            items: std::mem::take(&mut self.errors),
        }
    }

    fn maybe_raise(&mut self, err: impl Into<Report>) -> Result<(), Reports> {
        let e = err.into();
        let should_raise = e.level() >= self.raise_at();
        self.errors.push(e);
        if should_raise {
            Err(self.drain_into_error())
        } else {
            Ok(())
        }
    }

    fn open_archive(&mut self) -> Result<ZipArchive<impl Read + Seek + use<>>, Reports> {
        let f = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                self.maybe_raise(e)?;
                unreachable!()
            }
        };
        let reader = std::io::BufReader::new(f);
        let z = match ZipArchive::new(reader) {
            Ok(z) => z,
            Err(e) => {
                self.maybe_raise(e)?;
                unreachable!()
            }
        };
        Ok(z)
    }

    /// Return the OME-Zarr version and json_first value from the zip comment, if present.
    fn read_comment(
        &mut self,
        z: &ZipArchive<impl Read + Seek>,
    ) -> Result<(Option<String>, Option<bool>), Reports> {
        let comment = z.comment();
        if comment.is_empty() {
            self.maybe_raise(Report::MissingComment)?;
        }
        let name_json_first = match serde_json::from_slice::<ZipComment>(comment) {
            Ok(c) => (
                Some(c.ome.version),
                c.ome
                    .zip_file
                    .and_then(|zf| zf.central_directory?.json_first),
            ),
            Err(e) => {
                self.maybe_raise(Report::InvalidComment(e.to_string()))?;
                (None, None)
            }
        };
        Ok(name_json_first)
    }

    pub fn check_order(&mut self, name: &[u8]) -> Result<(), Reports> {
        let is_zarr_metadata = name == b"zarr.json" || name.ends_with(b"zarr.json");
        if !is_zarr_metadata {
            self.all_zarr_json = false;
            return Ok(());
        }

        if !self.all_zarr_json && !self.jsonfirst_flagged {
            self.jsonfirst_flagged = true;
            self.maybe_raise(Report::MetadataAfterNonMetadata {
                unexpected: self.json_first.is_some_and(|j| j),
            })?;
        }

        let n_slash = name.iter().filter(|&&c| c == b'/').count();
        match n_slash.cmp(&self.max_slash) {
            std::cmp::Ordering::Greater => self.max_slash = n_slash,
            std::cmp::Ordering::Less if !self.breadthfirst_flagged => {
                self.breadthfirst_flagged = true;
                self.maybe_raise(Report::MetadataNotBreadthFirst {
                    unexpected: self.json_first.is_some_and(|j| j),
                })?;
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_entry(
        &mut self,
        z: &ZipArchive<impl Read + Seek>,
        idx: usize,
    ) -> Result<(), Reports> {
        let entry = match z.by_index_data(idx) {
            Ok(e) => e,
            Err(e) => {
                self.maybe_raise(e)?;
                return Ok(());
            }
        };
        if entry.is_dir() {
            return Ok(());
        }
        if get_ext(entry.name_raw()) == Some(b"ozx") {
            self.maybe_raise(Report::ContainsOzxEntry)?;
        }

        let comp = entry.compression();
        if comp != CompressionMethod::Stored && !self.compression_methods.contains(&comp) {
            self.compression_methods.push(comp);
            self.maybe_raise(Report::Compression(entry.compression()))?;
        }
        self.check_order(entry.name_raw())
    }

    pub fn check_root_zarr_json(
        &mut self,
        z: &mut ZipArchive<impl Read + Seek>,
        version: Option<&str>,
    ) -> Result<(), Reports> {
        let mut entry = match z.by_name_raw(b"zarr.json") {
            Ok(e) => e,
            Err(e) => {
                if matches!(e, zip::result::ZipError::FileNotFound) {
                    self.maybe_raise(Report::NoRootZarrJson)?;
                } else {
                    self.maybe_raise(e)?;
                }
                return Ok(());
            }
        };
        if !matches!(entry.compression(), CompressionMethod::STORE) {
            return Ok(());
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        if let Err(e) = entry.read_to_end(&mut buf) {
            self.maybe_raise(e)?;
        }
        let metadata: ZarrMetadata = match serde_json::from_slice(&buf) {
            Ok(m) => m,
            Err(e) => {
                self.maybe_raise(Report::InvalidRootZarrJson(e))?;
                return Ok(());
            }
        };
        let Some(v) = metadata.version() else {
            self.maybe_raise(Report::MissingRootOmeZarrVersion)?;
            return Ok(());
        };
        if let Some(comment_version) = version
            && comment_version != v
        {
            self.maybe_raise(Report::MismatchRootOmeZarrVersion {
                zarr_json: v.to_string(),
                comment: comment_version.to_string(),
            })?;
        }
        Ok(())
    }

    pub fn validate(mut self) -> Result<PathBuf, Reports> {
        if self.path.extension().map(|x| x.as_bytes()) != Some(b"ozx") {
            self.maybe_raise(Report::InvalidExtension)?;
        }
        let mut z = self.open_archive()?;
        let (opt_version, opt_json_first) = { self.read_comment(&z)? };
        self.json_first = opt_json_first;

        for idx in 0..z.len() {
            self.validate_entry(&z, idx)?;
        }

        self.check_root_zarr_json(&mut z, opt_version.as_deref())?;

        if self.errors.is_empty() {
            Ok(self.path)
        } else {
            Err(self.drain_into_error())
        }
    }
}

impl super::Command for ValidateCmd {
    async fn run(self, _global: crate::GlobalArgs) -> anyhow::Result<ExitCode> {
        let mut futs = FuturesOrdered::new();
        for path in self.paths {
            futs.push_back(tokio::task::spawn_blocking(move || {
                let val = Validator::new(path, self.strict, self.fail_fast);
                val.validate()
            }));
        }

        let mut counts_by_level = BTreeMap::<ReportLevel, usize>::new();

        while let Some(res) = futs.next().await {
            match res {
                Ok(Ok(path)) => {
                    println!("{}\tOK", path.display());
                }
                Ok(Err(reports)) => {
                    println!(
                        "{}\t{} reports",
                        reports.path.display(),
                        reports.items.len()
                    );
                    for err in reports.items {
                        counts_by_level
                            .entry(err.level())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);
                        println!("\t- {}: {}", err.level(), err);
                    }
                }
                Err(e) => {
                    println!("Task failed: {}", e);
                }
            }
        }

        if self.no_exit_code {
            return Ok(Default::default());
        }

        let Some((level, _)) = counts_by_level.last_key_value() else {
            return Ok(Default::default());
        };

        let min_level = if self.strict {
            ReportLevel::Warn
        } else {
            ReportLevel::Fail
        };

        if *level >= min_level {
            return Ok(match level {
                ReportLevel::Info => 0.into(),
                ReportLevel::Warn => 1.into(),
                ReportLevel::Fail => 2.into(),
                ReportLevel::Error => 3.into(),
                ReportLevel::Critical => 4.into(),
            });
        }
        Ok(Default::default())
    }
}
