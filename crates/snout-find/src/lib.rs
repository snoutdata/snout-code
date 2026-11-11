//! How SnoutCode's searches match what is typed, and in which order they show it. Every
//! search box goes through here, so a query means the same in each of them:
//!
//! - Words separated by spaces must all match, in any order: `user test`.
//! - `*` and `?` stand for whatever is there: `token*.ja` finds `TokenController.java`.
//! - The starts of words find the words they begin: `TC`, `tc` and `tokcon` find
//!   `TokenController`, `gn` finds `getName`.
//! - Failing all that, the letters in order with gaps between them (fuzzy).
//! - For files: a word with a `/` names folders too (`web/app` finds `web/src/App.tsx`),
//!   and `Foo.java:12` or `Foo:12:5` names a line (and column) to open at.
//!
//! What matches is ranked by [`Tier`]: the name as typed, then a name that begins so, then
//! the starts of its words, then anywhere in it, then loosely; a file whose name matches
//! before one only its folders do. Within a tier the caller's boost decides (open and recent
//! files, the project's own code before what a build makes, see [`Place`]), then a finer
//! score (the case as typed, an earlier start), then the shorter text.
//!
//! Find in Files searches text, not names; [`wildcard_regex`] gives it the same `*`.

mod place;
mod text;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashSet;

use nucleo_matcher::{Config, Matcher, Utf32Str};

pub use place::Place;
pub use text::{literal_runs, wildcard_regex};

/// How well a text matches, worst first (so `a > b` means `a` ranks higher).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// A file whose path has the letters, in order, only across its folders.
    LooseInFolders,
    /// A file whose folders hold what was typed, not its name.
    InFolders,
    /// The letters, in order, with gaps.
    Loose,
    /// Somewhere in the name, as typed.
    Inside,
    /// At the starts of the name's words (camel humps): `TC`, `tokCon`.
    WordStarts,
    /// The name begins as typed.
    Prefix,
    /// The name is what was typed (a file's without its extension too).
    Exact,
}

/// How a text matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub tier: Tier,
    /// The caller's say within a tier: higher first.
    pub boost: u32,
    /// Finer, within a tier and a boost: higher first.
    pub score: i32,
    /// The characters matched, as indices of the text's `char`s, ascending.
    pub indices: Vec<usize>,
    /// The text's length in `char`s: the shorter first, all else equal.
    pub len: usize,
}

impl Match {
    pub fn boosted(mut self, boost: u32) -> Self {
        self.boost = boost;
        self
    }

    /// `Less` when `self` ranks before `other`.
    pub fn rank(&self, other: &Self) -> Ordering {
        other
            .tier
            .cmp(&self.tier)
            .then(other.boost.cmp(&self.boost))
            .then(other.score.cmp(&self.score))
            .then(self.len.cmp(&other.len))
    }
}

/// Sort `hits` best first. The sort is stable: equals keep the caller's order.
pub fn best_first<T>(hits: &mut [(Match, T)]) {
    hits.sort_by(|a, b| a.0.rank(&b.0));
}

/// The `limit` best of `items` for `query`, best first, as each one's match and its index:
/// `matches` says how an item matches (with [`Pattern::match_path`] or
/// [`Pattern::match_name`], boosted as the caller sees fit). Equals keep the order of `items`.
/// A long list is matched on every core.
pub fn rank<T: Sync>(
    items: &[T],
    query: &str,
    limit: usize,
    matches: impl Fn(&Pattern, usize, &T) -> Option<Match> + Sync,
) -> Vec<(Match, usize)> {
    let run = |from: usize, part: &[T]| -> Vec<(Match, usize)> {
        let p = Pattern::new(query);
        part.iter().enumerate().filter_map(|(i, it)| matches(&p, from + i, it).map(|m| (m, from + i))).collect()
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
    let mut hits = if items.len() < 4_000 || threads < 2 {
        run(0, items)
    } else {
        let size = items.len().div_ceil(threads);
        std::thread::scope(|scope| {
            let parts: Vec<_> = items
                .chunks(size)
                .enumerate()
                .map(|(n, part)| {
                    let run = &run;
                    scope.spawn(move || run(n * size, part))
                })
                .collect();
            parts.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        })
    };
    if hits.len() > limit {
        // Only the best need sorting.
        hits.select_nth_unstable_by(limit, |a, b| a.0.rank(&b.0).then(a.1.cmp(&b.1)));
        hits.truncate(limit);
    }
    hits.sort_by(|a, b| a.0.rank(&b.0).then(a.1.cmp(&b.1)));
    hits
}

/// One word of the query, ready to match.
#[derive(Clone, Debug, Default)]
struct Term {
    /// The characters to match, as typed (without wildcards).
    typed: Vec<char>,
    /// Them in lower case, split where a wildcard stood. None is empty.
    pieces: Vec<Vec<char>>,
    /// Which of them were typed as capitals: those match only a capital (smart case).
    caps: Vec<Vec<bool>>,
    /// A wildcard stood first, or last.
    lead: bool,
    trail: bool,
}

impl Term {
    fn new(word: &str) -> Self {
        let mut t = Term::default();
        let (mut piece, mut caps) = (Vec::new(), Vec::new());
        for c in word.chars() {
            if matches!(c, '*' | '?') {
                if t.typed.is_empty() {
                    t.lead = true;
                }
                if !piece.is_empty() {
                    t.pieces.push(std::mem::take(&mut piece));
                    t.caps.push(std::mem::take(&mut caps));
                }
            } else {
                t.typed.push(c);
                piece.push(lower(c));
                caps.push(c.is_uppercase());
            }
        }
        t.trail = piece.is_empty() && !t.typed.is_empty();
        if !piece.is_empty() {
            t.pieces.push(piece);
            t.caps.push(caps);
        }
        t
    }

    fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    fn needle(&self) -> impl Iterator<Item = char> + '_ {
        self.pieces.iter().flatten().copied()
    }
}

