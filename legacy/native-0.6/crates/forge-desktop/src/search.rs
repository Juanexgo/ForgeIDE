use crate::{
    find,
    language_edit::{self, Preview},
};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const FILE_LIMIT: u64 = 512 * 1024;
const SEARCH_BUDGET: u64 = 64 * 1024 * 1024;
const REVIEW_BUDGET: usize = 16 * 1024 * 1024;
const RESULT_LIMIT: usize = 200;
const REVIEW_FILE_LIMIT: usize = 64;
const REVIEW_EDIT_LIMIT: usize = 500;

pub struct Match {
    pub path: PathBuf,
    pub line: usize,
    pub excerpt: String,
}

#[derive(Default)]
pub struct Results {
    pub matches: Vec<Match>,
    pub capped: bool,
    pub skipped_files: usize,
}

pub struct Request {
    pub query: String,
    pub regex: bool,
    pub ignore_case: bool,
    pub path_filter: String,
    pub replacement: Option<String>,
}

pub enum Outcome {
    Search(Results),
    Review {
        preview: Preview,
        skipped_files: usize,
    },
}

pub fn run(
    roots: &[PathBuf],
    paths: &[PathBuf],
    buffers: &HashMap<PathBuf, Option<String>>,
    request: Request,
    cancelled: &AtomicBool,
) -> io::Result<Outcome> {
    let pattern = find::compile(&request.query, request.regex, request.ignore_case)?;
    let filter = request.path_filter.trim().to_lowercase();
    let mut candidates = HashSet::new();
    candidates.extend(
        paths
            .iter()
            .filter(|path| roots.iter().any(|root| matches_path(path, root, &filter))),
    );
    candidates.extend(
        buffers
            .keys()
            .filter(|path| roots.iter().any(|root| matches_path(path, root, &filter))),
    );
    let mut paths = candidates.into_iter().collect::<Vec<_>>();
    paths.sort_unstable();

    let mut results = Results::default();
    let mut searched = 0;
    let mut before_after_bytes = 0;
    let mut edit_count = 0;
    let mut changes = Vec::new();
    for path in paths {
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Search cancelled",
            ));
        }
        let Some(root) = roots
            .iter()
            .filter(|root| path.starts_with(root))
            .max_by_key(|root| root.components().count())
        else {
            continue;
        };
        let text = if let Some(text) = buffers.get(path) {
            // The editor enforces canonical paths, but the underlying path can
            // be replaced while an open workspace is active.
            text.as_deref()
                .filter(|_| canonical_file(root, path))
                .filter(|text| text.len() as u64 <= FILE_LIMIT && !text.contains('\0'))
                .map(std::borrow::Cow::Borrowed)
        } else {
            read_text(root, path).map(std::borrow::Cow::Owned)
        };
        let Some(text) = text else {
            results.skipped_files += 1;
            continue;
        };

        searched += text.len() as u64;
        if searched > SEARCH_BUDGET {
            if request.replacement.is_some() {
                return Err(io::Error::other(
                    "Search exceeded 64 MiB. Narrow the path filter before replacing.",
                ));
            }
            results.capped = true;
            break;
        }
        if let Some(replacement) = &request.replacement {
            let edits = find::replacement_edits(&text, &pattern, request.regex, replacement)?;
            if edits.is_empty() {
                continue;
            }
            edit_count += edits.len();
            if edit_count > REVIEW_EDIT_LIMIT || changes.len() == REVIEW_FILE_LIMIT {
                return Err(io::Error::other(
                    "Review is limited to 500 changes in 64 files. Narrow the search.",
                ));
            }
            let change = language_edit::replace(path.clone(), text.into_owned(), edits)?;
            before_after_bytes += change.before.len() + change.after.len();
            if before_after_bytes > REVIEW_BUDGET {
                return Err(io::Error::other(
                    "Review exceeded 16 MiB. Narrow the search.",
                ));
            }
            changes.push(change);
        } else if append_matches(&mut results, path, &text, &pattern) {
            break;
        }
    }
    if request.replacement.is_some() {
        if changes.is_empty() {
            return Err(io::Error::other("No changes to apply."));
        }
        Ok(Outcome::Review {
            preview: Preview {
                title: "Review workspace replacements",
                files: changes,
            },
            skipped_files: results.skipped_files,
        })
    } else {
        Ok(Outcome::Search(results))
    }
}

