//! The text side of reviewing a change Claude proposes, without any UI: the changed
//! line runs between two texts, the marks the editor draws for them, and the frames of
//! the change typing itself in, hunk by hunk. When the user types while it streams, the
//! edit is carried over to the proposed text, so nothing typed is lost.

use std::ops::Range;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

/// One run of changed lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// Bytes of the original the run replaces.
    pub old: Range<usize>,
    /// Bytes of the new text that take their place.
    pub new: Range<usize>,
    pub old_lines: Range<usize>,
    pub new_lines: Range<usize>,
}

fn line_starts(lines: &[&str]) -> Vec<usize> {
    let mut at = Vec::with_capacity(lines.len() + 1);
    let mut sum = 0;
    at.push(0);
    for l in lines {
        sum += l.len();
        at.push(sum);
    }
    at
}

/// The runs of lines that differ between `a` and `b`, in order.
pub fn hunks(a: &str, b: &str) -> Vec<Hunk> {
    let la: Vec<&str> = a.split_inclusive('\n').collect();
    let lb: Vec<&str> = b.split_inclusive('\n').collect();
    let (oa, ob) = (line_starts(&la), line_starts(&lb));
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut out: Vec<Hunk> = Vec::new();
    for op in capture_diff_slices_deadline(Algorithm::Myers, &la, &lb, Some(deadline)) {
        let (old_lines, new_lines) = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Insert { old_index, new_index, new_len } => (old_index..old_index, new_index..new_index + new_len),
            DiffOp::Delete { old_index, old_len, new_index } => (old_index..old_index + old_len, new_index..new_index),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                (old_index..old_index + old_len, new_index..new_index + new_len)
            }
        };
        if let Some(last) = out.last_mut()
            && last.old_lines.end == old_lines.start
            && last.new_lines.end == new_lines.start
        {
            last.old_lines.end = old_lines.end;
            last.new_lines.end = new_lines.end;
            last.old.end = oa[old_lines.end];
            last.new.end = ob[new_lines.end];
            continue;
        }
        out.push(Hunk {
            old: oa[old_lines.start]..oa[old_lines.end],
            new: ob[new_lines.start]..ob[new_lines.end],
            old_lines,
            new_lines,
        });
    }
    out
}

/// What the editor marks in the text under review: lines that are new, and where
/// lines of the original were taken out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    /// The new lines, in the text shown (empty where lines were only removed).
    pub lines: Range<usize>,
    /// How many lines of the original are gone at `lines.start`.
    pub removed: usize,
}

pub fn marks(original: &str, now: &str) -> Vec<Mark> {
    hunks(original, now).into_iter().map(|h| Mark { lines: h.new_lines, removed: h.old_lines.len() }).collect()
}

/// Lines added and removed, for the review bar.
pub fn counts(marks: &[Mark]) -> (usize, usize) {
    marks.iter().fold((0, 0), |(a, r), m| (a + m.lines.len(), r + m.removed))
}

/// How long the change takes to type itself in: a few hundred milliseconds, a little
/// longer for more hunks and more text.
pub fn stream_time(hunks: usize, chars: usize) -> Duration {
    Duration::from_millis((180 + 70 * hunks as u64 + chars as u64 / 3).clamp(250, 900))
}

/// A piece of a frame and where it sits in the proposed text. Pieces of the proposed
/// text map exactly; old text still waiting for its turn maps to its hunk's ends.
#[derive(Clone, Debug)]
struct Seg {
    at: Range<usize>,
    to: Range<usize>,
    exact: bool,
}

/// The document part way through typing the change in.
#[derive(Clone, Debug)]
pub struct Frame {
    pub text: String,
    segs: Vec<Seg>,
    /// Old text still waiting for its hunk's turn, in this frame's text.
    pub waiting: Vec<Range<usize>>,
    /// New text typed in so far, in this frame's text.
    pub typed: Vec<Range<usize>>,
    /// Where the typing is now, while a hunk is part way in.
    pub typing: Option<usize>,
}

impl Frame {
    /// Where `offset` of this frame falls in the proposed text; `end` picks the far end
    /// of text that has no counterpart there.
    fn to_proposed(&self, offset: usize, end: bool) -> usize {
        let hit = |s: &&Seg| s.at.start <= offset && offset <= s.at.end;
        let seg = self.segs.iter().filter(hit).find(|s| s.exact).or_else(|| self.segs.iter().find(hit));
        match seg {
            Some(s) if s.exact => (s.to.start + (offset - s.at.start)).min(s.to.end),
            Some(s) => {
                if end {
                    s.to.end
                } else {
                    s.to.start
                }
            }
            None => self.segs.last().map_or(0, |s| s.to.end),
        }
    }
}

