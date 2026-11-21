use std::ops::Range;

const LIMIT: usize = 500;

#[derive(Clone)]
pub struct Selections {
    needle: String,
    pub ranges: Vec<Range<usize>>,
    next: usize,
}

impl Selections {
    pub fn new(text: &str, selection: Range<usize>) -> Option<Self> {
        let range = if selection.is_empty() {
            let offset = selection.start;
            let left = text
                .get(..offset)?
                .chars()
                .rev()
                .take_while(|c| word(*c))
                .map(char::len_utf8)
                .sum::<usize>();
            let right = text
                .get(offset..)?
                .chars()
                .take_while(|c| word(*c))
                .map(char::len_utf8)
                .sum::<usize>();
            offset - left..offset + right
        } else {
            selection
        };
        let needle = text.get(range.clone())?;
        if needle.is_empty() || needle.contains(['\r', '\n']) {
            return None;
        }
        Some(Self {
            needle: needle.into(),
            next: range.end,
            ranges: vec![range],
        })
    }

    pub fn rectangle(text: &str, selection: Range<usize>) -> Option<Self> {
        use forge_text::position::{Encoding, LineIndex, Position};
        let index = LineIndex::new(text);
        let start = index.from_byte(selection.start, Encoding::Utf32).ok()?;
        let end = index.from_byte(selection.end, Encoding::Utf32).ok()?;
        if start.line == end.line || end.line - start.line >= LIMIT {
            return None;
        }
        let left = start.character.min(end.character);
        let right = start.character.max(end.character);
        let mut ranges = Vec::new();
        for line in start.line..=end.line {
            let offset = index
                .to_byte(Position { line, character: 0 }, Encoding::Utf32)
                .ok()?;
            let contents = text[offset..].split(['\r', '\n']).next()?;
            let byte = |column| {
                offset
                    + contents
                        .char_indices()
                        .nth(column)
                        .map_or(contents.len(), |(offset, _)| offset)
            };
            ranges.push(byte(left)..byte(right));
        }
        Some(Self {
            needle: String::new(),
            ranges,
            next: 0,
        })
    }

    pub fn delete(&mut self, text: &str, backwards: bool) -> Option<String> {
        let mut candidate = self.clone();
        for range in &mut candidate.ranges {
            if range.start == range.end {
                if backwards {
                    range.start = text
                        .get(..range.start)?
                        .char_indices()
                        .next_back()
                        .map_or(range.start, |(offset, _)| offset);
                } else if let Some(character) = text.get(range.end..)?.chars().next() {
                    range.end += character.len_utf8();
                }
            }
        }
        let updated = candidate.replace(text, "")?;
        *self = candidate;
        Some(updated)
    }

    pub fn add_next(&mut self, text: &str) -> bool {
        if self.needle.is_empty() || self.ranges.len() >= LIMIT {
            return false;
        }
        let needle_len = self.needle.len();
        let next = text
            .get(self.next..)
            .into_iter()
            .flat_map(|tail| {
                tail.match_indices(&self.needle)
                    .map(|(start, _)| start + self.next)
            })
            .chain(text.match_indices(&self.needle).map(|(start, _)| start))
            .map(|start| start..start + needle_len)
            .find(|candidate| {
                !self
                    .ranges
                    .iter()
                    .any(|range| candidate.start < range.end && range.start < candidate.end)
            });
        if let Some(range) = next {
            self.next = range.end;
            self.ranges.push(range);
            self.ranges.sort_unstable_by_key(|range| range.start);
            true
        } else {
            false
        }
    }

    pub fn replace(&mut self, text: &str, replacement: &str) -> Option<String> {
        let target_len = text
            .len()
            .checked_sub(self.ranges.iter().map(Range::len).sum())?
            .checked_add(replacement.len().checked_mul(self.ranges.len())?)?;
        if target_len > 10 * 1024 * 1024 || replacement.contains('\0') {
            return None;
        }
        let mut cursor = 0;
        let mut output = String::with_capacity(target_len);
        let mut ranges = Vec::with_capacity(self.ranges.len());
        for range in &self.ranges {
            output.push_str(text.get(cursor..range.start)?);
            cursor = range.end;
            output.push_str(replacement);
            ranges.push(output.len()..output.len());
        }
        output.push_str(text.get(cursor..)?);
        ranges.dedup();
        self.ranges = ranges;
        Some(output)
    }
}

pub fn bytes(text: &str, selection: Range<usize>) -> Range<usize> {
    let byte = |index| {
        text.char_indices()
            .nth(index)
            .map_or(text.len(), |(offset, _)| offset)
    };
    byte(selection.start)..byte(selection.end)
}

fn word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

pub fn indentation(text: &str, offset: usize) -> String {
    let prefix = &text[..offset];
    let line = prefix.rsplit(['\r', '\n']).next().unwrap_or("");
    let line_ending = if text.contains("\r\n") {
        "\r\n"
    } else if !text.contains('\n') && text.contains('\r') {
        "\r"
    } else {
        "\n"
    };
    let mut result = line_ending.to_owned();
    result.extend(line.chars().take_while(|c| matches!(c, ' ' | '\t')));
    if line.trim_end().ends_with(['{', '(', '[']) {
        result.push_str(if result.contains('\t') { "\t" } else { "    " });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_unicode_occurrences_and_advances_each_cursor() {
        let text = "λ + λ + λ";
        let mut selections = Selections::new(text, 0..0).unwrap();
        assert!(selections.add_next(text));
        assert!(selections.add_next(text));
        assert!(!selections.add_next(text));
        let text = selections.replace(text, "🦀").unwrap();
        assert_eq!(&text, "🦀 + 🦀 + 🦀");
        assert_eq!(selections.replace(&text, "!").unwrap(), "🦀! + 🦀! + 🦀!");
    }

    #[test]
    fn rectangular_edit_clamps_short_rows_and_deletes_unicode() {
        let text = "aλz\nx\nabc";
        let mut selections = Selections::rectangle(text, 1..9).unwrap();
        let updated = selections.replace(text, "🦀").unwrap();
        assert_eq!(updated, "a🦀z\nx🦀\na🦀c");
        assert_eq!(selections.delete(&updated, true).unwrap(), "az\nx\nac");
    }

    #[test]
    fn indentation_keeps_crlf_and_existing_tabs() {
        let text = "outer\r\n\tif ok {";
        assert_eq!(indentation(text, text.len()), "\r\n\t\t");
    }
}