fn matches_path(path: &Path, root: &Path, filter: &str) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|relative| relative.to_string_lossy().to_lowercase().contains(filter))
}

fn append_matches(results: &mut Results, path: &Path, source: &str, pattern: &Regex) -> bool {
    let mut line = 1;
    let mut previous = 0;
    for matched in pattern.find_iter(source) {
        if results.matches.len() == RESULT_LIMIT {
            results.capped = true;
            return true;
        }
        line += source.as_bytes()[previous..matched.start()]
            .iter()
            .filter(|byte| **byte == b'\n')
            .count();
        previous = matched.start();
        let context = source[..matched.start()].rsplit('\n').next().unwrap_or("");
        let prefix = context
            .chars()
            .rev()
            .take(24)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        let excerpt = prefix
            .chars()
            .chain(source[matched.start()..].chars())
            .take(100)
            .map(|character| {
                if matches!(character, '\r' | '\n') {
                    '↵'
                } else {
                    character
                }
            })
            .collect();
        results.matches.push(Match {
            path: path.to_owned(),
            line,
            excerpt,
        });
    }
    false
}

fn canonical_file(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
        && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
        && path.canonicalize().is_ok_and(|canonical| canonical == path)
}

fn read_text(root: &Path, path: &Path) -> Option<String> {
    if !canonical_file(root, path) || fs::metadata(path).ok()?.len() > FILE_LIMIT {
        return None;
    }
    let mut source = String::new();
    File::open(path)
        .ok()?
        .take(FILE_LIMIT + 1)
        .read_to_string(&mut source)
        .ok()?;
    if source.len() as u64 > FILE_LIMIT || source.contains('\0') {
        return None;
    }
    if source.starts_with('\u{feff}') {
        source.drain(..'\u{feff}'.len_utf8());
    }
    Some(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(query: &str) -> Request {
        Request {
            query: query.into(),
            regex: true,
            ignore_case: true,
            path_filter: String::new(),
            replacement: None,
        }
    }

    #[test]
    fn matches_multiple_hits_and_multiline_patterns() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().canonicalize()?;
        let text = root.join("one.txt");
        let binary = root.join("data.bin");
        fs::write(&text, "skip\r\n🦀 needle NEEDLE\nMORE\n")?;
        fs::write(&binary, b"\0needle")?;
        let Outcome::Search(result) = run(
            std::slice::from_ref(&root),
            &[text.clone(), binary],
            &HashMap::new(),
            request(r"needle(?:\nmore)?"),
            &AtomicBool::new(false),
        )?
        else {
            panic!("expected matches");
        };
        assert_eq!(result.skipped_files, 1);
        assert_eq!(result.matches.len(), 2);
        assert!(
            result
                .matches
                .iter()
                .all(|item| item.path == text && item.line == 2)
        );
        Ok(())
    }

    #[test]
    fn review_uses_unsaved_buffers_restricts_paths_and_preserves_bom_on_disk() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let root = folder.path().canonicalize()?;
        let indexed = root.join("source.rs");
        let open = root.join("open.rs");
        let excluded = root.join("notes.txt");
        fs::write(&indexed, "\u{feff}let count = 41;")?;
        fs::write(&open, "let count = 42;")?;
        fs::write(&excluded, "let count = 43;")?;
        let mut options = request(r"count = (\d+)");
        options.path_filter = ".RS".into();
        options.replacement = Some("total = $1".into());
        let Outcome::Review {
            preview,
            skipped_files,
        } = run(
            std::slice::from_ref(&root),
            &[indexed.clone(), open.clone(), excluded],
            &HashMap::from([(open, Some("let count = 99;".into()))]),
            options,
            &AtomicBool::new(false),
        )?
        else {
            panic!("expected review");
        };
        assert_eq!(skipped_files, 0);
        assert_eq!(preview.files.len(), 2);
        assert_eq!(preview.files[0].after, "let total = 99;");
        assert_eq!(preview.files[1].before, "let count = 41;");
        assert_eq!(preview.files[1].after, "let total = 41;");
        assert_eq!(fs::read_to_string(indexed)?, "\u{feff}let count = 41;");
        Ok(())
    }
}
