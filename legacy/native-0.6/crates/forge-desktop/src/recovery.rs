use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};
use tempfile::Builder;

const RECORD_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub struct Record {
    version: u8,
    pub path: PathBuf,
    pub base: String,
    pub text: String,
}

impl Record {
    pub fn new(path: PathBuf, base: String, text: String) -> Self {
        Self {
            version: 1,
            path,
            base,
            text,
        }
    }
}

pub fn directory() -> io::Result<PathBuf> {
    let home = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
    };
    let directory = home
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("User data folder unavailable."))?
        .join("ForgeIDE")
        .join("Recovery");
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory)?;
    Ok(directory)
}

pub fn write(
    directory: &Path,
    existing: Option<&Path>,
    record: &Record,
) -> io::Result<Option<PathBuf>> {
    let mut temp = Builder::new()
        .prefix("forge-")
        .suffix(".tmp")
        .tempfile_in(directory)?;
    serde_json::to_writer(&mut temp, record)?;
    temp.as_file().sync_all()?;
    if let Some(existing) = existing {
        temp.persist(existing).map_err(|error| error.error)?;
        Ok(None)
    } else {
        let path = temp.path().with_extension("journal");
        temp.persist_noclobber(&path).map_err(|error| error.error)?;
        Ok(Some(path))
    }
}

pub fn read(path: &Path) -> io::Result<Record> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > RECORD_LIMIT {
        return Err(io::Error::other(
            "This recovery file is too large or is not a regular file.",
        ));
    }
    let record: Record = serde_json::from_reader(file.take(RECORD_LIMIT))?;
    if record.version != 1 {
        return Err(io::Error::other("Unsupported recovery format."));
    }
    Ok(record)
}
