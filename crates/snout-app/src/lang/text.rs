//! Between the editor's text (a rope, byte offsets, GPUI Kit positions whose columns count
//! characters) and the protocol's positions (UTF-16 columns).

use gpui_component::input::{Rope, RopeExt as _};
use lsp_types::{Position, Range, TextEdit};

/// The protocol position of byte `offset`.
pub fn to_lsp(rope: &Rope, offset: usize) -> Position {
    let offset = offset.min(rope.len());
    let row = rope.offset_to_point(offset).row;
    let start = rope.line_start_offset(row);
    Position::new(row as u32, rope.slice(start..offset).len_utf16() as u32)
}

/// The byte offset of a protocol position, clamped into the text.
pub fn from_lsp(rope: &Rope, pos: Position) -> usize {
    let line = pos.line as usize;
    if line >= rope.lines_len() {
        return rope.len();
    }
    let start = rope.line_start_offset(line);
    let end = rope.line_end_offset(line);
    let slice = rope.slice(start..end);
    start + slice.utf16_to_byte_idx((pos.character as usize).min(slice.len_utf16()))
}

/// A protocol range as bytes.
pub fn range_from_lsp(rope: &Rope, r: Range) -> std::ops::Range<usize> {
    let (a, b) = (from_lsp(rope, r.start), from_lsp(rope, r.end));
    a.min(b)..a.max(b)
}

/// Bytes as a protocol range.
pub fn range_to_lsp(rope: &Rope, r: &std::ops::Range<usize>) -> Range {
    Range::new(to_lsp(rope, r.start), to_lsp(rope, r.end))
}

/// A protocol position as GPUI Kit counts it (columns in characters).
pub fn to_kit(rope: &Rope, pos: Position) -> Position {
    rope.offset_to_position(from_lsp(rope, pos))
}

/// A protocol range as GPUI Kit counts it.
pub fn range_to_kit(rope: &Rope, r: Range) -> Range {
    Range::new(to_kit(rope, r.start), to_kit(rope, r.end))
}

/// The edit that turns `old` into `new`: the replaced bytes of `old` and the new text. `None`
/// when they are the same. Compares the ropes' chunks from both ends; nothing is copied
/// but the changed part.
pub fn change(old: &Rope, new: &Rope) -> Option<(std::ops::Range<usize>, String)> {
    let prefix = snout_lang::text::common_prefix(old.chunks(), new.chunks());
    let suffix =
        snout_lang::text::common_suffix(old.chunks_at(old.len()).0.reversed(), new.chunks_at(new.len()).0.reversed());
    let boundary = |mut i: usize| {
        while i > 0 && !new.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let (old_range, new_range) = snout_lang::text::edit_between(old.len(), new.len(), prefix, suffix, boundary)?;
    Some((old_range, new.slice(new_range).to_string()))
}

/// `edits` (protocol ranges against `rope`) as byte ranges, last first, so applying them in
/// order leaves the earlier ranges valid.
pub fn edits_from_lsp(rope: &Rope, edits: &[TextEdit]) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out: Vec<_> = edits.iter().map(|e| (range_from_lsp(rope, e.range), e.new_text.clone())).collect();
    out.sort_by(|a, b| b.0.start.cmp(&a.0.start).then(b.0.end.cmp(&a.0.end)));
    out
}

/// Apply `edits` to a plain string (a file no editor has open).
pub fn apply_to_string(text: &str, edits: &[TextEdit]) -> String {
    let rope = Rope::from(text);
    let mut out = text.to_owned();
    for (range, new_text) in edits_from_lsp(&rope, edits) {
        out.replace_range(range, &new_text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_utf16_and_kit_columns_count_characters() {
        let rope = Rope::from("a😀b\nxyz");
        // The emoji is two UTF-16 units, one character, four bytes.
        assert_eq!(to_lsp(&rope, 5), Position::new(0, 3));
        assert_eq!(from_lsp(&rope, Position::new(0, 3)), 5);
        assert_eq!(to_kit(&rope, Position::new(0, 3)), Position::new(0, 2));
        assert_eq!(from_lsp(&rope, Position::new(1, 99)), rope.len(), "a column past the end clamps");
        assert_eq!(from_lsp(&rope, Position::new(9, 0)), rope.len());
        assert_eq!(to_lsp(&rope, rope.len()), Position::new(1, 3));
    }

    #[test]
    fn finds_the_change_between_two_ropes() {
        let a = Rope::from("class A {\n  int x;\n}\n");
        let b = Rope::from("class A {\n  int xy;\n}\n");
        assert_eq!(change(&a, &b), Some((17..17, "y".to_owned())));
        assert_eq!(change(&a, &a.clone()), None);
        let edits = vec![
            TextEdit::new(Range::new(Position::new(0, 0), Position::new(0, 0)), "import x;\n".into()),
            TextEdit::new(Range::new(Position::new(1, 6), Position::new(1, 7)), "y".into()),
        ];
        assert_eq!(apply_to_string("class A {\n  int x;\n}\n", &edits), "import x;\nclass A {\n  int y;\n}\n");
    }
}