/// A word of the query against a file's path: what its folders must hold, and its name.
#[derive(Clone, Debug)]
struct PathTerm {
    folders: Option<Term>,
    name: Option<Term>,
    /// Typed without a `/`: it may be found in the folders when not in the name.
    anywhere: bool,
}

/// A text ready to be matched: its characters, them in lower case, and where its words start.
struct Text<'a> {
    src: &'a str,
    chars: Vec<char>,
    low: Vec<char>,
    starts: Vec<usize>,
    /// Where the name ends without its extension (`chars.len()` when it has none).
    stem: usize,
}

impl<'a> Text<'a> {
    fn new(s: &'a str, file: bool) -> Self {
        let chars: Vec<char> = s.chars().collect();
        let low = chars.iter().map(|c| lower(*c)).collect();
        let starts = word_starts(&chars);
        let stem = if file {
            chars.iter().rposition(|c| *c == '.').filter(|i| *i > 0).unwrap_or(chars.len())
        } else {
            chars.len()
        };
        Text { src: s, chars, low, starts, stem }
    }
}

/// What was typed, ready to match names and paths.
pub struct Pattern {
    words: Vec<Term>,
    paths: Vec<PathTerm>,
    at: Option<(u32, u32)>,
    /// Made when a loose match is first tried: it is not small.
    matcher: RefCell<Option<Matcher>>,
    /// The same, whatever the case of what was typed: for the filters.
    uncased: std::cell::OnceCell<Box<Pattern>>,
}

impl Pattern {
    pub fn new(query: &str) -> Self {
        let mut raw: Vec<&str> = query.split_whitespace().collect();
        let mut at = None;
        if let Some(last) = raw.last_mut()
            && let Some((rest, line, column)) = split_line(last)
        {
            at = Some((line, column));
            *last = rest;
        }
        raw.retain(|w| !w.is_empty());
        let words: Vec<Term> = raw.iter().map(|w| Term::new(w)).filter(|t| !t.is_empty()).collect();
        let paths = raw
            .iter()
            .filter_map(|w| {
                let (folders, name) = match w.rsplit_once('/') {
                    Some((d, n)) => (Some(Term::new(&d.replace('/', "*"))).filter(|t| !t.is_empty()), n),
                    None => (None, *w),
                };
                let name = Some(Term::new(name)).filter(|t| !t.is_empty());
                (folders.is_some() || name.is_some()).then_some(PathTerm { folders, name, anywhere: !w.contains('/') })
            })
            .collect();
        Pattern { words, paths, at, matcher: RefCell::new(None), uncased: Default::default() }
    }

