//! The lines a diff shows, side by side or unified, optionally compacted: what to draw on
//! each screen line, leaving how to draw it to the front end.

use crate::{FileDiff, RowKind};

/// Unchanged lines kept around each change in compact mode.
pub const CONTEXT: usize = 3;

/// A screen line of the side-by-side view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbsLine {
    /// Row `i` of the diff: old on the left, new on the right.
    Row(usize),
    /// This many unchanged lines left out.
    Gap(usize),
}

/// A screen line of the unified view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniLine {
    Both(usize),
    Old(usize),
    New(usize),
    Gap(usize),
}

/// A diff prepared for display. Rebuild when the diff or the compact flag changes.
#[derive(Debug, Clone)]
pub struct DiffLayout {
    pub diff: FileDiff,
    pub sbs: Vec<SbsLine>,
    pub unified: Vec<UniLine>,
    /// Digits in the largest line number, for the gutter.
    pub number_digits: usize,
}

impl DiffLayout {
    pub fn new(diff: FileDiff, compact: bool) -> Self {
        let rows = &diff.rows;
        let keep = visible_rows(&diff, compact);

        let mut sbs = Vec::new();
        let mut unified = Vec::new();
        let mut i = 0;
        while i < rows.len() {
            if !keep[i] {
                let start = i;
                while i < rows.len() && !keep[i] {
                    i += 1;
                }
                sbs.push(SbsLine::Gap(i - start));
                unified.push(UniLine::Gap(i - start));
                continue;
            }
            sbs.push(SbsLine::Row(i));
            match rows[i].kind {
                RowKind::Equal => unified.push(UniLine::Both(i)),
                RowKind::Delete => unified.push(UniLine::Old(i)),
                RowKind::Insert => unified.push(UniLine::New(i)),
                RowKind::Replace => {
                    // Emit the whole replaced block as all-old then all-new.
                    let start = i;
                    let mut end = i;
                    while end < rows.len() && rows[end].kind == RowKind::Replace {
                        end += 1;
                    }
                    let block = rows[start..end].iter().enumerate();
                    unified
                        .extend(block.clone().filter(|(_, r)| r.old.is_some()).map(|(k, _)| UniLine::Old(start + k)));
                    unified.extend(block.filter(|(_, r)| r.new.is_some()).map(|(k, _)| UniLine::New(start + k)));
                    for k in start + 1..end {
                        sbs.push(SbsLine::Row(k));
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }

        let max_line = rows
            .iter()
            .flat_map(|r| [r.old.as_ref().map(|l| l.0), r.new.as_ref().map(|l| l.0)])
            .flatten()
            .max()
            .unwrap_or(1);
        Self { diff, sbs, unified, number_digits: max_line.to_string().len() }
    }

    /// Screen lines in the chosen view.
    pub fn len(&self, side_by_side: bool) -> usize {
        if side_by_side { self.sbs.len() } else { self.unified.len() }
    }

    pub fn is_empty(&self) -> bool {
        self.sbs.is_empty()
    }
}

/// The text of a gap line.
pub fn gap_label(n: usize) -> String {
    format!("⋯ {n} unchanged line{}", if n == 1 { "" } else { "s" })
}

fn visible_rows(diff: &FileDiff, compact: bool) -> Vec<bool> {
    let n = diff.rows.len();
    if !compact {
        return vec![true; n];
    }
    let changed: Vec<bool> = diff.rows.iter().map(|r| r.kind != RowKind::Equal).collect();
    let mut keep = vec![false; n];
    let mut since = usize::MAX;
    for i in 0..n {
        since = if changed[i] { 0 } else { since.saturating_add(1) };
        keep[i] = since <= CONTEXT;
    }
    let mut until = usize::MAX;
    for i in (0..n).rev() {
        until = if changed[i] { 0 } else { until.saturating_add(1) };
        keep[i] |= until <= CONTEXT;
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiffRow;

    fn row(kind: RowKind, n: u32) -> DiffRow {
        let line = Some((n, format!("line {n}")));
        let (old, new) = match kind {
            RowKind::Insert => (None, line),
            RowKind::Delete => (line, None),
            _ => (line.clone(), line),
        };
        DiffRow { kind, old, new }
    }

    #[test]
    fn compact_keeps_context_around_changes() {
        let mut rows: Vec<DiffRow> = (1..=20).map(|n| row(RowKind::Equal, n)).collect();
        rows[10] = row(RowKind::Insert, 11);
        let diff = FileDiff {
            path: "a".into(),
            base: None,
            binary: false,
            too_large: false,
            rows,
            additions: 1,
            deletions: 0,
        };
        let full = DiffLayout::new(diff.clone(), false);
        assert_eq!(full.len(true), 20);
        let compact = DiffLayout::new(diff, true);
        assert_eq!(compact.sbs.first(), Some(&SbsLine::Gap(7)));
        assert_eq!(compact.sbs.last(), Some(&SbsLine::Gap(6)));
        assert_eq!(compact.len(true), 2 + 2 * CONTEXT + 1);
        assert_eq!(compact.number_digits, 2);
        assert_eq!(gap_label(1), "⋯ 1 unchanged line");
    }
}
