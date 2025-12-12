use std::{
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

pub use zip::write::StreamWriter;
use zip::{self, ZipWriter, write::SimpleFileOptions};

use crate::{local::HierarchyWalkers, types::ZipComment};

pub struct Zipper<W: Write + Seek> {
    writer: ZipWriter<W>,
}

impl<W: Write + Seek> Zipper<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: ZipWriter::new(writer),
        }
    }

    pub fn set_comment(&mut self, comment: &ZipComment) {
        let s = serde_json::to_string(comment).unwrap();
        self.writer.set_zip64_comment(Some(s));
    }

    pub fn write_dir<S: Into<String>>(&mut self, fpath: S) -> Result<(), String> {
        self.writer
            .add_directory(fpath.into(), SimpleFileOptions::default())
            .map_err(|e| e.to_string())
    }

    pub fn write<P: AsRef<Path>, R: Read>(&mut self, fpath: P, mut rdr: R) -> Result<(), String> {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let s = fpath
            .as_ref()
            .to_str()
            .ok_or_else(|| format!("Path is not valid UTF-8: {}", fpath.as_ref().display()))?;
        self.writer
            .start_file(s, options)
            .map_err(|e| e.to_string())?;
        // on linux, this is pretty efficient;
        // other platforms might benefit from buffering
        std::io::copy(&mut rdr, &mut self.writer).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn finish(self) -> Result<(), String> {
        self.writer.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub struct OzxCreator<W: Write + Seek> {
    zipper: Zipper<W>,
    walkers: HierarchyWalkers,
    json_first: bool,
}

impl<W: Write + Seek> OzxCreator<W> {
    pub fn new<P: Into<PathBuf>>(writer: W, root: P, json_first: bool) -> Self {
        Self {
            zipper: Zipper::new(writer),
            walkers: HierarchyWalkers::new(root),
            json_first,
        }
    }

    pub fn create(&mut self) -> Result<(), String> {
        let root_meta = self.walkers.root_metadata()?;
        let Some(version) = root_meta
            .attributes
            .ome
            .as_ref()
            .map(|ome| ome.version.as_str())
        else {
            return Err("No OME metadata found in root zarr.json".into());
        };

        self.zipper
            .set_comment(&ZipComment::new(version, self.json_first));

        if self.json_first {
            for (p, contents) in self.walkers.walk_metadata() {
                self.zipper.write(&p, contents.as_slice())?;
            }
        }

        for (p, maybe_f) in self.walkers.walk_data(self.json_first) {
            let Some(f) = maybe_f else { continue };
            self.zipper.write(&p, f)?;
        }
        Ok(())
    }
}