    /// This pattern with capitals that match small letters too, when it has any.
    fn uncased(&self) -> Option<&Pattern> {
        let cased = |t: &Term| t.caps.iter().flatten().any(|c| *c);
        let any = self.words.iter().any(cased) || self.paths.iter().any(|p| p.folders.iter().chain(&p.name).any(cased));
        any.then(|| {
            &**self.uncased.get_or_init(|| {
                let clear =
                    |t: &Term| Term { caps: t.caps.iter().map(|c| vec![false; c.len()]).collect(), ..t.clone() };
                Box::new(Pattern {
                    words: self.words.iter().map(clear).collect(),
                    paths: self
                        .paths
                        .iter()
                        .map(|p| PathTerm {
                            folders: p.folders.as_ref().map(clear),
                            name: p.name.as_ref().map(clear),
                            anywhere: p.anywhere,
                        })
                        .collect(),
                    at: self.at,
                    matcher: RefCell::new(None),
                    uncased: Default::default(),
                })
            })
        })
    }

    /// Nothing to match: every text does.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() && self.paths.is_empty()
    }

    /// The line and column (from 0) a `:12` or `:12:5` at the end named.
    pub fn line(&self) -> Option<(u32, u32)> {
        self.at
    }

    /// How `name` (a symbol, a command, a branch) matches, if it does. Every word must.
    pub fn match_name(&self, name: &str) -> Option<Match> {
        if !self.words.iter().all(|t| subsequence(t, name)) {
            return None;
        }
        let text = Text::new(name, false);
        let mut m = self.matcher.borrow_mut();
        let mut out = Match { tier: Tier::Exact, boost: 0, score: 0, indices: Vec::new(), len: text.chars.len() };
        for t in &self.words {
            let (tier, score, idx) = term_in(t, &text, &mut m, Config::DEFAULT)?;
            out.tier = out.tier.min(tier);
            out.score += score;
            out.indices.extend(idx);
        }
        out.indices.sort_unstable();
        out.indices.dedup();
        Some(out)
    }

    /// Whether a row of a list (settings, branches, the keymap) stays when it is filtered:
    /// a match short of the loose one.
    /// A capital typed finds a small letter too, here.
    pub fn filter(&self, text: &str) -> bool {
        let keeps = |p: &Pattern| p.match_name(text).is_some_and(|m| m.tier >= Tier::Inside);
        self.is_empty() || keeps(self) || self.uncased().is_some_and(keeps)
    }

    /// [`Self::filter`] for a list of files: by name, or by folders as typed.
    pub fn filter_path(&self, path: &str) -> bool {
        let keeps =
            |p: &Pattern| p.match_path(path).is_some_and(|m| m.tier >= Tier::InFolders && m.tier != Tier::Loose);
        self.is_empty() || keeps(self) || self.uncased().is_some_and(keeps)
    }

    /// How the file at `path` (with `/` between folders) matches, if it does: by its name,
    /// or failing that its folders. The indices count in the whole path.
    pub fn match_path(&self, path: &str) -> Option<Match> {
        let quick = self.paths.iter().all(|p| {
            p.folders.as_ref().is_none_or(|t| subsequence(t, path))
                && p.name.as_ref().is_none_or(|t| subsequence(t, path))
        });
        if !quick {
            return None;
        }
        let split = path.rfind('/').map_or(0, |i| i + 1);
        let (dir, name) = path.split_at(split);
        let offset = dir.chars().count();
        let name_text = Text::new(name, true);
        let mut path_text: Option<Text> = None;
        let mut path_config = Config::DEFAULT;
        path_config.set_match_paths();
        let mut m = self.matcher.borrow_mut();
        let mut out =
            Match { tier: Tier::Exact, boost: 0, score: 0, indices: Vec::new(), len: offset + name_text.chars.len() };
        let mut named = false;
        for p in &self.paths {
            if let Some(f) = &p.folders {
                let dir_text = path_text.get_or_insert_with(|| Text::new(path, false));
                // The folders only: the name is not theirs.
                let dir_only = Text {
                    src: dir,
                    chars: dir_text.chars[..offset].to_vec(),
                    low: dir_text.low[..offset].to_vec(),
                    starts: dir_text.starts.iter().copied().filter(|s| *s < offset).collect(),
                    stem: offset,
                };
                let (tier, score, idx) = term_in(f, &dir_only, &mut m, path_config.clone())?;
                out.score += score + if tier >= Tier::Inside { 20 } else { 0 };
                out.indices.extend(idx);
            }
            let Some(t) = &p.name else { continue };
            named = true;
            if let Some((tier, score, idx)) = term_in(t, &name_text, &mut m, Config::DEFAULT) {
                out.tier = out.tier.min(tier);
                out.score += score;
                out.indices.extend(idx.into_iter().map(|i| i + offset));
                continue;
            }
            if !p.anywhere {
                return None;
            }
            let whole = path_text.get_or_insert_with(|| Text::new(path, false));
            let (tier, score, idx) = term_in(t, whole, &mut m, path_config.clone())?;
            out.tier = out.tier.min(if tier >= Tier::Inside { Tier::InFolders } else { Tier::LooseInFolders });
            out.score += score;
            out.indices.extend(idx);
        }
        if !named {
            out.tier = Tier::Inside;
        }
        out.indices.sort_unstable();
        out.indices.dedup();
        Some(out)
    }
}

