use crate::file::OpenFile;
use forge_text::position::{Encoding, LineIndex, Position};
use serde_json::Value;
use std::{
    io,
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub before: String,
    pub after: String,
    pub snippets: Vec<(String, String)>,
    pub edits: Option<Vec<(Range<usize>, String)>>,
}

#[derive(Clone)]
pub struct Preview {
    pub title: &'static str,
    pub files: Vec<FileChange>,
}

pub fn rename(
    result: &Value,
    roots: &[PathBuf],
    current: (&str, i32),
    tabs: &[OpenFile],
) -> io::Result<Preview> {
    let mut groups = Vec::new();
    if let Some(changes) = result["changes"].as_object() {
        groups.extend(
            changes
                .iter()
                .map(|(uri, edits)| (uri.as_str(), None, edits)),
        );
    }
    if let Some(changes) = result["documentChanges"].as_array() {
        for change in changes {
            let doc = &change["textDocument"];
            let uri = doc["uri"]
                .as_str()
                .ok_or_else(|| io::Error::other("The rename includes a file operation."))?;
            groups.push((uri, doc["version"].as_i64(), &change["edits"]));
        }
    }

    if groups.is_empty() || groups.len() > 64 {
        return Err(io::Error::other(
            "Rename returned no edits or exceeds 64 files.",
        ));
    }

    let mut files: Vec<FileChange> = Vec::new();
    let mut total_edits = 0usize;
    let mut total_bytes = 0usize;
    for (uri, version, edits) in groups {
        if version.is_some_and(|version| uri != current.0 || version != i64::from(current.1)) {
            return Err(io::Error::other(
                "The language server referenced an outdated document version.",
            ));
        }
        let path = url::Url::parse(uri)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .ok_or_else(|| io::Error::other("Rename requires local file paths."))?
            .canonicalize()?;
        if !roots.iter().any(|root| path.starts_with(root))
            || files.iter().any(|file| file.path == path)
        {
            return Err(io::Error::other(
                "The rename contains an out-of-workspace or duplicate file.",
            ));
        }
        let text = load_text(tabs, &path)?;
        let edits = edits
            .as_array()
            .ok_or_else(|| io::Error::other("Invalid rename edits."))?;
        total_edits += edits.len();
        total_bytes += text.len();
        if total_edits > 500 || total_bytes > 16 * 1024 * 1024 {
            return Err(io::Error::other("The rename exceeds the preview limit."));
        }
        files.push(build_change(path, text, edits)?);
    }
    Ok(Preview {
        title: "Review rename",
        files,
    })
}

pub fn completion(path: PathBuf, text: &str, item: &Value, offset: usize) -> io::Result<Preview> {
    if !item["command"].is_null() {
        return Err(io::Error::other("This suggestion requires a command."));
    }
    let replacement = item["textEdit"]["newText"]
        .as_str()
        .or_else(|| item["insertText"].as_str())
        .or_else(|| item["label"].as_str())
        .ok_or_else(|| io::Error::other("The suggestion contains no text."))?;
    let expanded;
    let replacement = if item["insertTextFormat"] == 2 {
        expanded = crate::snippets::expand(replacement)?.text;
        expanded.as_str()
    } else {
        replacement
    };
    let edit = &item["textEdit"];
    let range = if edit.is_null() {
        let prefix = text
            .get(..offset)
            .ok_or_else(|| io::Error::other("Invalid cursor."))?;
        let identifier_bytes = prefix
            .chars()
            .rev()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        offset - identifier_bytes..offset
    } else {
        decode_range(
            &LineIndex::new(text),
            edit.get("range").unwrap_or(&edit["replace"]),
        )?
    };
    let mut edits = vec![(range, replacement.to_owned())];
    if !item["additionalTextEdits"].is_null() {
        let extra = item["additionalTextEdits"]
            .as_array()
            .filter(|edits| edits.len() <= 100)
            .ok_or_else(|| io::Error::other("Invalid additional completion edits."))?;
        let index = LineIndex::new(text);
        for edit in extra {
            let range = decode_range(&index, &edit["range"])?;
            if edits.iter().any(|(other, _)| {
                (range.start < other.end && other.start < range.end) || range.start == other.start
            }) {
                return Err(io::Error::other("Additional completion edits overlap."));
            }
            let replacement = edit["newText"]
                .as_str()
                .ok_or_else(|| io::Error::other("An edit has no replacement."))?;
            edits.push((range, replacement.to_owned()));
        }
    }
    Ok(Preview {
        title: "Review completion",
        files: vec![replace(path, text.to_owned(), edits)?],
    })
}

