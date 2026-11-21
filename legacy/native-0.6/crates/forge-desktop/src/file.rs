use crate::recovery::{self, Record};
use forge_text::{Document, Edit};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const EDIT_LIMIT: u64 = 10 * 1024 * 1024;
const BOM: &[u8] = b"\xef\xbb\xbf";

#[derive(Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub text: String,
    buffer: Document,
    saved: Vec<u8>,
    dirty: bool,
    bom: bool,
    journal: Option<PathBuf>,
    checkpoint: Option<forge_text::Revision>,
    disk_stamp: Option<DiskStamp>,
    pub external_change: bool,
}

impl OpenFile {
    pub fn open(path: &Path) -> io::Result<Self> {
        let path = path.canonicalize()?;
        let disk_stamp = DiskStamp::read(&path).ok();
        let saved = read_file(&path)?;
        let bom = saved.starts_with(BOM);
        let text = std::str::from_utf8(saved.strip_prefix(BOM).unwrap_or(&saved))
            .map_err(|_| io::Error::other("Only UTF-8 files can be edited."))?;
        if text.contains('\0') {
            return Err(io::Error::other("Binary files cannot be edited."));
        }

        Ok(Self {
            path,
            text: text.to_owned(),
            buffer: Document::new(text),
            saved,
            dirty: false,
            bom,
            journal: None,
            checkpoint: None,
            disk_stamp,
            external_change: false,
        })
    }

    /// Save a new file or a conflict copy without replacing an existing path.
    pub fn create_at(path: &Path, text: &str, bom: bool) -> io::Result<Self> {
        if text.len() as u64 + if bom { BOM.len() as u64 } else { 0 } > EDIT_LIMIT {
            return Err(io::Error::other(
                "The edited file exceeds the 10 MiB limit.",
            ));
        }
        let mut replacement = NamedTempFile::new_in(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing parent folder."))?,
        )?;
        if bom {
            replacement.write_all(BOM)?;
        }
        replacement.write_all(text.as_bytes())?;
        replacement.as_file().sync_all()?;
        replacement
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
        Self::open(path)
    }

    /// Keep the open buffer and recovery record attached to a moved file.
    pub fn relocate(&mut self, target: &Path) -> io::Result<()> {
        let target = target.canonicalize()?;
        let current = read_file(&target)?;
        self.external_change = current != self.saved;
        self.path = target;
        self.disk_stamp = DiskStamp::read(&self.path).ok();
        self.checkpoint = None;
        if self.is_dirty() {
            self.checkpoint(&recovery::directory()?)?;
        }
        Ok(())
    }

    pub fn save_copy(&self, path: &Path) -> io::Result<Self> {
        Self::create_at(path, &self.text, self.bom)
    }

    pub fn recover(record: Record) -> io::Result<Self> {
        if !record.path.is_absolute()
            || record.text.len() as u64 > EDIT_LIMIT
            || record.base.len() as u64 > EDIT_LIMIT
            || record.text.contains('\0')
        {
            return Err(io::Error::other("Invalid recovery data."));
        }
        let saved = record.base.into_bytes();
        let disk_stamp = DiskStamp::read(&record.path).ok();
        let external_change = !read_file(&record.path).is_ok_and(|current| current == saved);
        let mut file = Self {
            path: record.path,
            buffer: Document::new(&record.text),
            bom: saved.starts_with(BOM),
            saved,
            text: record.text,
            dirty: true,
            journal: None,
            checkpoint: None,
            disk_stamp,
            external_change,
        };
        file.sync_edit().map_err(io::Error::other)?;
        Ok(file)
    }

    pub fn checkpoint(&mut self, directory: &Path) -> io::Result<()> {
        if !self.dirty {
            return self.clear_checkpoint();
        }
        let revision = self.buffer.snapshot().revision();
        if self.checkpoint == Some(revision) {
            return Ok(());
        }
        let base = std::str::from_utf8(&self.saved)
            .map_err(io::Error::other)?
            .to_owned();
        let record = Record::new(self.path.clone(), base, self.text.clone());
        if let Some(path) = recovery::write(directory, self.journal.as_deref(), &record)? {
            self.journal = Some(path);
        }
        self.checkpoint = Some(revision);
        Ok(())
    }

    pub fn clear_checkpoint(&mut self) -> io::Result<()> {
        if let Some(path) = &self.journal {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            self.journal = None;
        }
        self.checkpoint = None;
        Ok(())
    }

    pub fn disk_stamp(&self) -> Option<&DiskStamp> {
        self.disk_stamp.as_ref()
    }

