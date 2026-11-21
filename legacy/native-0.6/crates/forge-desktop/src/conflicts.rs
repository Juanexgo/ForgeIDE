use crate::language_edit::{self, Preview};
use eframe::egui;
use std::{io, ops::Range, path::Path};

struct Conflict {
    range: Range<usize>,
    ours: String,
    theirs: String,
}

struct PendingConflict {
    start: usize,
    ours_start: usize,
    ours_end: Option<usize>,
    theirs_start: Option<usize>,
    base_start: Option<usize>,
}

fn parse(source: &str) -> io::Result<Vec<Conflict>> {
    let mut conflicts = Vec::new();
    let mut current: Option<PendingConflict> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let marker = line.trim_end_matches(['\r', '\n']);
        if marker.starts_with("<<<<<<< ") {
            if current.is_some() {
                return Err(io::Error::other(
                    "Nested conflict markers need manual editing.",
                ));
            }
            current = Some(PendingConflict {
                start: offset,
                ours_start: offset + line.len(),
                ours_end: None,
                theirs_start: None,
                base_start: None,
            });
        } else if let Some(PendingConflict {
            start,
            ours_start,
            ours_end,
            theirs_start,
            base_start,
        }) = current.as_mut()
        {
            if marker.starts_with("||||||| ") {
                if base_start.is_some() || theirs_start.is_some() {
                    return Err(io::Error::other("Invalid base conflict marker."));
                }
                *base_start = Some(offset);
                *ours_end = Some(offset);
            } else if marker == "=======" {
                if theirs_start.is_some() {
                    return Err(io::Error::other("Duplicate conflict separator."));
                }
                ours_end.get_or_insert(offset);
                *theirs_start = Some(offset + line.len());
            } else if marker.starts_with(">>>>>>> ") {
                let end =
                    ours_end.ok_or_else(|| io::Error::other("Conflict separator is missing."))?;
                let theirs = theirs_start
                    .ok_or_else(|| io::Error::other("Conflict separator is missing."))?;
                conflicts.push(Conflict {
                    range: *start..offset + line.len(),
                    ours: source[*ours_start..end].to_owned(),
                    theirs: source[theirs..offset].to_owned(),
                });
                current = None;
            }
        }
        offset += line.len();
    }
    if current.is_some() {
        return Err(io::Error::other(
            "An unfinished conflict marker needs manual editing.",
        ));
    }
    Ok(conflicts)
}

pub fn show(ui: &mut egui::Ui, path: &Path, source: &str) -> Option<io::Result<Preview>> {
    if source.len() > 512 * 1024 || !source.contains("<<<<<<< ") {
        return None;
    }
    let conflicts = match parse(source) {
        Ok(conflicts) => conflicts,
        Err(error) => {
            ui.colored_label(crate::chrome::theme_red(), error.to_string());
            return None;
        }
    };
    if conflicts.is_empty() {
        return None;
    }
    let mut choice = None;
    ui.collapsing(format!("Merge conflicts · {}", conflicts.len()), |ui| {
        egui::ScrollArea::vertical()
            .id_salt(("merge-conflicts", path))
            .max_height(240.0)
            .show(ui, |ui| {
                for (index, conflict) in conflicts.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.columns(2, |columns| {
                            columns[0].label("Current");
                            columns[1].label("Incoming");
                            columns[0].add(
                                egui::Label::new(egui::RichText::new(&conflict.ours).monospace())
                                    .selectable(true),
                            );
                            columns[1].add(
                                egui::Label::new(egui::RichText::new(&conflict.theirs).monospace())
                                    .selectable(true),
                            );
                        });
                        ui.horizontal(|ui| {
                            for (label, replacement) in [
                                ("Keep current", conflict.ours.clone()),
                                ("Keep incoming", conflict.theirs.clone()),
                                ("Keep both", format!("{}{}", conflict.ours, conflict.theirs)),
                            ] {
                                if ui.button(label).clicked() {
                                    choice = Some((conflict.range.clone(), replacement));
                                }
                            }
                        });
                        ui.separator();
                    });
                }
            });
    });
    choice.map(|edit| {
        language_edit::replace(path.to_owned(), source.to_owned(), vec![edit]).map(|change| {
            Preview {
                title: "Review conflict resolution",
                files: vec![change],
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff3_resolution_preserves_surroundings_and_crlf() {
        let text = "before\r\n<<<<<<< HEAD\r\nours\r\n||||||| base\r\nold\r\n=======\r\ntheirs\r\n>>>>>>> topic\r\nafter";
        let conflicts = parse(text).unwrap();
        assert_eq!(conflicts.len(), 1);
        let conflict = &conflicts[0];
        assert_eq!(conflict.ours, "ours\r\n");
        assert_eq!(conflict.theirs, "theirs\r\n");
        let result = language_edit::replace(
            "conflict.txt".into(),
            text.into(),
            vec![(conflict.range.clone(), conflict.theirs.clone())],
        )
        .unwrap();
        assert_eq!(result.after, "before\r\ntheirs\r\nafter");
        assert!(parse("<<<<<<< HEAD\nunfinished").is_err());
    }
}