/// The plan for typing `proposed` in over `original`.
#[derive(Clone, Debug)]
pub struct Stream {
    pub hunks: Vec<Hunk>,
    /// What each hunk costs to type, in characters (a removal costs one).
    costs: Vec<usize>,
    pub total: usize,
    pub chars: usize,
}

impl Stream {
    pub fn new(original: &str, proposed: &str) -> Self {
        let hunks = hunks(original, proposed);
        let costs: Vec<usize> = hunks.iter().map(|h| proposed[h.new.clone()].chars().count().max(1)).collect();
        let chars = hunks.iter().map(|h| proposed[h.new.clone()].chars().count()).sum();
        Self { total: costs.iter().sum(), costs, hunks, chars }
    }

    /// The document once `typed` characters of the change are in: hunks before it done,
    /// the one under way part typed (its old lines already gone), the rest as they were.
    pub fn frame(&self, original: &str, proposed: &str, typed: usize) -> Frame {
        let mut text = String::with_capacity(original.len().max(proposed.len()));
        let mut segs = Vec::new();
        let (mut waiting, mut done, mut typing) = (Vec::new(), Vec::new(), None);
        let mut push = |text: &mut String, piece: &str, to: Range<usize>, exact: bool| {
            let start = text.len();
            text.push_str(piece);
            segs.push(Seg { at: start..text.len(), to, exact });
            start..text.len()
        };
        let (mut old_at, mut new_at, mut spent) = (0, 0, 0);
        for (h, &cost) in self.hunks.iter().zip(&self.costs) {
            push(&mut text, &original[old_at..h.old.start], new_at..h.new.start, true);
            if typed >= spent + cost {
                done.push(push(&mut text, &proposed[h.new.clone()], h.new.clone(), true));
            } else if typed > spent {
                let piece = &proposed[h.new.clone()];
                let cut = piece.char_indices().nth(typed - spent).map_or(piece.len(), |(i, _)| i);
                let at = push(&mut text, &piece[..cut], h.new.start..h.new.start + cut, true);
                typing = Some(at.end);
                done.push(at);
                // A line part way typed ends there, not on the line after it.
                if !piece[..cut].ends_with('\n') && piece.ends_with('\n') {
                    push(&mut text, "\n", h.new.start + cut..h.new.end, false);
                }
            } else {
                let at = push(&mut text, &original[h.old.clone()], h.new.clone(), false);
                if !at.is_empty() {
                    waiting.push(at);
                }
            }
            spent += cost;
            old_at = h.old.end;
            new_at = h.new.end;
        }
        push(&mut text, &original[old_at..], new_at..proposed.len(), true);
        done.retain(|r| !r.is_empty());
        Frame { text, segs, waiting, typed: done, typing }
    }
}

/// The part of `now` that differs from `before`: the byte range in `before` and what
/// replaced it, cut at characters.
pub fn change(before: &str, now: &str) -> Option<(Range<usize>, String)> {
    if before == now {
        return None;
    }
    let (a, b) = (before.as_bytes(), now.as_bytes());
    let mut start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    while !before.is_char_boundary(start) || !now.is_char_boundary(start) {
        start -= 1;
    }
    let max_tail = a.len().min(b.len()) - start;
    let mut tail = a.iter().rev().zip(b.iter().rev()).take(max_tail).take_while(|(x, y)| x == y).count();
    while !before.is_char_boundary(a.len() - tail) || !now.is_char_boundary(b.len() - tail) {
        tail -= 1;
    }
    Some((start..a.len() - tail, now[start..b.len() - tail].to_owned()))
}

