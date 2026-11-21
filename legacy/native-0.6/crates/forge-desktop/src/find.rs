use crate::{
    file::OpenFile,
    language_edit::{self, Preview},
};
use eframe::egui::{self, Key, RichText, TextEdit};
use regex::{Regex, RegexBuilder};
use std::{io, ops::Range};

const MAX_REPLACEMENTS: usize = 500;

#[derive(Default)]
pub struct Panel {
    open: bool,
    replace: bool,
    query: String,
    replacement: String,
    regex: bool,
    ignore_case: bool,
    selection: Option<Range<usize>>,
    focus: bool,
    go_next: bool,
    error: Option<String>,
}

pub struct Outcome {
    pub selection: Option<Range<usize>>,
    pub preview: Option<Preview>,
}

impl Panel {
    pub fn open(&mut self, replace: bool) {
        self.open = true;
        self.replace = replace;
        self.focus = true;
    }

    pub fn next(&mut self) {
        if self.open {
            self.go_next = true;
        }
    }

    pub fn reset_selection(&mut self) {
        self.selection = None;
        self.error = None;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, file: &OpenFile) -> Outcome {
        let mut result = Outcome {
            selection: None,
            preview: None,
        };
        if !self.open {
            return result;
        }
        let mut review = false;
        let mut next = std::mem::take(&mut self.go_next);
        let mut previous = false;
        crate::chrome::panel(crate::chrome::theme_panel(), 12, 8).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let field = ui.add(
                    TextEdit::singleline(&mut self.query)
                        .hint_text("Find in file")
                        .desired_width((ui.available_width() * 0.42).clamp(100.0, 260.0)),
                );
                if std::mem::take(&mut self.focus) {
                    field.request_focus();
                }
                let case = ui.toggle_value(&mut self.ignore_case, "Ignore case");
                let regex = ui.toggle_value(&mut self.regex, "Regex");
                if field.changed() || case.changed() || regex.changed() {
                    self.reset_selection();
                    next = true;
                }
                next |= (field.has_focus() || field.lost_focus())
                    && ui.input(|input| input.key_pressed(Key::Enter));
                previous = ui.button("↑").on_hover_text("Previous match").clicked();
                next |= ui.button("↓").on_hover_text("Next match").clicked();
                ui.toggle_value(&mut self.replace, "Replace");
                if crate::chrome::icon_button(
                    ui,
                    crate::chrome::Icon::Close,
                    "Close Find",
                    false,
                    24.0,
                )
                .clicked()
                {
                    self.open = false;
                }
            });
            if self.replace {
                ui.horizontal_wrapped(|ui| {
                    let field = ui.add(
                        TextEdit::singleline(&mut self.replacement)
                            .hint_text("Replace with")
                            .desired_width((ui.available_width() * 0.42).clamp(100.0, 260.0)),
                    );
                    if field.changed() {
                        self.error = None;
                    }
                    review = ui
                        .add_enabled(
                            !self.query.is_empty(),
                            egui::Button::new("Review Replace All")
                                .fill(crate::chrome::theme_active()),
                        )
                        .clicked();
                });
            }
            if let Some(error) = &self.error {
                ui.label(
                    RichText::new(error)
                        .size(11.0)
                        .color(crate::chrome::theme_red()),
                );
            }
        });

        if !self.open || self.query.is_empty() || !(next || previous || review) {
            return result;
        }
        self.error = None;
        let action = (|| {
            let pattern = compile(&self.query, self.regex, self.ignore_case)?;
            if review {
                result.preview = Some(replacement_preview(
                    file,
                    &pattern,
                    self.regex,
                    &self.replacement,
                )?);
            } else {
                let range = navigate(&pattern, &file.text, self.selection.as_ref(), previous)
                    .ok_or_else(|| io::Error::other("No matches"))?;
                self.selection = Some(range.clone());
                result.selection = Some(range);
            }
            Ok::<_, io::Error>(())
        })();
        if let Err(error) = action {
            self.error = Some(error.to_string());
        }
        result
    }
}

