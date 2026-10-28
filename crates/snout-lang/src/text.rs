//! Incremental document sync: the one edit that turns the last text the server saw into
//! the text now, found by comparing the two from both ends. The texts come as chunks (a
//! rope's leaves), so nothing is copied and unchanged runs compare with `memcmp`.

/// Bytes the two chunk streams share from the start.
pub fn common_prefix<'a>(a: impl IntoIterator<Item = &'a str>, b: impl IntoIterator<Item = &'a str>) -> usize {
    common(a.into_iter().map(str::as_bytes), b.into_iter().map(str::as_bytes), false)
}

/// Bytes the two texts share at the end; the chunks come last first.
pub fn common_suffix<'a>(
    a_reversed: impl IntoIterator<Item = &'a str>,
    b_reversed: impl IntoIterator<Item = &'a str>,
) -> usize {
    common(a_reversed.into_iter().map(str::as_bytes), b_reversed.into_iter().map(str::as_bytes), true)
}

fn common<'a>(mut a: impl Iterator<Item = &'a [u8]>, mut b: impl Iterator<Item = &'a [u8]>, from_end: bool) -> usize {
    let (mut x, mut y): (&[u8], &[u8]) = (&[], &[]);
    let mut total = 0;
    loop {
        if x.is_empty() {
            match a.next() {
                Some(c) => x = c,
                None => return total,
            }
            continue;
        }
        if y.is_empty() {
            match b.next() {
                Some(c) => y = c,
                None => return total,
            }
            continue;
        }
        let n = x.len().min(y.len());
        let (xs, ys) = if from_end { (&x[x.len() - n..], &y[y.len() - n..]) } else { (&x[..n], &y[..n]) };
        if xs == ys {
            total += n;
            if from_end {
                x = &x[..x.len() - n];
                y = &y[..y.len() - n];
            } else {
                x = &x[n..];
                y = &y[n..];
            }
            continue;
        }
        let same = if from_end {
            xs.iter().rev().zip(ys.iter().rev()).take_while(|(p, q)| p == q).count()
        } else {
            xs.iter().zip(ys).take_while(|(p, q)| p == q).count()
        };
        return total + same;
    }
}

/// The edit between two texts of `old_len` and `new_len` bytes sharing `prefix` bytes at
/// the start and `suffix` at the end: the replaced byte range of the old text, and the
/// range of the new text that replaces it. `None` when they are the same. The suffix is
/// trimmed so the two never overlap; `boundary` moves an offset in the new text back to a
/// character boundary, and must agree with the old text below the prefix.
pub fn edit_between(
    old_len: usize,
    new_len: usize,
    prefix: usize,
    suffix: usize,
    boundary: impl Fn(usize) -> usize,
) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    if old_len == new_len && prefix >= old_len {
        return None;
    }
    let start = boundary(prefix.min(old_len).min(new_len));
    let suffix = suffix.min(old_len - start).min(new_len - start);
    // The new text's end of the edit, at a boundary; the old one follows by length.
    let new_end = new_len - suffix;
    let mut new_end_b = new_end;
    while new_end_b < new_len && boundary(new_end_b) != new_end_b {
        new_end_b += 1;
    }
    let tail = new_len - new_end_b;
    Some((start..old_len - tail, start..new_end_b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(old: &[&str], new: &[&str]) -> Option<(std::ops::Range<usize>, String)> {
        let (o, n): (String, String) = (old.concat(), new.concat());
        let p = common_prefix(old.iter().copied(), new.iter().copied());
        let s = common_suffix(old.iter().rev().copied(), new.iter().rev().copied());
        let back = |mut i: usize| {
            while !n.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        edit_between(o.len(), n.len(), p, s, back).map(|(r, nr)| (r, n[nr].to_owned()))
    }

    #[test]
    fn finds_the_edit_across_chunk_boundaries() {
        assert_eq!(edit(&["ab", "cd"], &["a", "bcd"]), None);
        assert_eq!(edit(&["hello ", "world"], &["hello", " big ", "world"]), Some((6..6, "big ".into())));
        assert_eq!(edit(&["abc"], &["ac"]), Some((1..2, String::new())));
        assert_eq!(edit(&["aaa"], &["aaaa"]), Some((3..3, "a".into())), "a repeated run grows at its end");
        assert_eq!(edit(&[""], &["x"]), Some((0..0, "x".into())));
        assert_eq!(edit(&["x"], &[""]), Some((0..1, String::new())));
        assert_eq!(edit(&["foo(a)"], &["foo(b, c)"]), Some((4..5, "b, c".into())));
    }

    #[test]
    fn never_splits_a_character() {
        // é and è share their first byte.
        assert_eq!(edit(&["café"], &["cafè"]), Some((3..5, "è".into())));
        assert_eq!(edit(&["☕a"], &["☕☕a"]), Some((3..3, "☕".into())));
    }
}