/// How `t` matches `text`: its tier, a finer score, and the characters it took.
fn term_in(t: &Term, text: &Text, m: &mut Option<Matcher>, config: Config) -> Option<(Tier, i32, Vec<usize>)> {
    let low = &text.low;
    let found = exact(t, &low[..text.stem])
        .or_else(|| (text.stem < low.len()).then(|| exact(t, low)).flatten())
        .map(|idx| (Tier::Exact, 0, idx))
        .or_else(|| (!t.lead).then(|| in_order(t, text, true)).flatten().map(|idx| (Tier::Prefix, 0, idx)))
        .or_else(|| {
            humps(t, text).map(|(idx, chunks)| {
                let first = if idx.first() == Some(&0) { 8 } else { 0 };
                (Tier::WordStarts, first - chunks as i32, idx)
            })
        })
        .or_else(|| in_order(t, text, false).map(|idx| (Tier::Inside, -(idx[0].min(50) as i32), idx)))
        .or_else(|| {
            let m = m.get_or_insert_with(|| Matcher::new(Config::DEFAULT));
            m.config = config;
            m.config.normalize = false;
            let needle: String = t.needle().collect();
            let (mut hb, mut nb) = (Vec::new(), Vec::new());
            let mut idx = Vec::new();
            let score = m.fuzzy_indices(Utf32Str::new(text.src, &mut hb), Utf32Str::new(&needle, &mut nb), &mut idx)?;
            let mut idx: Vec<usize> = idx.into_iter().map(|i| i as usize).collect();
            idx.sort_unstable();
            Some((Tier::Loose, score as i32, idx))
        })?;
    let (tier, score, idx) = found;
    // Capitals as typed count, a little.
    let same_case =
        idx.iter().zip(&t.typed).filter(|(i, c)| c.is_uppercase() && text.chars.get(**i) == Some(*c)).count() as i32;
    Some((tier, score * 4 + same_case, idx))
}

/// The whole of `low` matched by `t`'s pieces, wildcards between them (and first or last when
/// typed so).
fn exact(t: &Term, low: &[char]) -> Option<Vec<usize>> {
    let total: usize = t.pieces.iter().map(Vec::len).sum();
    if total > low.len() {
        return None;
    }
    if t.pieces.len() == 1 && !t.lead && !t.trail {
        return (low == t.pieces[0].as_slice()).then(|| (0..low.len()).collect());
    }
    let first = &t.pieces[0];
    let last = &t.pieces[t.pieces.len() - 1];
    if !t.lead && !low.starts_with(first) {
        return None;
    }
    if !t.trail && !low.ends_with(last) {
        return None;
    }
    // In order, the last one at the end unless a wildcard follows it.
    let idx = plain_in_order(&t.pieces, low, !t.lead)?;
    if t.trail || idx.last() == Some(&(low.len() - 1)) {
        return Some(idx);
    }
    // The greedy pass put the last piece too early: take the one at the end.
    let head = match &t.pieces[..t.pieces.len() - 1] {
        [] => Vec::new(),
        before => plain_in_order(before, &low[..low.len() - last.len()], !t.lead)?,
    };
    Some(head.into_iter().chain(low.len() - last.len()..low.len()).collect())
}

/// Each piece found whole, in order, the first at the start when `anchored`; a capital
/// typed only on a capital.
fn in_order(t: &Term, text: &Text, anchored: bool) -> Option<Vec<usize>> {
    let fits = |n: usize, at: usize| {
        let p = &t.pieces[n];
        text.low[at..].starts_with(p)
            && t.caps[n].iter().enumerate().all(|(j, c)| !c || text.chars[at + j].is_uppercase())
    };
    let mut at = 0;
    let mut idx = Vec::new();
    for (n, p) in t.pieces.iter().enumerate() {
        let start = if n == 0 && anchored {
            fits(0, 0).then_some(0)?
        } else {
            (at..=text.low.len().checked_sub(p.len())?).find(|&i| fits(n, i))?
        };
        idx.extend(start..start + p.len());
        at = start + p.len();
    }
    (!t.pieces.is_empty()).then_some(idx)
}