    pub fn revision(&self) -> forge_text::Revision {
        self.buffer.snapshot().revision()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn sync_edit(&mut self) -> Result<(), forge_text::EditError> {
        let snapshot = self.buffer.snapshot();
        let previous = snapshot.chunks().collect::<String>();
        if previous != self.text {
            let (range, replacement) = changed_range(&previous, &self.text);
            self.buffer
                .apply(snapshot.revision(), &[Edit { range, replacement }])?;
        }
        self.dirty = self.text.as_bytes()
            != self
                .saved
                .strip_prefix(if self.bom { BOM } else { b"" })
                .unwrap_or(&self.saved);
        Ok(())
    }

    pub fn save(&mut self) -> io::Result<()> {
        self.sync_edit().map_err(io::Error::other)?;
        if !self.dirty {
            return self.clear_checkpoint();
        }
        if self.text.len() as u64 + if self.bom { BOM.len() as u64 } else { 0 } > EDIT_LIMIT {
            return Err(io::Error::other(
                "The edited file exceeds the 10 MiB limit.",
            ));
        }

        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing parent folder."))?;
        let mut bytes = Vec::new();
        if self.bom {
            bytes.extend_from_slice(BOM);
        }
        bytes.extend_from_slice(self.text.as_bytes());

        let metadata = checked_metadata(&self.path)?;
        let mut replacement = NamedTempFile::new_in(parent)?;
        replacement.write_all(&bytes)?;
        replacement
            .as_file()
            .set_permissions(metadata.permissions())?;
        replacement.as_file().sync_all()?;
        if read_file(&self.path)? != self.saved {
            return Err(io::Error::other(
                "The file changed on disk. Your edits are still open.",
            ));
        }
        crate::history::archive(
            &self.path,
            std::str::from_utf8(&self.saved).map_err(io::Error::other)?,
        )?;
        replacement
            .persist(&self.path)
            .map_err(|error| error.error)?;
        self.saved = bytes;
        self.dirty = false;
        self.disk_stamp = DiskStamp::read(&self.path).ok();
        self.external_change = false;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        self.clear_checkpoint()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiskStamp {
    size: u64,
    modified: std::time::SystemTime,
    #[cfg(unix)]
    identity: (u64, u64),
}

impl DiskStamp {
    pub fn read(path: &Path) -> io::Result<Self> {
        let metadata = checked_metadata(path)?;
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified()?,
            #[cfg(unix)]
            identity: (metadata.dev(), metadata.ino()),
        })
    }
}

fn checked_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || path.canonicalize()? != path {
        return Err(io::Error::other(
            "The file path changed or is no longer a regular file.",
        ));
    }
    Ok(metadata)
}

fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    checked_metadata(path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(EDIT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > EDIT_LIMIT {
        return Err(io::Error::other(
            "Files larger than 10 MiB cannot be edited.",
        ));
    }
    Ok(bytes)
}

pub(crate) fn changed_range<'a>(before: &str, after: &'a str) -> (std::ops::Range<usize>, &'a str) {
    let start = before
        .chars()
        .zip(after.chars())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();
    let suffix = before[start..]
        .chars()
        .rev()
        .zip(after[start..].chars().rev())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();
    (
        start..before.len() - suffix,
        &after[start..after.len() - suffix],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_keeps_previous_version_for_later_review() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("history.txt");
        std::fs::write(&path, "previous 🦀")?;
        let mut file = OpenFile::open(&path)?;
        file.text = "next version".into();
        file.save()?;
        let versions = crate::history::versions(&file.path)?;
        assert_eq!(versions.len(), 1);
        let record = crate::recovery::read(&versions[0])?;
        assert_eq!(record.text, "previous 🦀");
        assert_eq!(std::fs::read_to_string(path)?, "next version");
        std::fs::remove_file(&versions[0])?;
        Ok(())
    }

    #[test]
    fn edits_and_reopens_utf8_with_bom_and_mixed_endings() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("code.rs");
        fs::write(&path, b"\xef\xbb\xbfa\r\nb\nc")?;
        let mut file = OpenFile::open(&path)?;
        file.text.replace_range(3..4, "🦀");
        file.sync_edit().unwrap();
        assert!(file.is_dirty());
        file.save()?;
        assert!(!file.is_dirty());
        assert_eq!(fs::read(&path)?, "\u{feff}a\r\n🦀\nc".as_bytes());
        assert_eq!(OpenFile::open(&path)?.text, file.text);
        Ok(())
    }

    #[test]
    fn saving_a_copy_never_replaces_an_existing_file() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("code.rs");
        let file = OpenFile::create_at(&path, "first", false)?;
        assert_eq!(
            OpenFile::create_at(&path, "second", false)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(file.text, fs::read_to_string(&path)?);
        Ok(())
    }

    #[test]
    fn recovery_survives_dropped_session_and_checks_original_disk_version() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("code.rs");
        fs::write(&path, "original")?;
        let mut file = OpenFile::open(&path)?;
        file.text = "unsaved 🦀".to_owned();
        file.sync_edit().unwrap();
        file.checkpoint(folder.path())?;
        let journal = file.journal.clone().unwrap();
        drop(file);
        fs::write(&path, "external")?;
        let mut file = OpenFile::recover(recovery::read(&journal)?)?;
        assert_eq!(file.text, "unsaved 🦀");
        assert!(file.save().is_err());
        assert_eq!(fs::read_to_string(&path)?, "external");
        Ok(())
    }

    #[test]
    fn external_change_keeps_both_versions() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("code.rs");
        fs::write(&path, "original")?;
        let mut file = OpenFile::open(&path)?;
        file.text = "local".to_owned();
        fs::write(&path, "external")?;
        assert!(file.save().is_err());
        assert!(file.is_dirty());
        assert_eq!(file.text, "local");
        assert_eq!(fs::read_to_string(&path)?, "external");
        Ok(())
    }
    #[test]
    fn moving_an_open_file_keeps_unsaved_edits_and_updates_recovery_path() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let original = directory.path().join("old.rs");
        let target = directory.path().join("new.rs");
        std::fs::write(&original, "saved")?;
        let mut file = OpenFile::open(&original)?;
        file.text = "unsaved".into();
        file.sync_edit().map_err(io::Error::other)?;
        std::fs::rename(&original, &target)?;
        file.relocate(&target)?;
        assert_eq!(file.text, "unsaved");
        assert!(file.is_dirty() && !file.external_change);
        assert_eq!(file.path, target.canonicalize()?);
        file.clear_checkpoint()?;
        Ok(())
    }
}