pub(crate) fn compile(query: &str, regex: bool, ignore_case: bool) -> io::Result<Regex> {
    if query.len() > 1024 {
        return Err(io::Error::other("Search exceeds 1 KiB."));
    }
    let literal;
    let expression = if regex {
        query
    } else {
        literal = regex::escape(query);
        &literal
    };
    RegexBuilder::new(expression)
        .case_insensitive(ignore_case)
        .size_limit(1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(io::Error::other)
}

fn navigate(
    pattern: &Regex,
    source: &str,
    selected: Option<&Range<usize>>,
    previous: bool,
) -> Option<Range<usize>> {
    let mut matches = pattern.find_iter(source).map(|item| item.range());
    if previous {
        if let Some(selected) = selected {
            let before = matches
                .by_ref()
                .take_while(|range| range.start < selected.start)
                .last();
            if before.is_some() {
                return before;
            }
        }
        pattern.find_iter(source).last().map(|item| item.range())
    } else if let Some(selected) = selected {
        let after = matches.find(|range| range.start > selected.start);
        after.or_else(|| pattern.find(source).map(|item| item.range()))
    } else {
        matches.next()
    }
}

fn replacement_preview(
    file: &OpenFile,
    pattern: &Regex,
    expand_captures: bool,
    replacement: &str,
) -> io::Result<Preview> {
    let edits = replacement_edits(&file.text, pattern, expand_captures, replacement)?;
    if edits.is_empty() {
        return Err(io::Error::other("No changes to apply."));
    }
    Ok(Preview {
        title: "Review replacements",
        files: vec![language_edit::replace(
            file.path.clone(),
            file.text.clone(),
            edits,
        )?],
    })
}

pub(crate) type Replacement = (Range<usize>, String);

pub(crate) fn replacement_edits(
    text: &str,
    pattern: &Regex,
    expand_captures: bool,
    replacement: &str,
) -> io::Result<Vec<Replacement>> {
    if replacement.len() > 8 * 1024 {
        return Err(io::Error::other("Replacement exceeds 8 KiB."));
    }
    let mut edits = Vec::new();
    let mut delta = 0_i64;
    for (index, capture) in pattern.captures_iter(text).enumerate() {
        if index == MAX_REPLACEMENTS {
            return Err(io::Error::other(
                "More than 500 matches. Narrow the search to review every replacement.",
            ));
        }
        let matched = capture.get(0).expect("full regex match");
        let value = if expand_captures {
            let mut rendered = String::new();
            capture.expand(replacement, &mut rendered);
            rendered
        } else {
            replacement.to_owned()
        };
        if value == matched.as_str() {
            continue;
        }
        delta += value.len() as i64 - matched.len() as i64;
        if value.len() > 8 * 1024 || text.len() as i64 + delta > 10 * 1024 * 1024 {
            return Err(io::Error::other(
                "Replacement exceeds the editor size limit.",
            ));
        }
        edits.push((matched.range(), value));
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_wraps_after_empty_matches() {
        let pattern = compile(r"(?m)^", true, false).unwrap();
        let source = "🦀\nback";
        let first = navigate(&pattern, source, None, false).unwrap();
        assert_eq!(first, 0..0);
        let second = navigate(&pattern, source, Some(&first), false).unwrap();
        assert_eq!(second, 5..5);
        assert_eq!(
            navigate(&pattern, source, Some(&second), false),
            Some(first.clone())
        );
        assert_eq!(navigate(&pattern, source, Some(&first), true), Some(second));
    }

    #[test]
    fn replacement_expands_captures_only_in_regex_mode() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let file = OpenFile::create_at(&folder.path().join("test.rs"), "α=12", false)?;
        let regex = compile(r"(\w+)=(\d+)", true, false)?;
        let preview = replacement_preview(&file, &regex, true, "$2:$1")?;
        assert_eq!(preview.files[0].after, "12:α");
        let literal = compile("=12", false, false)?;
        assert_eq!(
            replacement_preview(&file, &literal, false, "$1")?.files[0].after,
            "α$1"
        );
        Ok(())
    }
}
