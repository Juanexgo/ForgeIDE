use crate::{file::OpenFile, multicursor};
use eframe::egui::{self, Key, Modifiers, TextEdit};
use std::{collections::BTreeMap, io, ops::Range};

pub struct Expansion {
    pub text: String,
    pub fields: Vec<Range<usize>>,
}

pub fn expand(template: &str) -> io::Result<Expansion> {
    let mut text = String::new();
    let mut fields: BTreeMap<u32, (Range<usize>, String)> = BTreeMap::new();
    let mut cursor = 0;
    while cursor < template.len() {
        let remaining = &template[cursor..];
        let character = remaining.chars().next().expect("nonempty suffix");
        if character == '\\' {
            let escaped = remaining[1..]
                .chars()
                .next()
                .ok_or_else(|| io::Error::other("Incomplete snippet escape."))?;
            if !matches!(escaped, '\\' | '$' | '}') {
                return Err(io::Error::other("Unsupported snippet escape."));
            }
            text.push(escaped);
            cursor += 1 + escaped.len_utf8();
        } else if character == '$' {
            cursor += 1;
            let braced = template[cursor..].starts_with('{');
            if braced {
                cursor += 1;
            }
            let start = cursor;
            while template
                .as_bytes()
                .get(cursor)
                .is_some_and(u8::is_ascii_digit)
            {
                cursor += 1;
            }
            let number = template[start..cursor]
                .parse::<u32>()
                .ok()
                .filter(|number| *number <= 100)
                .ok_or_else(|| {
                    io::Error::other("Snippet variables and transforms require an extension.")
                })?;
            let mut value = String::new();
            if braced {
                match template.as_bytes().get(cursor) {
                    Some(b':') => {
                        cursor += 1;
                        loop {
                            let character = template[cursor..]
                                .chars()
                                .next()
                                .ok_or_else(|| io::Error::other("Unclosed snippet field."))?;
                            cursor += character.len_utf8();
                            match character {
                                '}' => break,
                                '$' | '{' => {
                                    return Err(io::Error::other(
                                        "Nested snippet fields require an extension.",
                                    ));
                                }
                                '\\' => {
                                    let escaped =
                                        template[cursor..].chars().next().ok_or_else(|| {
                                            io::Error::other("Incomplete snippet escape.")
                                        })?;
                                    value.push(escaped);
                                    cursor += escaped.len_utf8();
                                }
                                _ => value.push(character),
                            }
                        }
                    }
                    Some(b'}') => cursor += 1,
                    Some(b'|') => {
                        cursor += 1;
                        let end = template[cursor..]
                            .find("|}")
                            .ok_or_else(|| io::Error::other("Unclosed snippet choice."))?;
                        value = template[cursor..cursor + end]
                            .split(',')
                            .next()
                            .unwrap_or("")
                            .to_owned();
                        cursor += end + 2;
                    }
                    _ => return Err(io::Error::other("Unsupported snippet field.")),
                }
            }
            if let Some((_, previous)) = fields.get(&number) {
                text.push_str(previous);
            } else {
                let start = text.len();
                text.push_str(&value);
                fields.insert(number, (start..text.len(), value));
            }
        } else {
            text.push(character);
            cursor += character.len_utf8();
        }
        if text.len() > 8 * 1024 {
            return Err(io::Error::other("Snippet exceeds 8 KiB."));
        }
    }
    let final_field = fields.remove(&0).map(|(range, _)| range);
    let mut fields: Vec<_> = fields.into_values().map(|(range, _)| range).collect();
    fields.extend(final_field);
    Ok(Expansion { text, fields })
}

pub struct Live {
    editor: egui::Id,
    source: String,
    fields: Vec<Range<usize>>,
    active: usize,
}

fn select(context: &egui::Context, editor: egui::Id, source: &str, range: Range<usize>) {
    let mut state = TextEdit::load_state(context, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text_selection::CCursorRange::two(
            egui::text::CCursor::new(source[..range.start].chars().count()),
            egui::text::CCursor::new(source[..range.end].chars().count()),
        )));
    TextEdit::store_state(context, editor, state);
    context.memory_mut(|memory| memory.request_focus(editor));
}

