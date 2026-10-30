//! Annotate with Git Blame: a column left of the line numbers saying, for each line, when it
//! was last changed and by whom, shaded by age (recent lines stronger), the text shown once
//! per run of lines from the same commit. Lines changed since the last commit read "Not
//! committed yet", or, when an agent wrote them, the agent in its colour and mark: Annotate
//! with Agents opens the same column.
//!
//! A line's card tells the commit (author, email, dates, hash, whole message) with Show Diff,
//! Show in Git Log, Copy Hash and Annotate Previous Revision (the file as the commit's parent
//! had it, read only, with a breadcrumb back). A click on a line lights every line of its
//! commit and selects the commit in the Git view.
//!
//! The blame is `git blame --porcelain` at HEAD, run on the host off the UI thread and kept
//! by file and HEAD; the editor's text is diffed against the blamed text, so the column
//! follows typing (lines shift at once, the diff settles them once typing pauses) and comes
//! back after a save or a commit.
//!
//! The same data gives the author hints after each class and method's line ("Ada Lovelace
//! +2": the main author and how many others; a click lists them), and the current line's
//! blame after the end of the caret's line, faint.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Point, SharedString, StatefulInteractiveElement as _, Styled as _, Task, TextAlign,
    TextRun, Window, canvas, div, point, px, size,
};
use gpui_component::input::{EditorState, Rope, RopeExt as _};
use snout_core::blame::{Blame, BlameCommit};

use super::run_gutter::visible_lines;
use super::{EditorDefaults, EditorEvent, EditorPanel, font_size, language_for};
use crate::claude_ide::ledger::Owners;
use crate::components::{Button, IconName};
use crate::look;
use crate::theme::{Palette, mix, palette};

/// What a request from the column asks of the Git view.
#[derive(Clone, Debug, PartialEq)]
pub enum GitRequest {
    /// Select this commit in the Git view, quietly.
    Select(String),
    /// Show this commit in the Git view.
    ShowInLog(String),
    /// Show the diff of this file (its path at the commit) in this commit.
    ShowDiff(String, String),
}

/// What a line of the column says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// The commit (an index into the blame's commits) that last changed it.
    Commit(u32),
    /// Changed since the last commit, by you.
    Uncommitted,
    /// Changed since the last commit, by an agent (the ledger's edit).
    Agent(u32),
    /// Nothing to say (the empty line after the last one, or no history).
    Blank,
}

/// The line `line` as the column says it: `map` carries the editor's lines to the blamed
/// ones (None: the lines are the blame's own, a revision), `agent` is the agent's edit behind
/// the line, if any.
pub fn row_of(line: usize, blame: Option<&Blame>, map: Option<&[Option<u32>]>, agent: Option<u32>) -> Row {
    let Some(blame) = blame else { return agent.map_or(Row::Blank, Row::Agent) };
    let blamed = match map {
        None => (line < blame.lines.len()).then_some(line as u32),
        Some(map) => match map.get(line) {
            Some(Some(b)) => Some(*b),
            Some(None) => return agent.map_or(Row::Uncommitted, Row::Agent),
            None => return agent.map_or(Row::Blank, Row::Agent),
        },
    };
    blamed.and_then(|b| blame.lines.get(b as usize)).map_or(Row::Blank, |&c| Row::Commit(c))
}

/// How strongly a line of each commit is shaded, from its age among the file's commits: the
/// newest 1, the oldest 0.
pub fn age_ranks(blame: &Blame) -> Vec<f32> {
    let mut times: Vec<i64> = blame.commits.iter().map(|c| c.time).collect();
    times.sort_unstable();
    times.dedup();
    let n = times.len();
    blame
        .commits
        .iter()
        .map(|c| {
            if n <= 1 {
                return 1.;
            }
            let i = times.binary_search(&c.time).unwrap_or(0);
            i as f32 / (n - 1) as f32
        })
        .collect()
}

/// The weakest and strongest shade of the column: how much of the accent goes into the pane.
pub const SHADE_MIN: f32 = 0.03;
pub const SHADE_MAX: f32 = 0.30;

/// The shade of a line of rank `rank` (see [`age_ranks`]): old lines fade, recent ones stand
/// out, the last few commits most.
pub fn shade(rank: f32) -> f32 {
    SHADE_MIN + (SHADE_MAX - SHADE_MIN) * rank.clamp(0., 1.).powf(1.6)
}

/// The colour behind a line of rank `rank`.
pub fn shade_color(rank: f32, p: &Palette) -> Hsla {
    mix(p.accent, shade(rank), p.panel)
}

/// A name's initials: "Ada Lovelace" is "AL", "grace" is "G".
pub fn initials(name: &str) -> String {
    let words: Vec<&str> =
        name.split_whitespace().filter(|w| w.chars().next().is_some_and(char::is_alphanumeric)).collect();
    let pick = match words.len() {
        0 => return "?".into(),
        1 => vec![words[0]],
        n => vec![words[0], words[n - 1]],
    };
    pick.iter().filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

/// A name shortened: "Ada Lovelace" is "Ada L.".
pub fn short_name(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        [] => "?".into(),
        [one] => (*one).to_owned(),
        [first, .., last] => {
            format!("{first} {}.", last.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default())
        }
    }
}

/// How the column writes authors, by how wide the editor is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Full,
    Short,
    Initials,
}

impl Mode {
    /// The mode for an editor `width` pixels wide.
    pub fn for_width(width: f32) -> Self {
        if width >= 980. {
            Mode::Full
        } else if width >= 640. {
            Mode::Short
        } else {
            Mode::Initials
        }
    }

    pub fn author(self, name: &str) -> String {
        match self {
            Mode::Full => cut(name, AUTHOR_MAX),
            Mode::Short => cut(&short_name(name), 12),
            Mode::Initials => initials(name),
        }
    }
}

/// The longest author name the column shows in full.
const AUTHOR_MAX: usize = 20;
/// The date's characters, and the space after it.
const DATE_CHARS: usize = 10;

/// `s` cut to `n` characters, with an ellipsis when cut.
fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('\u{2026}');
        out
    }
}

/// The local time of `secs`: year, month (1 to 12), day, hour, minute.
fn local(secs: i64) -> (i32, i32, i32, i32, i32) {
    match snout_host::local_time(secs) {
        Some([y, mo, d, h, mi, _]) => (y, mo, d, h, mi),
        None => (1970, 1, 1, 0, 0),
    }
}