/// The proposed text with the edit that turned `frame` into `now` made to it as well.
pub fn carry(frame: &Frame, proposed: &str, now: &str) -> String {
    let Some((range, insert)) = change(&frame.text, now) else { return proposed.to_owned() };
    let a = frame.to_proposed(range.start, false);
    let b = frame.to_proposed(range.end, true).max(a);
    let mut out = String::with_capacity(proposed.len() + insert.len());
    out.push_str(&proposed[..a]);
    out.push_str(&insert);
    out.push_str(&proposed[b..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "a\nb\nc\nd\ne\n";
    const NEW: &str = "a\nB1\nB2\nc\ne\nf\n";

    #[test]
    fn hunks_pair_old_and_new_lines() {
        let h = hunks(OLD, NEW);
        assert_eq!(h.len(), 3, "{h:?}");
        assert_eq!((h[0].old_lines.clone(), h[0].new_lines.clone()), (1..2, 1..3));
        assert_eq!(&OLD[h[0].old.clone()], "b\n");
        assert_eq!(&NEW[h[0].new.clone()], "B1\nB2\n");
        assert_eq!((h[1].old_lines.clone(), h[1].new_lines.clone()), (3..4, 4..4), "d was removed");
        assert_eq!(&NEW[h[2].new.clone()], "f\n");
        assert_eq!(
            marks(OLD, NEW),
            [Mark { lines: 1..3, removed: 1 }, Mark { lines: 4..4, removed: 1 }, Mark { lines: 5..6, removed: 0 }]
        );
        assert_eq!(counts(&marks(OLD, NEW)), (3, 2));
        assert!(hunks(OLD, OLD).is_empty());
    }

    #[test]
    fn the_stream_types_hunk_by_hunk_from_the_original_to_the_proposal() {
        let s = Stream::new(OLD, NEW);
        assert_eq!(s.frame(OLD, NEW, 0).text, OLD);
        assert_eq!(s.frame(OLD, NEW, s.total).text, NEW);
        // Part of the first hunk typed: its old line gone, the rest untouched.
        assert_eq!(s.frame(OLD, NEW, 3).text, "a\nB1\nc\nd\ne\n");
        // The first done, the removal next.
        let after_first = "B1\nB2\n".chars().count();
        assert_eq!(s.frame(OLD, NEW, after_first + 1).text, "a\nB1\nB2\nc\ne\n");
        let (mut last, mut was_typing) = (String::new(), false);
        for typed in 0..=s.total {
            let f = s.frame(OLD, NEW, typed);
            // A line's own newline shows with its first character, so typing it changes nothing.
            assert!(f.text != last || was_typing, "every step shows something new");
            was_typing = f.typing.is_some();
            last = f.text;
        }
    }

    #[test]
    fn typing_during_the_stream_is_carried_into_the_proposal() {
        let s = Stream::new(OLD, NEW);
        let f = s.frame(OLD, NEW, 3);
        // The user types at the start of the file, and in old text still waiting.
        let now = format!("// hi\n{}", f.text);
        assert_eq!(carry(&f, NEW, &now), format!("// hi\n{NEW}"));
        let now = f.text.replace("e\n", "e!\n");
        assert_eq!(carry(&f, NEW, &now), NEW.replace("e\n", "e!\n"));
        // Inside the part already typed.
        let now = f.text.replacen("B1", "Bx1", 1);
        assert_eq!(carry(&f, NEW, &now), NEW.replacen("B1", "Bx1", 1));
        assert_eq!(carry(&f, NEW, &f.text), NEW);
    }

    #[test]
    fn a_frame_knows_what_waits_and_what_is_typed() {
        let s = Stream::new(OLD, NEW);
        let f = s.frame(OLD, NEW, 0);
        let waiting: Vec<&str> = f.waiting.iter().map(|r| &f.text[r.clone()]).collect();
        assert_eq!(waiting, ["b\n", "d\n"], "the old lines of the hunks that replace or remove");
        assert!(f.typed.is_empty() && f.typing.is_none());
        let f = s.frame(OLD, NEW, 3);
        assert_eq!(f.typed.iter().map(|r| &f.text[r.clone()]).collect::<Vec<_>>(), ["B1\n"]);
        assert_eq!(f.typing, Some(5));
        assert_eq!(f.waiting.iter().map(|r| &f.text[r.clone()]).collect::<Vec<_>>(), ["d\n"]);
        let f = s.frame(OLD, NEW, s.total);
        assert!(f.waiting.is_empty() && f.typing.is_none());
        assert_eq!(f.typed.iter().map(|r| &f.text[r.clone()]).collect::<Vec<_>>(), ["B1\nB2\n", "f\n"]);
    }

    #[test]
    fn the_stream_takes_a_few_hundred_milliseconds() {
        assert_eq!(stream_time(1, 1), Duration::from_millis(250));
        assert!(stream_time(4, 300) > Duration::from_millis(400));
        assert_eq!(stream_time(100, 100_000), Duration::from_millis(900));
    }

    #[test]
    fn multibyte_text_is_cut_at_characters() {
        let (old, new) = ("x\n", "ä€😀\n");
        let s = Stream::new(old, new);
        for typed in 0..=s.total {
            let _ = s.frame(old, new, typed).text;
        }
        assert_eq!(change("aé", "aè"), Some((1..3, "è".into())));
    }
}
