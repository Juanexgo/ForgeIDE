use crate::language_edit::{FileChange, Preview};
use std::time::{Duration, Instant};

struct Entry {
    inverse: Preview,
    group: Option<Instant>,
    when: Instant,
}

#[derive(Default)]
pub struct Timeline {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

pub fn inverse(preview: &Preview) -> Preview {
    Preview {
        title: preview.title,
        files: preview
            .files
            .iter()
            .map(|file| FileChange {
                path: file.path.clone(),
                before: file.after.clone(),
                after: file.before.clone(),
                snippets: vec![],
                edits: None,
            })
            .collect(),
    }
}

impl Timeline {
    pub fn record(&mut self, preview: &Preview, group: Option<Instant>) {
        if preview.files.iter().all(|file| file.before == file.after) {
            return;
        }
        let now = Instant::now();
        let previous = self.undo.last_mut().filter(|entry| {
            entry.inverse.files.len() == 1
                && preview.files.len() == 1
                && entry.inverse.files[0].path == preview.files[0].path
                && entry.inverse.files[0].before == preview.files[0].before
                && ((group.is_some() && entry.group == group)
                    || (group.is_none()
                        && entry.group.is_none()
                        && preview.title == "Typing"
                        && entry.inverse.title == "Typing"
                        && entry.when.elapsed() < Duration::from_millis(600)))
        });
        if let Some(previous) = previous {
            previous.inverse.files[0]
                .before
                .clone_from(&preview.files[0].after);
            previous.when = now;
        } else {
            self.undo.push(Entry {
                inverse: inverse(preview),
                group,
                when: now,
            });
        }
        self.redo.clear();
        let bytes = |entries: &[Entry]| {
            entries
                .iter()
                .flat_map(|entry| &entry.inverse.files)
                .map(|file| file.before.len() + file.after.len())
                .sum::<usize>()
        };
        while self.undo.len() > 1 && (self.undo.len() > 100 || bytes(&self.undo) > 32 * 1024 * 1024)
        {
            self.undo.remove(0);
        }
    }

    pub fn current(&self, redo: bool) -> Option<Preview> {
        let entries = if redo { &self.redo } else { &self.undo };
        entries.last().map(|entry| entry.inverse.clone())
    }

    pub fn completed(&mut self, redo: bool) {
        let entry = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(mut entry) = entry {
            entry.inverse = inverse(&entry.inverse);
            entry.group = None;
            if redo {
                self.undo.push(entry);
            } else {
                self.redo.push(entry);
            }
        }
    }
}

fn directory() -> std::io::Result<std::path::PathBuf> {
    let recovery = crate::recovery::directory()?;
    let path = recovery.parent().expect("data directory").join("History");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&path)?;
    Ok(path)
}

fn prefix(path: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hash);
    format!("{:016x}-", hash.finish())
}

pub fn archive(path: &std::path::Path, saved: &str) -> std::io::Result<()> {
    use std::io::Write;
    let directory = directory()?;
    let mut file = tempfile::Builder::new()
        .prefix(&prefix(path))
        .suffix(".journal")
        .tempfile_in(&directory)?;
    let text = saved.strip_prefix('\u{feff}').unwrap_or(saved);
    let record = crate::recovery::Record::new(path.to_owned(), saved.to_owned(), text.to_owned());
    file.write_all(&serde_json::to_vec(&record)?)?;
    file.as_file().sync_all()?;
    file.keep().map_err(|error| error.error)?;
    let entries = versions(path)?;
    for old in entries.into_iter().skip(20) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

pub fn versions(path: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut versions = Vec::new();
    let prefix = prefix(path);
    for entry in std::fs::read_dir(directory()?)?.take(20_000) {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with(&prefix)
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "journal")
            && entry.file_type()?.is_file()
        {
            versions.push((entry.metadata()?.modified()?, entry.path()));
        }
    }
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(versions.into_iter().map(|(_, path)| path).collect())
}