/// `secs` as a short date: "2024-03-15".
pub fn short_date(secs: i64) -> String {
    let (y, m, d, _, _) = local(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `secs` in full: "March 15, 2024 at 3:39 PM".
pub fn long_date(secs: i64) -> String {
    let (y, m, d, h, min) = local(secs);
    let (h12, half) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{} {d}, {y} at {h12}:{min:02} {half}", MONTHS[(m - 1).clamp(0, 11) as usize])
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// A blame with what the column needs of it worked out once.
#[derive(Debug, Default)]
pub struct Annotated {
    pub blame: Blame,
    /// Each commit's rank by age (see [`age_ranks`]) and short date.
    pub ranks: Vec<f32>,
    pub dates: Vec<String>,
    /// The longest author name among the commits, in characters.
    pub author_chars: usize,
}

impl Annotated {
    pub fn new(blame: Blame) -> Self {
        let ranks = age_ranks(&blame);
        let dates = blame.commits.iter().map(|c| short_date(c.time)).collect();
        let author_chars = blame.commits.iter().map(|c| c.author.chars().count()).max().unwrap_or(0);
        Self { blame, ranks, dates, author_chars }
    }
}

// ----- Reading the blame, kept by file and HEAD ---------------------------------------------

type Key = (usize, PathBuf, String, String);

/// The blames read lately, newest last.
static CACHE: Mutex<VecDeque<(Key, Arc<Annotated>)>> = Mutex::new(VecDeque::new());
const CACHE_MAX: usize = 48;

/// Annotate `rel` (relative to `root`) at `rev`, or at HEAD when `rev` is None (a file HEAD
/// does not have: every line not committed yet). Blocking: git on the host. Kept by file and
/// commit, so asking again after a save or for another tab costs one `rev-parse`.
pub fn read_blame(
    host: &snout_host::HostRef,
    root: &Path,
    rel: &str,
    rev: Option<&str>,
) -> Result<Arc<Annotated>, String> {
    let at_head = rev.is_none();
    let rev = match rev {
        Some(r) => r.to_owned(),
        None => match snout_core::blame::head(&**host, root) {
            Some(id) => id,
            None => return Ok(Arc::new(Annotated::default())),
        },
    };
    let key: Key = (Arc::as_ptr(host) as *const () as usize, root.to_owned(), rel.to_owned(), rev.clone());
    if let Some((_, a)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
        return Ok(a.clone());
    }
    let blame = match snout_core::blame::blame_file(&**host, root, &rev, rel) {
        Ok(b) => b,
        // A file the last commit does not have: none of it is committed.
        Err(_) if at_head => Blame { rev: rev.clone(), path: rel.to_owned(), ..Default::default() },
        Err(e) => return Err(e.to_string()),
    };
    let a = Arc::new(Annotated::new(blame));
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.push_back((key, a.clone()));
    while cache.len() > CACHE_MAX {
        cache.pop_front();
    }
    Ok(a)
}

// ----- Declarations, for the author hints -----------------------------------------------------

/// A class or method: its name, the line its name is on, and all its lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub name: String,
    pub line: usize,
    pub lines: Range<usize>,
}

/// Whether the author hints know the declarations of `language`.
pub fn has_declarations(language: &str) -> bool {
    matches!(language, "java" | "kotlin")
}

/// The classes and methods of `text` in `language` (Java or Kotlin), outermost first. Method
/// bodies are not looked into.
pub fn declarations(text: &str, language: &str) -> Vec<Decl> {
    let (lang, kinds, leaves): (tree_sitter::Language, &[&str], &[&str]) = match language {
        "java" => (
            tree_sitter::Language::new(tree_sitter_java::LANGUAGE),
            &[
                "class_declaration",
                "interface_declaration",
                "enum_declaration",
                "record_declaration",
                "annotation_type_declaration",
                "method_declaration",
                "constructor_declaration",
            ],
            &["method_declaration", "constructor_declaration"],
        ),
        "kotlin" => (
            tree_sitter::Language::new(tree_sitter_kotlin_sg::LANGUAGE),
            &["class_declaration", "object_declaration", "function_declaration"],
            &["function_declaration"],
        ),
        _ => return Vec::new(),
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(text, None) else { return Vec::new() };
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if kinds.contains(&kind) {
            let name = node.child_by_field_name("name").or_else(|| {
                let mut c = node.walk();
                node.named_children(&mut c).find(|n| matches!(n.kind(), "type_identifier" | "simple_identifier"))
            });
            if let Some(name) = name {
                out.push(Decl {
                    name: text[name.byte_range()].to_owned(),
                    line: name.start_position().row,
                    lines: node.start_position().row..node.end_position().row + 1,
                });
            }
            if leaves.contains(&kind) {
                continue;
            }
        }
        let mut c = node.walk();
        let children: Vec<_> = node.named_children(&mut c).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

/// Someone who wrote lines of a declaration: how many, and when last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorCount {
    pub name: String,
    pub email: String,
    pub lines: usize,
    pub latest: i64,
}

/// Who wrote the committed lines in `lines` (`row` says each line's), most lines first.
pub fn authors_of(lines: Range<usize>, row: impl Fn(usize) -> Row, blame: &Blame) -> Vec<AuthorCount> {
    let mut by: Vec<AuthorCount> = Vec::new();
    for line in lines {
        let Row::Commit(c) = row(line) else { continue };
        let Some(c) = blame.commits.get(c as usize) else { continue };
        let key = if c.email.is_empty() { c.author.to_lowercase() } else { c.email.to_lowercase() };
        match by
            .iter_mut()
            .find(|a| (if a.email.is_empty() { a.name.to_lowercase() } else { a.email.to_lowercase() }) == key)
        {
            Some(a) => {
                a.lines += 1;
                a.latest = a.latest.max(c.time);
            }
            None => by.push(AuthorCount { name: c.author.clone(), email: c.email.clone(), lines: 1, latest: c.time }),
        }
    }
    by.sort_by(|a, b| b.lines.cmp(&a.lines).then(b.latest.cmp(&a.latest)));
    by
}

/// The hint for `authors`: the main author, and how many others ("Ada Lovelace +2").
pub fn hint_label(authors: &[AuthorCount]) -> Option<String> {
    let first = authors.first()?;
    Some(match authors.len() {
        1 => first.name.clone(),
        n => format!("{} +{}", first.name, n - 1),
    })
}

/// An edit as lines: it replaced `removed` with `new` from the row `from` (after `prefix`, the
/// start of that line) to the row `to` (`to_at_start`: at that line's start). Returns the
/// rows touched, the new line breaks, and whether whole lines went or came (see
/// [`snout_core::blame::shift_lines`]). An edit found by comparing texts may start inside
/// the indentation it shares with the next line ("    // new\n" typed before "    if"):
/// slid back to the line's start, it is the whole line it is.
pub fn line_edit(
    prefix: &str,
    from: usize,
    to: usize,
    to_at_start: bool,
    removed: &str,
    new: &str,
) -> (usize, usize, usize, bool) {
    let lines = |s: &str| s.matches('\n').count();
    let slides = |t: &str| t.len() > prefix.len() && t.ends_with(prefix) && t[..t.len() - prefix.len()].ends_with('\n');
    match (removed.is_empty(), new.is_empty()) {
        // Lines put in.
        (true, false) if prefix.is_empty() && new.ends_with('\n') || slides(new) => (from, from, lines(new), true),
        // Lines taken out.
        (false, true) if (prefix.is_empty() && to_at_start) || slides(removed) => {
            (from, from + lines(removed), 0, true)
        }
        _ => {
            let whole = prefix.is_empty() && to_at_start && (new.is_empty() || new.ends_with('\n'));
            (from, to, lines(new), whole)
        }
    }
}

/// A colour of its own for each name, from the palette's hues.
fn person_color(name: &str, p: &Palette) -> Hsla {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let hues = p.lanes();
    hues[(h as usize) % 5]
}

// ----- The editor's state ----------------------------------------------------------------------

/// The card of a line of the column.
#[derive(Clone, Debug)]
pub struct LineCard {
    pub line: usize,
    pub at: Point<Pixels>,
}

/// The authors of a declaration, listed after a click on its hint.
#[derive(Clone, Debug)]
pub struct AuthorsCard {
    pub decl: Decl,
    pub at: Point<Pixels>,
}

/// An earlier revision of the file, annotated: the commit and path, the crumb it shows as,
/// and a read-only editor of its text.
pub struct Revision {
    pub rev: String,
    pub path: String,
    pub label: String,
    pub data: Option<Arc<Annotated>>,
    pub error: Option<String>,
    pub state: Entity<EditorState>,
    _task: Option<Task<()>>,
}

/// What was drawn last: the column and its rows (buffer line, where), and the hints.
#[derive(Clone, Debug, Default)]
pub struct Painted {
    pub column: Bounds<Pixels>,
    pub rows: Vec<(usize, Bounds<Pixels>)>,
    pub hints: Vec<(usize, Bounds<Pixels>)>,
}

#[derive(Default)]
pub struct BlameView {
    /// The column shows.
    pub shown: bool,
    /// The file as HEAD has it, annotated.
    pub data: Option<Arc<Annotated>>,
    pub loading: bool,
    pub error: Option<String>,
    /// For each line of the text, the blamed line it still is (None: changed since).
    pub map: Arc<Vec<Option<u32>>>,
    /// The text `map` is for.
    map_text: Rope,
    /// The classes and methods, for the author hints.
    pub decls: Arc<Vec<Decl>>,
    /// The commit whose lines are lit (a click on the column).
    pub selected: Option<String>,
    pub card: Option<LineCard>,
    pub card_hovered: bool,
    _card: Option<Task<()>>,
    pub authors: Option<AuthorsCard>,
    /// Earlier revisions annotated, the last showing.
    pub revisions: Vec<Revision>,
    /// The line whose blame shows after its end (the caret's, once it rested there).
    pub line: Option<usize>,
    line_seen: Option<usize>,
    _line: Option<Task<()>>,
    _task: Option<Task<()>>,
    pub painted: Rc<RefCell<Painted>>,
    /// Where each line's end-of-line hints end, this frame (the parameter hints draw first).
    pub tails: Rc<RefCell<HashMap<usize, Pixels>>>,
    /// The editor's width seen last, for the column's mode.
    width: Rc<Cell<f32>>,
}

/// An agent's look in the column: colour, mark, name, and when its edit was.
#[derive(Clone)]
struct AgentLook {
    color: Hsla,
    glyph: char,
    name: SharedString,
    when: String,
}

impl EditorPanel {
    /// The repository root and the file's path in it, when Git has the file.
    fn git_rel(&self) -> Option<(PathBuf, String)> {
        let root = self.root.clone()?;
        let rel = self.path.strip_prefix(&root).ok()?.to_string_lossy().replace('\\', "/");
        Some((root, rel))
    }

    /// Why Annotate with Git Blame does nothing here, if it does nothing.
    pub fn blame_unavailable(&self) -> Option<&'static str> {
        if self.class_file.is_some() {
            Some("A library's class file has no Git history.")
        } else if self.root.is_none() {
            Some("Not a Git project: no Git repository holds this file.")
        } else {
            None
        }
    }

    /// Whether the author hints show in this tab.
    fn hints_wanted(&self, cx: &App) -> bool {
        EditorDefaults::now(cx).author_hints && has_declarations(language_for(&self.path))
    }

    /// Whether the current line's blame shows.
    fn line_blame_wanted(&self, cx: &App) -> bool {
        EditorDefaults::now(cx).line_blame && !self.blame.shown
    }

    /// Whether this tab needs the blame: the column, the hints or the line's blame.
    fn blame_wanted(&self, cx: &App) -> bool {
        self.blame_unavailable().is_none()
            && !self.loading
            && (self.blame.shown || self.hints_wanted(cx) || EditorDefaults::now(cx).line_blame)
    }

    /// The file was loaded again or another one shows: what was known goes.
    pub(super) fn reset_blame(&mut self) {
        let shown = self.blame.shown;
        self.blame = BlameView { shown, ..Default::default() };
    }

    /// Read the blame (off the UI thread; kept by file and HEAD) when this tab needs it.
    pub fn refresh_blame(&mut self, cx: &mut Context<Self>) {
        if !self.blame_wanted(cx) {
            return;
        }
        let Some((root, rel)) = self.git_rel() else { return };
        let host = crate::host::host(cx);
        let path = self.path.clone();
        let rope = self.state.read(cx).text().clone();
        let text = rope.to_string();
        let lang = self.hints_wanted(cx).then(|| language_for(&path));
        let revision = self.revision;
        self.blame.loading = self.blame.data.is_none();
        self.blame._task = Some(cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move {
                    let a = read_blame(&host, &root, &rel, None)?;
                    let map = snout_core::blame::map_lines(&a.blame.text, &text);
                    let decls = lang.map(|l| declarations(&text, l));
                    Ok::<_, String>((a, map, decls))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.path != path {
                    return;
                }
                this.blame.loading = false;
                match read {
                    Ok((a, map, decls)) => {
                        this.blame.error = None;
                        this.blame.data = Some(a);
                        if this.revision == revision {
                            this.blame.map = Arc::new(map);
                            this.blame.map_text = rope;
                            if let Some(d) = decls {
                                this.blame.decls = Arc::new(d);
                            }
                        } else {
                            this.diff_soon(cx);
                        }
                    }
                    Err(e) => this.blame.error = Some(e),
                }
                cx.notify();
            });
        }));
    }

    /// What the settled diff found: the lines' map and the declarations, for `rope`.
    pub(super) fn blame_settled(&mut self, map: Option<Vec<Option<u32>>>, decls: Option<Vec<Decl>>, rope: Rope) {
        if let Some(map) = map {
            self.blame.map = Arc::new(map);
            self.blame.map_text = rope;
        }
        if let Some(d) = decls {
            self.blame.decls = Arc::new(d);
        }
    }

    /// What the settled diff needs: the blame and the language to find declarations in.
    pub(super) fn blame_for_diff(&self, cx: &App) -> (Option<Arc<Annotated>>, Option<&'static str>) {
        let lang = (self.hints_wanted(cx) && self.blame.data.is_some()).then(|| language_for(&self.path));
        (self.blame.data.clone(), lang)
    }

    /// The text changed: the column's lines (and the hints) move with it at once; the diff
    /// settles them once typing pauses.
    pub(super) fn blame_text_changed(&mut self, cx: &mut Context<Self>) {
        self.blame.card = None;
        if self.blame.data.is_none() {
            return;
        }
        let now = self.state.read(cx).text().clone();
        let old = &self.blame.map_text;
        let Some((range, new)) = crate::lang::text::change(old, &now) else { return };
        let (a, b) = (old.offset_to_point(range.start), old.offset_to_point(range.end));
        let prefix = old.slice(old.line_start_offset(a.row)..range.start).to_string();
        let removed_text = old.slice(range.clone()).to_string();
        let (from, to, inserted, whole) = line_edit(&prefix, a.row, b.row, b.column == 0, &removed_text, &new);
        self.blame.map = Arc::new(snout_core::blame::shift_lines(&self.blame.map, from, to, inserted, whole));
        let removed = to - from;
        if inserted != removed && !self.blame.decls.is_empty() {
            let delta = inserted as isize - removed as isize;
            let moved = |l: usize| if l > from { (l as isize + delta).max(from as isize) as usize } else { l };
            let decls = self
                .blame
                .decls
                .iter()
                .map(|d| Decl {
                    name: d.name.clone(),
                    line: moved(d.line),
                    lines: d.lines.start.min(moved(d.lines.start))..moved(d.lines.end).max(moved(d.line) + 1),
                })
                .collect();
            self.blame.decls = Arc::new(decls);
        }
        self.blame.map_text = now;
    }

    /// Annotate with Git Blame (or Close Annotations): show or hide the column. Outside a Git
    /// project, say why not.
    pub fn toggle_blame(&mut self, cx: &mut Context<Self>) {
        if let Some(why) = self.blame_unavailable() {
            if self.blame.shown {
                self.set_annotations(false, cx);
            } else {
                self.show_hint(why, cx);
            }
            return;
        }
        let shown = !self.blame.shown;
        self.set_annotations(shown, cx);
    }

    /// Annotate with Agents: the same column (with no Git, only the agents' lines).
    pub fn toggle_annotate(&mut self, cx: &mut Context<Self>) {
        let shown = !self.blame.shown;
        self.set_annotations(shown, cx);
    }

    /// Show or hide the column.
    pub fn set_annotations(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.blame.shown = shown;
        if !shown {
            self.blame.card = None;
            self.blame.selected = None;
            self.blame.revisions.clear();
        }
        self.refresh_blame(cx);
        cx.notify();
    }

    /// The editor's width seen last decides how the column writes names.
    fn blame_mode(&self) -> Mode {
        let w = self.blame.width.get();
        Mode::for_width(if w == 0. { 1200. } else { w })
    }

    /// The column's font size and the width of one of its characters.
    fn column_font(cx: &App) -> (f32, f32) {
        let size = (font_size(cx) * 0.86).max(9.);
        (size, size * 0.6)
    }

    /// How wide the column is (0 when it does not show).
    pub fn blame_width(&self, cx: &App) -> f32 {
        if !self.blame.shown {
            return 0.;
        }
        let data = match self.blame.revisions.last() {
            Some(r) => r.data.as_ref(),
            None => self.blame.data.as_ref(),
        };
        let mode = self.blame_mode();
        let author = match mode {
            Mode::Full => data.map_or(12, |d| d.author_chars).clamp(8, AUTHOR_MAX),
            Mode::Short => 12,
            Mode::Initials => 3,
        };
        let (_, cw) = Self::column_font(cx);
        // Wide enough for "Not committed yet" when names are long enough to allow it.
        let chars = (DATE_CHARS + 1 + author).max(if mode == Mode::Initials { 0 } else { 17 });
        (chars as f32 * cw + 18.).round()
    }

    /// The agents' look, by the ledger's edit.
    fn agent_looks(&self, cx: &App) -> Rc<Vec<AgentLook>> {
        let p = palette(cx);
        Rc::new(
            self.agent_edits
                .iter()
                .map(|e| {
                    let a = crate::agents::by_id(&e.agent, cx);
                    AgentLook {
                        color: a.color(&p),
                        glyph: a.glyph,
                        name: a.short.clone(),
                        when: crate::claude_ide::ledger::clock(e.time()),
                    }
                })
                .collect(),
        )
    }

    /// What the column says of `line`, in the view showing (the file, or a revision).
    pub fn blame_row(&self, line: usize) -> Row {
        match self.blame.revisions.last() {
            Some(r) => row_of(line, r.data.as_ref().map(|d| &d.blame), None, None),
            None => {
                let agent = self.agent_lines.as_ref().and_then(|o| o.at(line));
                row_of(line, self.blame.data.as_ref().map(|d| &d.blame), Some(&self.blame.map), agent)
            }
        }
    }

    /// The annotated data of the view showing.
    fn shown_data(&self) -> Option<&Arc<Annotated>> {
        match self.blame.revisions.last() {
            Some(r) => r.data.as_ref(),
            None => self.blame.data.as_ref(),
        }
    }

    /// The commit of `line` in the view showing.
    pub fn blame_commit(&self, line: usize) -> Option<&BlameCommit> {
        match self.blame_row(line) {
            Row::Commit(c) => self.shown_data()?.blame.commits.get(c as usize),
            _ => None,
        }
    }

    /// What the column says of each line, as text (for tests and the agents' card).
    pub fn annotation_labels(&self, cx: &App) -> Vec<(usize, String)> {
        let looks = self.agent_looks(cx);
        let lines = self.state.read(cx).text().lines_len();
        let mut out = Vec::new();
        let mut last = None;
        for line in 0..lines {
            let row = self.blame_row(line);
            if Some(row) == last {
                continue;
            }
            last = Some(row);
            let label = match row {
                Row::Commit(c) => {
                    let Some(d) = self.shown_data() else { continue };
                    let commit = &d.blame.commits[c as usize];
                    format!("{} {}", d.dates[c as usize], commit.author)
                }
                Row::Uncommitted => "Not committed yet".into(),
                Row::Agent(e) => match looks.get(e as usize) {
                    Some(l) => format!("{} {} \u{b7} {}", l.glyph, l.name, l.when),
                    None => continue,
                },
                Row::Blank => continue,
            };
            out.push((line, label));
        }
        out
    }

    /// The state the column is drawn against: the file's editor, or the revision's.
    fn shown_state(&self) -> Entity<EditorState> {
        self.blame.revisions.last().map_or_else(|| self.state.clone(), |r| r.state.clone())
    }

    /// The column, drawn left of the gutter's lanes from the editor's own line layout, and the
    /// lines of the selected commit lit across.
    pub(super) fn blame_column(&self, cx: &Context<Self>) -> Option<impl IntoElement + use<>> {
        let width = self.blame_width(cx);
        if width <= 0. {
            return None;
        }
        let p = palette(cx);
        let state = self.shown_state();
        let revision = !self.blame.revisions.is_empty();
        let data = self.shown_data().cloned();
        let map = (!revision).then(|| self.blame.map.clone());
        let owners: Option<Owners> = if revision { None } else { self.agent_lines.clone() };
        let looks = self.agent_looks(cx);
        let selected =
            self.blame.selected.clone().and_then(|id| data.as_ref()?.blame.commits.iter().position(|c| c.id == id));
        let loading = if revision {
            self.blame.revisions.last().is_some_and(|r| r.data.is_none() && r.error.is_none())
        } else {
            self.blame.loading
        };
        let mode = self.blame_mode();
        let (size_pt, _) = Self::column_font(cx);
        let mono = look::mono_font(cx);
        let painted = self.blame.painted.clone();
        let seen = self.blame.width.clone();
        let me = cx.entity_id();
        let hovered = self.blame.card.as_ref().map(|c| c.line);
        Some(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| {
                    let w = f32::from(bounds.size.width);
                    let before = if seen.get() == 0. { 1200. } else { seen.get() };
                    seen.set(w);
                    if Mode::for_width(w) != Mode::for_width(before) {
                        cx.notify(me);
                    }
                    let column = Bounds::new(bounds.origin, size(px(width), bounds.size.height));
                    let mut drawn = Painted { column, ..Default::default() };
                    window.paint_quad(gpui::fill(column, mix(p.fg, 0.025, p.panel)));
                    let s = state.read(cx);
                    let Some(line_height) = s.line_height() else {
                        *painted.borrow_mut() = drawn;
                        return;
                    };
                    let visible = visible_lines(s);
                    let blame = data.as_ref().map(|d| &d.blame);
                    let row = |line: usize| {
                        let agent = owners.as_ref().and_then(|o| o.at(line));
                        row_of(line, blame, map.as_deref().map(|m| m.as_slice()), agent)
                    };
                    let ts = window.text_system().clone();
                    let shape = |text: String, color: Hsla, bold: bool| {
                        let mut font = gpui::font(mono.clone());
                        if bold {
                            font.weight = gpui::FontWeight::SEMIBOLD;
                        }
                        let run = TextRun {
                            len: text.len(),
                            font,
                            color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        ts.shape_line(text.into(), px(size_pt), &[run], None)
                    };
                    let mut texts = Vec::new();
                    let mut prev: Option<Row> = None;
                    for (i, (line, top)) in visible.iter().enumerate() {
                        let r = row(*line);
                        let cell = Bounds::new(point(column.left(), *top), size(px(width), line_height));
                        drawn.rows.push((*line, cell));
                        let is_selected = matches!(r, Row::Commit(c) if Some(c as usize) == selected);
                        let ground = match r {
                            Row::Commit(_) if is_selected => Some(mix(p.accent, 0.42, p.panel)),
                            Row::Commit(c) => {
                                data.as_ref().map(|d| shade_color(d.ranks.get(c as usize).copied().unwrap_or(0.), &p))
                            }
                            Row::Uncommitted => Some(mix(p.modified, 0.16, p.panel)),
                            Row::Agent(e) => looks.get(e as usize).map(|l| mix(l.color, 0.2, p.panel)),
                            Row::Blank => None,
                        };
                        if let Some(g) = ground {
                            window.paint_quad(gpui::fill(cell, g));
                        }
                        // Lines not committed yet: a stripe in yours or the agent's colour.
                        let stripe = match r {
                            Row::Uncommitted => Some(p.modified),
                            Row::Agent(e) => looks.get(e as usize).map(|l| l.color),
                            _ => None,
                        };
                        if let Some(c) = stripe {
                            window.paint_quad(gpui::fill(Bounds::new(cell.origin, size(px(3.), line_height)), c));
                        }
                        if hovered == Some(*line) {
                            window.paint_quad(gpui::fill(cell, p.fg.opacity(0.06)));
                        }
                        if is_selected {
                            let band = Bounds::new(
                                point(column.right(), *top),
                                size(bounds.right() - column.right(), line_height),
                            );
                            window.paint_quad(gpui::fill(band, p.accent.opacity(if p.dark { 0.13 } else { 0.09 })));
                        }
                        // The text once per run of lines from the same commit (and on the first
                        // line showing).
                        let first = i == 0 || prev != Some(r);
                        prev = Some(r);
                        if !first {
                            continue;
                        }
                        let x = column.left() + px(8.);
                        match r {
                            Row::Commit(c) => {
                                let Some(d) = data.as_ref() else { continue };
                                let commit = &d.blame.commits[c as usize];
                                let date = d.dates.get(c as usize).cloned().unwrap_or_default();
                                let ink = if is_selected { p.fg } else { p.weak() };
                                texts.push((point(x, *top), shape(date, ink, false)));
                                let cw = size_pt * 0.6;
                                let ax = x + px((DATE_CHARS + 1) as f32 * cw);
                                texts.push((point(ax, *top), shape(mode.author(&commit.author), p.fg, is_selected)));
                            }
                            Row::Uncommitted => {
                                let label = if mode == Mode::Initials { "Uncommitted" } else { "Not committed yet" };
                                texts.push((point(x, *top), shape(label.into(), mix(p.modified, 0.75, p.fg), false)));
                            }
                            Row::Agent(e) => {
                                let Some(l) = looks.get(e as usize) else { continue };
                                let cw = size_pt * 0.6;
                                texts.push((point(x, *top), shape(cut(&l.when, DATE_CHARS), p.weak(), false)));
                                let ax = x + px((DATE_CHARS + 1) as f32 * cw);
                                let name = match mode {
                                    Mode::Initials => l.glyph.to_string(),
                                    _ => format!("{} {}", l.glyph, l.name),
                                };
                                texts.push((point(ax, *top), shape(name, l.color, true)));
                            }
                            Row::Blank => {}
                        }
                    }
                    if loading && let Some((_, top)) = visible.first() {
                        texts.push((
                            point(column.left() + px(8.), *top),
                            shape("Reading history\u{2026}".into(), p.fg_dim, false),
                        ));
                    }
                    for (at, shaped) in texts {
                        let _ = shaped.paint(at, line_height, TextAlign::Left, None, window, cx);
                    }
                    let rule =
                        Bounds::new(point(column.right() - px(1.), column.top()), size(px(1.), column.size.height));
                    window.paint_quad(gpui::fill(rule, p.border));
                    *painted.borrow_mut() = drawn;
                },
            )
            .absolute()
            .inset_0(),
        )
    }

    // ----- The pointer on the column --------------------------------------------------------

    /// The row of the column under `at`, if any.
    fn column_line_at(&self, at: Point<Pixels>) -> Option<usize> {
        let painted = self.blame.painted.borrow();
        if !self.blame.shown || !painted.column.contains(&at) {
            return None;
        }
        painted.rows.iter().find(|(_, b)| b.contains(&at)).map(|(l, _)| *l)
    }

    /// Whether `at` is on the column.
    pub(super) fn on_column(&self, at: Point<Pixels>) -> bool {
        self.blame.shown && self.blame.painted.borrow().column.contains(&at)
    }

    /// The pointer moved: a line's card shows once it rests on the line (an agent's line: the
    /// agent's card), and goes when it leaves the line and the card.
    pub(super) fn blame_hover(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(line) = self.column_line_at(at) else {
            if self.blame.card.is_some() && !self.blame.card_hovered {
                self.hide_blame_card_soon(cx);
            }
            return;
        };
        let row = self.blame_row(line);
        if let Row::Agent(edit) = row {
            self.blame.card = None;
            let run = self
                .agent_lines
                .as_ref()
                .and_then(|o| o.runs().into_iter().find(|(r, _)| r.contains(&line)).map(|(r, _)| r))
                .unwrap_or(line..line + 1);
            if self.agent_card.as_ref().is_none_or(|c| c.lines != run) {
                let b = self.blame.painted.borrow().column;
                let top =
                    self.blame.painted.borrow().rows.iter().find(|(l, _)| *l == line).map_or(at.y, |(_, b)| b.top());
                self.agent_card =
                    Some(super::AgentCard { lines: run, edit: Some(edit), at: point(b.right() + px(4.), top) });
                cx.notify();
            }
            return;
        }
        if self.agent_card.is_some() && !self.card_hovered {
            self.agent_card = None;
            cx.notify();
        }
        if row == Row::Blank {
            return;
        }
        if self.blame.card.as_ref().is_some_and(|c| c.line == line) {
            self.blame._card = None;
            return;
        }
        let at = {
            let p = self.blame.painted.borrow();
            let top = p.rows.iter().find(|(l, _)| *l == line).map_or(at.y, |(_, b)| b.top());
            point(p.column.right() + px(4.), top)
        };
        let shown = self.blame.card.is_some();
        let delay = if shown {
            Duration::from_millis(120)
        } else {
            EditorDefaults::now(cx).hover_delay.min(Duration::from_millis(600))
        };
        self.blame._card = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |this, cx| {
                this.blame.card = Some(LineCard { line, at });
                cx.notify();
            });
        }));
    }

    fn hide_blame_card_soon(&mut self, cx: &mut Context<Self>) {
        self.blame._card = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(180)).await;
            let _ = this.update(cx, |this, cx| {
                if !this.blame.card_hovered {
                    this.blame.card = None;
                    cx.notify();
                }
            });
        }));
    }

    /// A press on the column or on an author hint. A left click on a line lights its commit's
    /// lines and selects the commit in the Git view (again: lights none); a right click opens
    /// the gutter's menu. True when the press was the column's.
    pub(super) fn blame_press(
        &mut self,
        e: &gpui::MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if e.button == gpui::MouseButton::Left
            && let Some(i) =
                self.blame.painted.borrow().hints.iter().find(|(_, b)| b.contains(&e.position)).map(|(i, _)| *i)
            && let Some(decl) = self.blame.decls.get(i).cloned()
        {
            self.blame.authors = Some(AuthorsCard { decl, at: point(e.position.x, e.position.y + px(12.)) });
            cx.notify();
            return true;
        }
        if !self.on_column(e.position) {
            return false;
        }
        if e.button == gpui::MouseButton::Right {
            let line = self.column_line_at(e.position);
            self.open_gutter_menu(e.position, line.map(|l| l as u32), window, cx);
            return true;
        }
        let Some(line) = self.column_line_at(e.position) else { return true };
        if let Some(id) = self.blame_commit(line).map(|c| c.id.clone()) {
            if self.blame.selected.as_deref() == Some(id.as_str()) {
                self.blame.selected = None;
            } else {
                self.blame.selected = Some(id.clone());
                cx.emit(EditorEvent::Git(GitRequest::Select(id)));
            }
            self.blame.card = None;
            cx.notify();
        }
        true
    }

    /// The gutter's right-click menu: Annotate with Git Blame (Close Annotations while it
    /// shows; greyed out, with the reason, outside Git), and the breakpoint's condition.
    pub fn open_gutter_menu(
        &mut self,
        at: Point<Pixels>,
        line: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let why = self.blame_unavailable();
        let shown = self.blame.shown;
        let breakpoints = self.breakpoint_line(cx).is_some();
        let target = gpui::Focusable::focus_handle(&self.state, cx);
        self.context.open(
            Some(target),
            at,
            window,
            cx,
            |e| &mut e.context,
            move |menu, _, _| {
                use crate::components::{PopupMenuItem, action_item};
                let menu = crate::components::styled(menu);
                let menu = match (why, shown) {
                    (_, true) => {
                        menu.item(action_item("Close Annotations", IconName::X, Box::new(super::AnnotateWithGitBlame)))
                    }
                    (None, false) => menu.item(action_item(
                        "Annotate with Git Blame",
                        IconName::FileClock,
                        Box::new(super::AnnotateWithGitBlame),
                    )),
                    (Some(why), false) => menu
                        .item(
                            PopupMenuItem::new("Annotate with Git Blame")
                                .icon(gpui_component::Icon::new(IconName::FileClock))
                                .disabled(true),
                        )
                        .item(PopupMenuItem::label(why)),
                };
                let menu = menu.item(action_item(
                    "Annotate with Agents",
                    IconName::UserPen,
                    Box::new(crate::claude_ide::AnnotateWithAgents),
                ));
                match line.filter(|_| breakpoints) {
                    Some(l) => menu.separator().item(action_item(
                        "Breakpoint Condition\u{2026}",
                        IconName::CircleDot,
                        Box::new(super::BreakpointConditionAt(l)),
                    )),
                    None => menu,
                }
            },
        );
    }

    // ----- The cards ---------------------------------------------------------------------------

    /// A line's card: who changed it, when, the commit's hash and message, and what to do.
    pub(super) fn blame_card_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.blame.card.as_ref()?;
        let p = palette(cx);
        let row = self.blame_row(card.line);
        let body = div()
            .id("blame-card")
            .debug_selector(|| "blame-card".into())
            .occlude()
            .w(px(420.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(9.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.blame.card_hovered = *hovered;
                if !*hovered {
                    this.hide_blame_card_soon(cx);
                }
            }));
        let body = match row {
            Row::Commit(_) => {
                let commit = self.blame_commit(card.line)?.clone();
                let color = person_color(&commit.author, &p);
                let when =
                    format!("{} \u{b7} {}", long_date(commit.time), snout_core::blame::ago(commit.time, now_secs()));
                let path = commit.path.clone();
                let (id1, id2, id3) = (commit.id.clone(), commit.id.clone(), commit.id.clone());
                let previous = commit.previous.is_some();
                let c2 = commit.clone();
                body.child(
                    div()
                        .flex()
                        .gap(px(10.))
                        .child(avatar(&initials(&commit.author), color, 34., &p))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(commit.author.clone()))
                                .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(commit.email.clone()))
                                .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(when)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .h(px(20.))
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .rounded(px(5.))
                                .bg(p.wash(p.accent))
                                .text_color(p.accent)
                                .font_family(look::mono_font(cx))
                                .text_size(px(11.))
                                .child(crate::components::icon(IconName::GitCommitHorizontal, 12., p.accent))
                                .child(commit.short_id().to_owned()),
                        ),
                )
                .child(
                    div()
                        .id("blame-card-message")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .px(px(9.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .bg(mix(p.fg, 0.04, p.card))
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .whitespace_normal()
                        .children(commit.full_message().lines().map(|l| div().min_h(px(8.)).child(l.to_owned()))),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .child(
                            Button::new("blame-card-diff", IconName::GitCompare)
                                .compact()
                                .label("Show Diff")
                                .debug_selector("blame-card-diff")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.blame.card = None;
                                    cx.emit(EditorEvent::Git(GitRequest::ShowDiff(id1.clone(), path.clone())));
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("blame-card-log", IconName::GitCommitHorizontal)
                                .compact()
                                .label("Show in Git Log")
                                .debug_selector("blame-card-log")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.blame.card = None;
                                    cx.emit(EditorEvent::Git(GitRequest::ShowInLog(id2.clone())));
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("blame-card-copy", IconName::Copy)
                                .compact()
                                .label("Copy Hash")
                                .debug_selector("blame-card-copy")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(id3.clone()));
                                    this.blame.card = None;
                                    this.show_hint(format!("Copied {}", &id3[..id3.len().min(8)]), cx);
                                })),
                        )
                        .child(
                            Button::new("blame-card-previous", IconName::StepBack)
                                .compact()
                                .label("Annotate Previous Revision")
                                .debug_selector("blame-card-previous")
                                .disabled(!previous)
                                .tooltip(if previous {
                                    "The file as it was before this commit"
                                } else {
                                    "This commit brought the file in"
                                })
                                .on_click(cx.listener(move |this, _, w, cx| this.annotate_previous(&c2, w, cx))),
                        ),
                )
            }
            Row::Uncommitted => body.child(
                div()
                    .flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(34.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(mix(p.modified, 0.2, p.card))
                            .child(crate::components::icon(IconName::Pencil, 15., p.modified)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("Not committed yet"))
                            .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(
                                "You changed this line since the last commit. Commit it and the column says so.",
                            )),
                    ),
            ),
            _ => return None,
        };
        Some(
            gpui::deferred(gpui::anchored().position(card.at).snap_to_window_with_margin(px(8.)).child(body))
                .with_priority(2)
                .into_any_element(),
        )
    }

    /// The authors of a class or method, after a click on its hint: who wrote how many lines,
    /// and when last.
    pub(super) fn authors_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.blame.authors.as_ref()?;
        let data = self.blame.data.as_ref()?;
        let p = palette(cx);
        let authors = authors_of(card.decl.lines.clone(), |l| self.blame_row(l), &data.blame);
        let total: usize = authors.iter().map(|a| a.lines).sum();
        let now = now_secs();
        let body = div()
            .id("blame-authors")
            .debug_selector(|| "blame-authors".into())
            .occlude()
            .w(px(340.))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.blame.authors = None;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .pb(px(2.))
                    .child(crate::components::icon(IconName::Users, 14., p.accent))
                    .child(
                        div().font_weight(gpui::FontWeight::SEMIBOLD).child(format!("Authors of {}", card.decl.name)),
                    )
                    .child(div().flex_1())
                    .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(format!("{total} committed lines"))),
            )
            .children(authors.into_iter().map(|a| {
                let share = if total > 0 { a.lines as f32 / total as f32 } else { 0. };
                let color = person_color(&a.name, &p);
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(3.))
                    .child(avatar(&initials(&a.name), color, 24., &p))
                    .child(div().flex_1().min_w_0().flex().flex_col().child(div().child(a.name.clone())).child(
                        div().text_size(px(11.)).text_color(p.fg_dim).child(format!(
                            "{} \u{b7} last {}",
                            a.email,
                            snout_core::blame::ago(a.latest, now)
                        )),
                    ))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(2.))
                            .child(div().text_size(px(11.5)).child(if a.lines == 1 {
                                "1 line".to_owned()
                            } else {
                                format!("{} lines", a.lines)
                            }))
                            .child(
                                div()
                                    .w(px(54.))
                                    .h(px(4.))
                                    .rounded(px(2.))
                                    .bg(mix(p.fg, 0.08, p.card))
                                    .child(div().h_full().rounded(px(2.)).w(px(54. * share)).bg(color)),
                            ),
                    )
            }));
        Some(
            gpui::deferred(gpui::anchored().position(card.at).snap_to_window_with_margin(px(8.)).child(body))
                .with_priority(2)
                .into_any_element(),
        )
    }

    // ----- Earlier revisions -------------------------------------------------------------------

    /// Annotate the file as it was before `commit` (at its parent, under the file's name
    /// there), read only, with a breadcrumb back.
    pub fn annotate_previous(&mut self, commit: &BlameCommit, window: &mut Window, cx: &mut Context<Self>) {
        let Some((rev, path)) = commit.previous.clone() else { return };
        let Some((root, _)) = self.git_rel() else { return };
        // Where the card's line was in the commit: near where it is in the parent.
        let line = self.blame.card.take().map_or(0, |c| {
            let blamed = match self.blame.revisions.last() {
                Some(_) => Some(c.line as u32),
                None => self.blame.map.get(c.line).copied().flatten(),
            };
            blamed.and_then(|b| self.shown_data()?.blame.orig.get(b as usize).copied()).unwrap_or(0) as usize
        });
        self.blame.selected = None;
        let state = cx.new(|cx| EditorState::new(window, cx).line_number(true).soft_wrap(false));
        let language = language_for(Path::new(&path));
        state.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_readonly(true, cx);
        });
        let host = crate::host::host(cx);
        let (r2, p2) = (rev.clone(), path.clone());
        let read = cx.background_executor().spawn(async move { read_blame(&host, &root, &p2, Some(&r2)) });
        let depth = self.blame.revisions.len();
        let task = cx.spawn_in(window, async move |this, cx| {
            let read = read.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(r) = this.blame.revisions.get_mut(depth) else { return };
                match read {
                    Ok(a) => {
                        let text = a.blame.text.join("\n") + "\n";
                        r.state.update(cx, |s, cx| {
                            s.set_value(text, window, cx);
                            let at = s.text().line_start_offset(line.min(a.blame.text.len().saturating_sub(1)));
                            s.set_selected_range(at..at, cx);
                        });
                        r.data = Some(a);
                    }
                    Err(e) => r.error = Some(e),
                }
                cx.notify();
            });
        });
        let label = format!("before {}", commit.short_id());
        self.blame.revisions.push(Revision { rev, path, label, data: None, error: None, state, _task: Some(task) });
        self.blame.shown = true;
        cx.notify();
    }

    /// Go back to the revision at `depth` (0: the file as it is).
    pub fn revision_back(&mut self, depth: usize, cx: &mut Context<Self>) {
        self.blame.revisions.truncate(depth);
        self.blame.card = None;
        self.blame.selected = None;
        cx.notify();
    }

    /// The bar over an earlier revision: where it is, the way back.
    pub(super) fn revision_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let last = self.blame.revisions.last()?;
        let p = palette(cx);
        let depth = self.blame.revisions.len();
        let name = Path::new(&last.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let crumb = |id: SharedString, label: String, to: usize, current: bool, cx: &mut Context<Self>| {
            let name = id.to_string();
            div()
                .id(id)
                .debug_selector(move || name.clone())
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .font_family(look::mono_font(cx))
                .text_size(px(11.5))
                .when(current, |d| d.bg(p.wash(p.accent)).text_color(p.accent).font_weight(gpui::FontWeight::SEMIBOLD))
                .when(!current, |d| {
                    d.text_color(p.fg_dim)
                        .cursor_pointer()
                        .hover(move |s| s.bg(p.button_hover))
                        .on_click(cx.listener(move |this, _, _, cx| this.revision_back(to, cx)))
                })
                .child(label)
        };
        let mut crumbs: Vec<AnyElement> =
            vec![crumb("blame-crumb-0".into(), "Now".into(), 0, false, cx).into_any_element()];
        for (i, r) in self.blame.revisions.iter().enumerate() {
            crumbs.push(crate::components::icon(IconName::ChevronRight, 11., p.fg_dim).into_any_element());
            crumbs.push(
                crumb(format!("blame-crumb-{}", i + 1).into(), r.label.clone(), i + 1, i + 1 == depth, cx)
                    .into_any_element(),
            );
        }
        let status = match (&last.data, &last.error) {
            (_, Some(e)) => Some(e.clone()),
            (None, None) => Some("Reading history\u{2026}".into()),
            _ => None,
        };
        Some(
            div()
                .id("blame-revision-bar")
                .debug_selector(|| "blame-revision-bar".into())
                .flex_none()
                .mx(px(8.))
                .mb(px(4.))
                .px(px(10.))
                .py(px(5.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(6.))
                .bg(p.wash(p.accent))
                .border_1()
                .border_color(mix(p.accent, 0.35, p.panel))
                .child(crate::components::icon(IconName::FileClock, 14., p.accent))
                .child(div().child(format!("{name}:")))
                .children(crumbs)
                .children(status.map(|s| div().text_size(px(11.5)).text_color(p.fg_dim).child(s)))
                .child(div().flex_1())
                .child(div().text_size(px(11.5)).text_color(p.fg_dim).child("Read only"))
                .child(
                    Button::new("blame-revision-back", IconName::ArrowLeft)
                        .compact()
                        .label("Back")
                        .debug_selector("blame-revision-back")
                        .on_click(cx.listener(move |this, _, _, cx| this.revision_back(depth - 1, cx))),
                )
                .child(
                    Button::new("blame-revision-close", IconName::X)
                        .compact()
                        .label("Back to Now")
                        .debug_selector("blame-revision-close")
                        .on_click(cx.listener(|this, _, _, cx| this.revision_back(0, cx))),
                ),
        )
    }

    /// The revision showing, if any: its editor.
    pub fn revision_state(&self) -> Option<Entity<EditorState>> {
        self.blame.revisions.last().map(|r| r.state.clone())
    }

    // ----- After the end of the line: the author hints and the line's blame ------------------

    /// The caret moved: the line's blame shows once it rests on a line.
    pub(super) fn blame_caret_moved(&mut self, cx: &mut Context<Self>) {
        let line = self.state.read(cx).cursor_position().line as usize;
        if self.blame.line_seen == Some(line) {
            return;
        }
        self.blame.line_seen = Some(line);
        if self.blame.line.take().is_some() {
            cx.notify();
        }
        if !self.line_blame_wanted(cx) {
            self.blame._line = None;
            return;
        }
        let delay = EditorDefaults::now(cx).hover_delay;
        self.blame._line = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |this, cx| {
                this.blame.line = Some(line);
                cx.notify();
            });
        }));
    }

    /// What the caret line's blame says: "Ada Lovelace, 3 years ago · Fix the totals".
    pub fn line_blame_text(&self, line: usize, cx: &App) -> Option<String> {
        match self.blame_row(line) {
            Row::Commit(_) => {
                let c = self.blame_commit(line)?;
                Some(format!("{}, {} \u{b7} {}", c.author, snout_core::blame::ago(c.time, now_secs()), c.summary))
            }
            Row::Uncommitted => Some("You, not committed yet".into()),
            Row::Agent(e) => {
                let edit = self.agent_edits.get(e as usize)?;
                let a = crate::agents::by_id(&edit.agent, cx);
                let mut s = format!("{}, {}", a.short, crate::claude_ide::ledger::clock(edit.time()));
                if let Some(p) = &edit.prompt {
                    s.push_str(&format!(" \u{b7} {p}"));
                }
                Some(s)
            }
            Row::Blank => None,
        }
    }

    /// The author hint of each declaration in sight, and the caret line's blame, after the end
    /// of their lines (after the parameter hints there).
    pub(super) fn blame_hints(&self, cx: &App) -> Option<impl IntoElement + use<>> {
        let data = self.blame.data.clone()?;
        if !self.blame.revisions.is_empty() {
            return None;
        }
        let hints = self.hints_wanted(cx).then(|| self.blame.decls.clone());
        let line_text = self
            .blame
            .line
            .filter(|_| self.line_blame_wanted(cx))
            .and_then(|l| Some((l, self.line_blame_text(l, cx)?)));
        if hints.as_ref().is_none_or(|h| h.is_empty()) && line_text.is_none() {
            self.blame.painted.borrow_mut().hints.clear();
            return None;
        }
        let p = palette(cx);
        let state = self.state.clone();
        let map = self.blame.map.clone();
        let owners = self.agent_lines.clone();
        let tails = self.blame.tails.clone();
        let painted = self.blame.painted.clone();
        let ui = look::ui_font(cx);
        let size_pt = font_size(cx);
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, window, cx| {
                    let s = state.read(cx);
                    let Some(line_height) = s.line_height() else { return };
                    let rope = s.text();
                    let visible = visible_lines(s);
                    let shown = |l: usize| visible.iter().any(|(v, _)| *v == l);
                    let row = |line: usize| {
                        let agent = owners.as_ref().and_then(|o| o.at(line));
                        row_of(line, Some(&data.blame), Some(&map), agent)
                    };
                    let small = px((size_pt * 0.82).max(9.));
                    let ts = window.text_system().clone();
                    let shape = |text: String, color: Hsla, italic: bool| {
                        let run = TextRun {
                            len: text.len(),
                            font: gpui::Font {
                                style: if italic { gpui::FontStyle::Italic } else { gpui::FontStyle::Normal },
                                ..gpui::font(ui.clone())
                            },
                            color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        ts.shape_line(text.into(), small, &[run], None)
                    };
                    let end_x = |line: usize| -> Option<Pixels> {
                        let end = rope.line_end_offset(line);
                        let b = s.range_to_bounds(&(end..end))?;
                        Some(tails.borrow().get(&line).copied().unwrap_or(b.right() + px(16.)).max(b.right() + px(16.)))
                    };
                    let mut drawn = Vec::new();
                    let mut texts = Vec::new();
                    let mut after: HashMap<usize, Pixels> = HashMap::new();
                    for (i, d) in hints.iter().flat_map(|h| h.iter().enumerate()) {
                        if d.line >= rope.lines_len() || !shown(d.line) {
                            continue;
                        }
                        let authors = authors_of(d.lines.clone(), row, &data.blame);
                        let Some(label) = hint_label(&authors) else { continue };
                        let Some(x) = end_x(d.line) else { continue };
                        let at = rope.line_start_offset(d.line);
                        let Some(top) = s.range_to_bounds(&(at..at)).map(|b| b.top()) else { continue };
                        let color = person_color(&authors[0].name, &p);
                        let r = (line_height * 0.62).max(px(12.));
                        let dot = Bounds::new(point(x, top + (line_height - r) / 2.), size(r, r));
                        window.paint_quad(gpui::fill(dot, mix(color, 0.85, p.panel)).corner_radii(r / 2.));
                        let ini = initials(&authors[0].name);
                        let run = TextRun {
                            len: ini.len(),
                            font: gpui::Font { weight: gpui::FontWeight::BOLD, ..gpui::font(ui.clone()) },
                            color: p.on_badge,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped_ini = ts.shape_line(ini.into(), r * 0.48, &[run], None);
                        texts.push((point(dot.center().x - shaped_ini.width / 2., dot.top()), r, shaped_ini));
                        let shaped = shape(label, p.fg_dim, false);
                        let tx = dot.right() + px(5.);
                        let width = shaped.width;
                        texts.push((point(tx, top), line_height, shaped));
                        let hit = Bounds::new(point(x - px(2.), top), size(tx + width - x + px(4.), line_height));
                        drawn.push((i, hit));
                        after.insert(d.line, hit.right() + px(14.));
                    }
                    if let Some((line, text)) = &line_text
                        && *line < rope.lines_len()
                        && shown(*line)
                        && let Some(x) = end_x(*line)
                    {
                        let x = after.get(line).copied().unwrap_or(x).max(x);
                        let at = rope.line_start_offset(*line);
                        if let Some(top) = s.range_to_bounds(&(at..at)).map(|b| b.top()) {
                            let shaped = shape(text.clone(), p.fg_dim.opacity(0.75), true);
                            texts.push((point(x, top), line_height, shaped));
                        }
                    }
                    painted.borrow_mut().hints = drawn;
                    for (at, h, shaped) in texts {
                        let _ = shaped.paint(at, h, TextAlign::Left, None, window, cx);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}

/// A round avatar with `text` (initials) on `color`.
fn avatar(text: &str, color: Hsla, d: f32, p: &Palette) -> gpui::Div {
    div()
        .flex_none()
        .size(px(d))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color)
        .text_color(p.on_badge)
        .text_size(px(d * 0.4))
        .font_weight(gpui::FontWeight::BOLD)
        .child(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use snout_core::blame::BlameCommit;

    fn blame() -> Blame {
        let c = |id: &str, author: &str, time: i64| BlameCommit {
            id: id.repeat(40),
            author: author.into(),
            email: format!("{}@example.com", author.to_lowercase().replace(' ', ".")),
            time,
            ..Default::default()
        };
        Blame {
            commits: vec![c("a", "Ada Lovelace", 100), c("b", "Grace Hopper", 300), c("c", "Ada Lovelace", 200)],
            lines: vec![0, 0, 1, 2, 1],
            orig: vec![0, 1, 2, 3, 4],
            text: ["a", "b", "c", "d", "e"].map(str::to_owned).to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn rows_say_the_commit_or_why_not() {
        let b = blame();
        let map = vec![Some(0), None, Some(2), Some(3), Some(4)];
        assert_eq!(row_of(0, Some(&b), Some(&map), None), Row::Commit(0));
        assert_eq!(row_of(1, Some(&b), Some(&map), None), Row::Uncommitted);
        assert_eq!(row_of(1, Some(&b), Some(&map), Some(7)), Row::Agent(7), "an agent's uncommitted line");
        assert_eq!(row_of(2, Some(&b), Some(&map), Some(7)), Row::Commit(1), "committed: Git says who");
        assert_eq!(row_of(5, Some(&b), Some(&map), None), Row::Blank, "the empty line after the last");
        assert_eq!(row_of(4, Some(&b), None, None), Row::Commit(1), "a revision's own lines");
        assert_eq!(row_of(3, None, None, Some(2)), Row::Agent(2), "no Git: the agents only");
        assert_eq!(row_of(3, None, None, None), Row::Blank);
    }

    #[test]
    fn age_shades_recent_lines_stronger_and_stays_readable() {
        let b = blame();
        assert_eq!(age_ranks(&b), [0., 1., 0.5]);
        assert!(shade(1.) > shade(0.5) && shade(0.5) > shade(0.));
        assert!((shade(0.) - SHADE_MIN).abs() < 1e-6 && (shade(1.) - SHADE_MAX).abs() < 1e-6);
        assert_eq!(age_ranks(&Blame { commits: vec![BlameCommit::default()], ..Default::default() }), [1.]);
        // The text stays readable on the strongest shade, in every theme, light and dark.
        for theme in crate::settings::ColorTheme::ALL {
            for dark in [false, true] {
                let p = Palette::new(theme, dark);
                let ground = shade_color(1., &p);
                let c = crate::theme::contrast(p.fg, ground);
                assert!(c >= 4.5, "{theme:?} dark={dark}: names {c:.2}");
                let c = crate::theme::contrast(p.weak(), ground);
                assert!(c >= 3., "{theme:?} dark={dark}: dates {c:.2}");
                let selected = mix(p.accent, 0.42, p.panel);
                assert!(crate::theme::contrast(p.fg, selected) >= 3.5, "{theme:?} dark={dark}: the lit commit");
            }
        }
    }

    #[test]
    fn edits_read_as_lines() {
        // A line typed before an indented one: found inside the indentation, still a whole line.
        assert_eq!(line_edit("    ", 3, 3, false, "", "// new\n    "), (3, 3, 1, true));
        assert_eq!(line_edit("", 3, 3, true, "", "x\ny\n"), (3, 3, 2, true));
        // The same line taken out again.
        assert_eq!(line_edit("    ", 3, 4, false, "// new\n    ", ""), (3, 4, 0, true));
        assert_eq!(line_edit("", 3, 5, true, "a\nb\n", ""), (3, 5, 0, true));
        // Typing inside a line, and Return in one.
        assert_eq!(line_edit("    if", 3, 3, false, "", "x"), (3, 3, 0, false));
        assert_eq!(line_edit("    if (a)", 3, 3, false, "", "\n    "), (3, 3, 1, false));
        // A selection over two lines typed over.
        assert_eq!(line_edit("  ", 2, 3, false, "a\n  b", "c"), (2, 3, 0, false));
    }

    #[test]
    fn names_shorten_when_narrow() {
        assert_eq!(initials("Ada Lovelace"), "AL");
        assert_eq!(initials("grace"), "G");
        assert_eq!(initials("Jean-Luc de la Mer"), "JM");
        assert_eq!(initials(""), "?");
        assert_eq!(short_name("Ada King Lovelace"), "Ada L.");
        assert_eq!(Mode::for_width(1200.).author("Ada Lovelace"), "Ada Lovelace");
        assert_eq!(Mode::for_width(800.).author("Ada Lovelace"), "Ada L.");
        assert_eq!(Mode::for_width(500.).author("Ada Lovelace"), "AL");
        assert_eq!(cut("Bartholomew Montgomery-Smythe", 10), "Bartholom\u{2026}");
        assert_eq!(short_date(0).len(), 10);
    }

    #[test]
    fn author_hints_count_the_main_author_and_the_others() {
        let b = blame();
        let row = |l: usize| row_of(l, Some(&b), None, None);
        let all = authors_of(0..5, row, &b);
        assert_eq!(
            all.iter().map(|a| (a.name.as_str(), a.lines)).collect::<Vec<_>>(),
            [("Ada Lovelace", 3), ("Grace Hopper", 2)]
        );
        assert_eq!(all[0].latest, 200);
        assert_eq!(hint_label(&all).as_deref(), Some("Ada Lovelace +1"));
        assert_eq!(hint_label(&authors_of(0..2, row, &b)).as_deref(), Some("Ada Lovelace"));
        assert_eq!(hint_label(&[]), None);
    }

    #[test]
    fn declarations_are_classes_and_methods() {
        let java = "package p;\n\n/** Doc. */\npublic class Shop {\n    @Override\n    public String toString() {\n        return \"\";\n    }\n\n    Shop() {}\n\n    interface Inner { void go(); }\n}\n";
        let d = declarations(java, "java");
        let got: Vec<(&str, usize, Range<usize>)> =
            d.iter().map(|d| (d.name.as_str(), d.line, d.lines.clone())).collect();
        assert_eq!(
            got,
            [("Shop", 3, 3..13), ("toString", 5, 4..8), ("Shop", 9, 9..10), ("Inner", 11, 11..12), ("go", 11, 11..12)]
        );
        let kotlin = "class Cart {\n    fun total(): Int {\n        return 1\n    }\n}\nobject Registry\n";
        let names: Vec<String> = declarations(kotlin, "kotlin").into_iter().map(|d| d.name).collect();
        assert_eq!(names, ["Cart", "total", "Registry"]);
        assert!(declarations("x", "yaml").is_empty());
    }
}
