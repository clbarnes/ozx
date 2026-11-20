use std::{io::Write, path::PathBuf, process::ExitCode};

use zip::ZipArchive;

#[derive(Debug, clap::Args, Clone)]
pub struct CommentCmd {
    /// Local OZX archive whose zip comment should be printed.
    #[arg(value_name = "PATH", value_hint = clap::ValueHint::FilePath)]
    path: PathBuf,
}

impl super::Command for CommentCmd {
    async fn run(self, _global: crate::GlobalArgs) -> anyhow::Result<ExitCode> {
        let f = std::fs::File::open(&self.path)?;
        let z = ZipArchive::new(f)?;
        let cmt = z.comment().to_vec();

        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&cmt)?;
        if !cmt.ends_with(b"\n") {
            stdout.write_all(b"\n")?;
        }
        Ok(ExitCode::SUCCESS)
    }
}
