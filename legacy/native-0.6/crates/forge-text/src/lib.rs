//! Revisioned UTF-8 text. Mutations are serialized by the document owner.
#![forbid(unsafe_code)]

pub mod position;

use ropey::Rope;
use std::{fmt, ops::Range};

pub type Revision = u64;

/// A replacement in pre-transaction UTF-8 byte coordinates.
#[derive(Debug)]
pub struct Edit<'a> {
    pub range: Range<usize>,
    pub replacement: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditError {
    Stale {
        expected: Revision,
        actual: Revision,
    },
    InvalidRange,
    OverlappingOrUnsorted,
    RevisionExhausted,
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { expected, actual } => write!(
                formatter,
                "expected revision {expected}, current revision {actual}"
            ),
            Self::InvalidRange => {
                formatter.write_str("edit range is outside the document or splits a UTF-8 scalar")
            }
            Self::OverlappingOrUnsorted => {
                formatter.write_str("edits must be sorted and non-overlapping")
            }
            Self::RevisionExhausted => formatter.write_str("document revision exhausted"),
        }
    }
}

impl std::error::Error for EditError {}

#[derive(Clone)]
pub struct Snapshot {
    revision: Revision,
    text: Rope,
}

impl Snapshot {
    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        self.text.chunks()
    }
}

#[derive(Clone)]
pub struct Document {
    current: Snapshot,
}

impl Document {
    pub fn new(text: &str) -> Self {
        Self {
            current: Snapshot {
                revision: 0,
                text: Rope::from_str(text),
            },
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.current.clone()
    }

    /// Apply a batch atomically. Adjacent ranges and ordered insertions at the
    /// same offset are allowed. An empty batch leaves the revision unchanged.
    pub fn apply(&mut self, expected: Revision, edits: &[Edit<'_>]) -> Result<Revision, EditError> {
        if expected != self.current.revision {
            return Err(EditError::Stale {
                expected,
                actual: self.current.revision,
            });
        }
        if edits.is_empty() {
            return Ok(expected);
        }

        let next_revision = expected
            .checked_add(1)
            .ok_or(EditError::RevisionExhausted)?;
        let text = &mut self.current.text;
        let mut previous_end = 0;
        let mut ranges = Vec::with_capacity(edits.len());
        for edit in edits {
            let Range { start, end } = edit.range;
            if start > end || end > text.len_bytes() {
                return Err(EditError::InvalidRange);
            }
            if start < previous_end {
                return Err(EditError::OverlappingOrUnsorted);
            }
            let start_char = text.byte_to_char(start);
            let end_char = text.byte_to_char(end);
            if text.char_to_byte(start_char) != start || text.char_to_byte(end_char) != end {
                return Err(EditError::InvalidRange);
            }
            ranges.push(start_char..end_char);
            previous_end = end;
        }

        for (edit, range) in edits.iter().zip(ranges).rev() {
            let start = range.start;
            text.remove(range);
            text.insert(start, edit.replacement);
        }
        self.current.revision = next_revision;
        Ok(next_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(range: Range<usize>, replacement: &str) -> Edit<'_> {
        Edit { range, replacement }
    }

    #[test]
    fn applies_pre_edit_coordinates_and_preserves_snapshots() {
        let mut doc = Document::new("a🦀\r\nb");
        let before = doc.snapshot();
        assert_eq!(doc.apply(0, &[edit(1..5, "Ω"), edit(7..8, "B")]), Ok(1));
        assert_eq!(doc.snapshot().chunks().collect::<String>(), "aΩ\r\nB");
        assert_eq!(before.chunks().collect::<String>(), "a🦀\r\nb");
        assert_eq!(before.revision(), 0);
        assert_eq!(doc.snapshot().revision(), 1);
    }

    #[test]
    fn rejects_invalid_or_overlapping_batches_atomically() {
        let mut doc = Document::new("a🦀b");
        assert_eq!(
            doc.apply(0, &[edit(0..1, "A"), edit(2..5, "")]),
            Err(EditError::InvalidRange)
        );
        assert_eq!(
            doc.apply(0, &[edit(0..5, ""), edit(1..6, "")]),
            Err(EditError::OverlappingOrUnsorted)
        );
        assert_eq!(
            doc.apply(0, &[edit(6..7, "")]),
            Err(EditError::InvalidRange)
        );
        assert_eq!(doc.snapshot().chunks().collect::<String>(), "a🦀b");
        assert_eq!(doc.snapshot().revision(), 0);
    }

    #[test]
    fn orders_insertions_and_rejects_stale_requests() {
        let mut doc = Document::new("");
        assert_eq!(doc.apply(0, &[]), Ok(0));
        assert_eq!(doc.apply(0, &[edit(0..0, "a"), edit(0..0, "b")]), Ok(1));
        assert_eq!(
            doc.apply(0, &[]),
            Err(EditError::Stale {
                expected: 0,
                actual: 1
            })
        );
        assert_eq!(doc.snapshot().chunks().collect::<String>(), "ab");
    }
}