pub fn selected(
    preview: &Preview,
    selections: &std::collections::HashMap<PathBuf, Vec<bool>>,
) -> io::Result<Preview> {
    let mut files = Vec::new();
    for change in &preview.files {
        let selected = selections
            .get(&change.path)
            .ok_or_else(|| io::Error::other("Review selection is missing."))?;
        if let Some(edits) = &change.edits {
            if edits.len() != selected.len() {
                return Err(io::Error::other("Review selection changed."));
            }
            let edits: Vec<_> = edits
                .iter()
                .zip(selected)
                .filter(|(_, selected)| **selected)
                .map(|(edit, _)| edit.clone())
                .collect();
            if !edits.is_empty() {
                files.push(replace(change.path.clone(), change.before.clone(), edits)?);
            }
        } else if selected.first() == Some(&true) {
            files.push(change.clone());
        }
    }
    if files.is_empty() {
        return Err(io::Error::other("Select at least one change."));
    }
    Ok(Preview {
        title: preview.title,
        files,
    })
}

fn load_text(tabs: &[OpenFile], path: &Path) -> io::Result<String> {
    if let Some(file) = tabs.iter().find(|file| file.path == path) {
        Ok(file.text.clone())
    } else {
        Ok(OpenFile::open(path)?.text)
    }
}

fn build_change(path: PathBuf, text: String, values: &[Value]) -> io::Result<FileChange> {
    let index = LineIndex::new(&text);
    let edits = values
        .iter()
        .map(|value| {
            let range = decode_range(&index, &value["range"])?;
            let replacement = value["newText"]
                .as_str()
                .ok_or_else(|| io::Error::other("An edit has no replacement."))?;
            Ok((range, replacement.to_owned()))
        })
        .collect::<io::Result<Vec<_>>>()?;
    replace(path, text, edits)
}

pub fn replace(
    path: PathBuf,
    text: String,
    mut edits: Vec<(Range<usize>, String)>,
) -> io::Result<FileChange> {
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let original_edits = edits.clone();
    let mut previous_end = 0;
    let mut snippets = Vec::with_capacity(edits.len());
    let mut after = String::with_capacity(text.len());
    for (range, replacement) in edits {
        if range.start < previous_end
            || range.start > range.end
            || replacement.contains('\0')
            || replacement.len() > 8 * 1024
            || range.len() > 8 * 1024
        {
            return Err(io::Error::other(
                "A proposed edit overlaps or exceeds the preview limit.",
            ));
        }
        let original = text
            .get(range.clone())
            .ok_or_else(|| io::Error::other("An edit splits a character."))?;
        after.push_str(&text[previous_end..range.start]);
        after.push_str(&replacement);
        previous_end = range.end;
        snippets.push((original.to_owned(), replacement));
    }
    after.push_str(&text[previous_end..]);
    if after.len() > 10 * 1024 * 1024 {
        return Err(io::Error::other(
            "The proposed edit exceeds the file size limit.",
        ));
    }
    Ok(FileChange {
        path,
        before: text,
        after,
        snippets,
        edits: Some(original_edits),
    })
}

fn decode_range(index: &LineIndex<'_>, range: &Value) -> io::Result<Range<usize>> {
    let offset = |position: &Value| {
        let unit = |field: &str| {
            position[field]
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| io::Error::other("Invalid language server position."))
        };
        index
            .to_byte(
                Position {
                    line: unit("line")?,
                    character: unit("character")?,
                },
                Encoding::Utf16,
            )
            .map_err(io::Error::other)
    };
    Ok(offset(&range["start"])?..offset(&range["end"])?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_review_keeps_the_exact_duplicate_occurrence() {
        let path = PathBuf::from("duplicate.rs");
        let preview = Preview {
            title: "Replace",
            files: vec![
                replace(
                    path.clone(),
                    "same same same".into(),
                    vec![(5..9, "second".into()), (10..14, "third".into())],
                )
                .unwrap(),
            ],
        };
        let selection = std::collections::HashMap::from([(path.clone(), vec![false, true])]);
        let result = selected(&preview, &selection).unwrap();
        assert_eq!(result.files[0].after, "same same third");
        assert_eq!(preview.files[0].before, "same same same");
        assert!(
            selected(
                &preview,
                &std::collections::HashMap::from([(path, vec![false, false])])
            )
            .is_err()
        );
    }

    #[test]
    fn completion_previews_auto_import_and_rejects_overlap() {
        let path = PathBuf::from("src/main.ts");
        let text = "\ncon";
        let mut item = serde_json::json!({
            "insertText": "connect",
            "additionalTextEdits": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "newText": "import { connect } from './db';"
            }]
        });
        let preview = completion(path.clone(), text, &item, text.len()).unwrap();
        assert_eq!(
            preview.files[0].after,
            "import { connect } from './db';\nconnect"
        );
        item["additionalTextEdits"][0]["range"]["end"] =
            serde_json::json!({"line": 1, "character": 1});
        assert!(completion(path, text, &item, text.len()).is_err());
    }

    #[test]
    fn completion_respects_utf16_character_boundaries() {
        let path = PathBuf::from("sample.rs");
        let item = json!({"textEdit": {"range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 4}}, "newText": "answer"}});
        assert_eq!(
            completion(path.clone(), "🦀an", &item, 6).unwrap().files[0].after,
            "🦀answer"
        );
        let mut malformed = item;
        malformed["textEdit"]["range"]["start"]["character"] = json!(1);
        assert!(completion(path, "🦀an", &malformed, 6).is_err());
    }
}