impl Live {
    pub fn sync(&mut self, source: &str) -> bool {
        if source == self.source {
            return true;
        }
        let old = &self.source;
        let prefix_chars = old
            .chars()
            .zip(source.chars())
            .take_while(|(a, b)| a == b)
            .count();
        let start = old
            .char_indices()
            .nth(prefix_chars)
            .map_or(old.len(), |(index, _)| index);
        let common = old[start..]
            .chars()
            .rev()
            .zip(source[start..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(character, _)| character.len_utf8())
            .sum::<usize>();
        let old_end = old.len() - common;
        let field = &self.fields[self.active];
        if start < field.start || old_end > field.end {
            return false;
        }
        let delta = source.len() as isize - old.len() as isize;
        for (index, range) in self.fields.iter_mut().enumerate() {
            if index == self.active {
                range.end = range.end.saturating_add_signed(delta);
            } else if range.start >= old_end {
                range.start = range.start.saturating_add_signed(delta);
                range.end = range.end.saturating_add_signed(delta);
            }
        }
        self.source = source.to_owned();
        true
    }
}

pub fn sync(live: &mut Option<Live>, id: egui::Id, source: &str) {
    if let Some(session) = live.as_mut().filter(|session| session.editor == id) {
        if !session.sync(source) {
            *live = None;
        }
    }
}

pub fn keyboard(
    context: &egui::Context,
    id: egui::Id,
    file: &mut OpenFile,
    live: &mut Option<Live>,
    library: &crate::packages::Library,
) -> bool {
    if let Some(session) = live.as_mut().filter(|session| session.editor == id) {
        if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            *live = None;
        } else {
            let backwards =
                context.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::Tab));
            if backwards || context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Tab))
            {
                if backwards {
                    session.active = session.active.saturating_sub(1);
                } else {
                    session.active += 1;
                }
                if let Some(range) = session.fields.get(session.active).cloned() {
                    select(context, id, &file.text, range);
                } else {
                    *live = None;
                }
            }
        }
        return false;
    }
    let Some(cursor) =
        TextEdit::load_state(context, id).and_then(|state| state.cursor.char_range())
    else {
        return false;
    };
    if cursor.primary.index != cursor.secondary.index
        || !context.input(|input| input.key_pressed(Key::Tab) && input.modifiers == Modifiers::NONE)
    {
        return false;
    }
    let offset = multicursor::bytes(&file.text, cursor.primary.index..cursor.primary.index).start;
    let prefix = &file.text[..offset];
    let token = prefix
        .rsplit(|character: char| !character.is_alphanumeric() && character != '_')
        .next()
        .unwrap_or("");
    let extension = file
        .path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let template = library
        .snippet(extension, token)
        .or(match (extension, token) {
            ("rs", "fn") => Some("fn ${1:name}(${2}) {\n    ${0}\n}"),
            ("rs", "test") => Some("#[test]\nfn ${1:test_name}() {\n    ${0}\n}"),
            ("ts" | "tsx" | "js" | "jsx", "fn") => Some("function ${1:name}(${2}) {\n    ${0}\n}"),
            ("ts" | "tsx" | "js" | "jsx", "log") => Some("console.log(${1:value});${0}"),
            ("py", "def") => Some("def ${1:name}(${2}):\n    ${0:pass}"),
            ("py", "if") => Some("if ${1:condition}:\n    ${0:pass}"),
            ("rs" | "ts" | "tsx" | "js" | "jsx", "if") => Some("if ${1:condition} {\n    ${0}\n}"),
            _ => None,
        });
    let Some(template) = template else {
        return false;
    };
    let indent: String = prefix
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect();
    let Ok(expansion) = expand(&template.replace('\n', &format!("\n{indent}"))) else {
        return false;
    };
    let start = offset - token.len();
    if file.text.len() - token.len() + expansion.text.len() > 10 * 1024 * 1024 {
        return false;
    }
    context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Tab));
    file.text.replace_range(start..offset, &expansion.text);
    let fields: Vec<_> = expansion
        .fields
        .into_iter()
        .map(|range| range.start + start..range.end + start)
        .collect();
    if let Some(first) = fields.first() {
        select(context, id, &file.text, first.clone());
        *live = Some(Live {
            editor: id,
            source: file.text.clone(),
            fields,
            active: 0,
        });
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_keep_unicode_offsets_and_follow_edits() {
        let expansion = expand("λ(${1:name}, ${2|one,two|})$0").unwrap();
        assert_eq!(expansion.text, "λ(name, one)");
        assert_eq!(&expansion.text[expansion.fields[0].clone()], "name");
        let mut live = Live {
            editor: egui::Id::NULL,
            source: expansion.text,
            fields: expansion.fields,
            active: 0,
        };
        assert!(live.sync("λ(🦀, one)"));
        assert_eq!(&live.source[live.fields[1].clone()], "one");
        assert!(expand("${1:name").is_err());
        assert!(expand("${TM_FILENAME}").is_err());
    }
}
