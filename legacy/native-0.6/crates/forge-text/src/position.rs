//! Line and code-unit coordinates for the Language Server Protocol.
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16,
    Utf32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidPosition;

impl fmt::Display for InvalidPosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("position is outside the document or splits a character or line ending")
    }
}

impl Error for InvalidPosition {}

/// Byte ranges exclude line endings. CRLF is one line ending; bare CR and LF
/// also end lines. The final line exists even when it is empty.
pub struct LineIndex<'a> {
    text: &'a str,
    lines: Vec<std::ops::Range<usize>>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut bytes = text.bytes().enumerate().peekable();
        let mut lines = Vec::new();
        let mut start = 0;
        while let Some((index, byte)) = bytes.next() {
            if matches!(byte, b'\n' | b'\r') {
                lines.push(start..index);
                let crlf = byte == b'\r' && bytes.next_if(|(_, next)| *next == b'\n').is_some();
                start = index + 1 + usize::from(crlf);
            }
        }
        lines.push(start..text.len());
        Self { text, lines }
    }

    pub fn to_byte(
        &self,
        position: Position,
        encoding: Encoding,
    ) -> Result<usize, InvalidPosition> {
        let line = self.lines.get(position.line).ok_or(InvalidPosition)?;
        let contents = &self.text[line.clone()];
        if encoding == Encoding::Utf8 {
            return contents
                .is_char_boundary(position.character)
                .then(|| line.start + position.character)
                .ok_or(InvalidPosition);
        }
        let mut units = 0;
        for (offset, character) in contents.char_indices() {
            if units == position.character {
                return Ok(line.start + offset);
            }
            units += if encoding == Encoding::Utf16 {
                character.len_utf16()
            } else {
                1
            };
        }
        (units == position.character)
            .then_some(line.end)
            .ok_or(InvalidPosition)
    }

    pub fn from_byte(
        &self,
        offset: usize,
        encoding: Encoding,
    ) -> Result<Position, InvalidPosition> {
        let line = self.lines.partition_point(|range| range.start <= offset) - 1;
        let range = &self.lines[line];
        if offset > range.end {
            return Err(InvalidPosition);
        }
        let prefix = self.text.get(range.start..offset).ok_or(InvalidPosition)?;
        let character = match encoding {
            Encoding::Utf8 => prefix.len(),
            Encoding::Utf16 => prefix.encode_utf16().count(),
            Encoding::Utf32 => prefix.chars().count(),
        };
        Ok(Position { line, character })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_round_trip_unicode_and_reject_split_boundaries() {
        let index = LineIndex::new("a🦀Z\r\nβ\rz\n");
        for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
            for byte in [0, 1, 5, 6, 8, 10, 11, 12, 13] {
                let position = index.from_byte(byte, encoding).unwrap();
                assert_eq!(index.to_byte(position, encoding), Ok(byte));
            }
            for byte in [2, 7, 9, 14] {
                assert_eq!(index.from_byte(byte, encoding), Err(InvalidPosition));
            }
        }
        assert_eq!(
            index.to_byte(
                Position {
                    line: 0,
                    character: 2
                },
                Encoding::Utf16
            ),
            Err(InvalidPosition)
        );
        assert_eq!(
            index.to_byte(
                Position {
                    line: 3,
                    character: 0
                },
                Encoding::Utf8
            ),
            Ok(13)
        );
    }
}