/// [`in_order`] whatever the case.
fn plain_in_order(pieces: &[Vec<char>], low: &[char], anchored: bool) -> Option<Vec<usize>> {
    let mut at = 0;
    let mut idx = Vec::new();
    for (n, p) in pieces.iter().enumerate() {
        let start = if n == 0 && anchored { low.starts_with(p).then_some(0)? } else { at + find(&low[at..], p)? };
        idx.extend(start..start + p.len());
        at = start + p.len();
    }
    (!pieces.is_empty()).then_some(idx)
}

fn find(hay: &[char], needle: &[char]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// The pieces as runs at the starts of words, in order (`tokcon` as `Tok` `Con`), a capital
/// typed only on a capital: the characters taken, and how many runs.
fn humps(t: &Term, text: &Text) -> Option<(Vec<usize>, usize)> {
    struct Walk<'a> {
        t: &'a Term,
        text: &'a Text<'a>,
        idx: Vec<usize>,
        runs: usize,
        failed: HashSet<(usize, usize, usize)>,
    }
    impl Walk<'_> {
        /// Piece `p` from its character `o` on, at word `from` or a later one.
        fn go(&mut self, p: usize, o: usize, from: usize) -> bool {
            if p == self.t.pieces.len() {
                return true;
            }
            if self.failed.contains(&(p, o, from)) {
                return false;
            }
            let (piece, caps, low, chars) = (&self.t.pieces[p], &self.t.caps[p], &self.text.low, &self.text.chars);
            for k in from..self.text.starts.len() {
                let st = self.text.starts[k];
                let most = piece[o..]
                    .iter()
                    .zip(&caps[o..])
                    .zip(&low[st..])
                    .enumerate()
                    .take_while(|(j, ((a, cap), b))| a == b && (!**cap || chars[st + j].is_uppercase()))
                    .count();
                for l in (1..=most).rev() {
                    let end = st + l;
                    let next = self.text.starts.partition_point(|s| *s < end);
                    let (np, no) = if o + l == piece.len() { (p + 1, 0) } else { (p, o + l) };
                    let len = self.idx.len();
                    self.idx.extend(st..end);
                    self.runs += 1;
                    if self.go(np, no, next) {
                        return true;
                    }
                    self.idx.truncate(len);
                    self.runs -= 1;
                }
            }
            self.failed.insert((p, o, from));
            false
        }
    }
    let mut w = Walk { t, text, idx: Vec::new(), runs: 0, failed: HashSet::new() };
    w.go(0, 0, 0).then_some((w.idx, w.runs))
}

/// Where the words of `chars` start: after a character that is not a letter or digit, at a
/// capital after a small letter (`getName`), at the last capital of a run before a small
/// letter (`HTTPServer`), and where digits start or stop (`Base64Encoder`).
fn word_starts(chars: &[char]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            continue;
        }
        let Some(&prev) = i.checked_sub(1).and_then(|j| chars.get(j)) else {
            out.push(i);
            continue;
        };
        let next_small = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        if !prev.is_alphanumeric()
            || (c.is_uppercase() && prev.is_lowercase())
            || (c.is_uppercase() && prev.is_uppercase() && next_small)
            || c.is_numeric() != prev.is_numeric()
        {
            out.push(i);
        }
    }
    out
}

/// Whether `t`'s letters all appear in `hay`, in order: the cheap test before the rest.
fn subsequence(t: &Term, hay: &str) -> bool {
    let mut needle = t.needle().peekable();
    for c in hay.chars() {
        match needle.peek() {
            None => return true,
            Some(n) if *n == lower(c) => {
                needle.next();
            }
            _ => {}
        }
    }
    needle.peek().is_none()
}

fn lower(c: char) -> char {
    if c.is_ascii() { c.to_ascii_lowercase() } else { c.to_lowercase().next().unwrap_or(c) }
}

/// `Foo.java:12` or `Foo:12:5` as `Foo.java` and the line and column from 0.
fn split_line(word: &str) -> Option<(&str, u32, u32)> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (head, last) = word.rsplit_once(':')?;
    if !digits(last) {
        return None;
    }
    let (rest, line, column) = match head.rsplit_once(':') {
        Some((rest, line)) if digits(line) => (rest, line, Some(last)),
        _ => (head, last, None),
    };
    let line: u32 = line.parse().ok()?;
    let column: u32 = column.map_or(Some(1), |c| c.parse().ok())?;
    Some((rest, line.max(1) - 1, column.max(1) - 1))
}

#[cfg(test)]
mod tests;
