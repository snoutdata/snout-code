//! Find in Files (⇧⌘F) and Replace in Files (⇧⌘R): one window over the workspace with the
//! text to look for (plain, or a regular expression; case and whole words as asked), what to
//! put in its place, a file mask (`*.java, !*Test.java`) and a scope (the project, a module,
//! a folder, the open files). The matches show as a tree, grouped by file, each match and
//! each file ticked to be replaced or not, the change the selected row makes previewed
//! under it. Replace Selected and Replace All write through the host (so a project on a
//! remote machine works the same); an open editor takes the new text at once, as an edit of
//! its own. One undo (Edit ▸ Undo Replace in Files) puts every file back as it was, unless
//! it changed since.
//!
//! The search itself ([`search`]) and the text side of a replacement ([`Query`], [`find`],
//! [`replaced`], [`Mask`]) carry no UI, for the tests.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    StrikethroughStyle, Styled as _, StyledText, Task, Window, div, px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use regex::{Regex, RegexBuilder};
use snout_host::{Host, HostRef};

use crate::components::{self, Button, IconButton, IconName, RowState, Tone};
use crate::look;
use crate::search::{SearchClose, SearchDown, SearchUp};
use crate::theme::{Palette, palette};
use crate::workspace::Workspace;

/// The most matches one search shows; a search finding more says so.
pub const MAX_MATCHES: usize = 5_000;
/// Files larger than this are not searched.
const MAX_FILE: usize = 4 << 20;
/// Lines of context around a change in the preview.
const CONTEXT: usize = 2;

// ----- The text side ------------------------------------------------------------------------

/// What to look for and what to put in its place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub replace: String,
    /// Case matters.
    pub case: bool,
    /// Only whole words.
    pub word: bool,
    /// `text` is a regular expression, and `replace` may name its groups (`$1`, `${name}`).
    pub regex: bool,
}

impl Query {
    /// The pattern the search runs, or why `text` is not one.
    pub fn compile(&self) -> Result<Regex, String> {
        if self.text.is_empty() {
            return Err(String::new());
        }
        let mut pattern = if self.regex { self.text.clone() } else { regex::escape(&self.text) };
        if self.word {
            pattern = format!(r"\b(?:{pattern})\b");
        }
        RegexBuilder::new(&pattern).case_insensitive(!self.case).multi_line(true).size_limit(1 << 22).build().map_err(
            |e| match e {
                regex::Error::Syntax(s) => {
                    format!(
                        "Not a regular expression: {}",
                        s.lines().last().unwrap_or("").trim().trim_start_matches("error: ")
                    )
                }
                _ => "The regular expression is too large".to_owned(),
            },
        )
    }

    /// What replaces the match `caps`.
    fn replacement(&self, caps: &regex::Captures) -> String {
        if self.regex {
            let mut out = String::new();
            caps.expand(&self.replace, &mut out);
            out
        } else {
            self.replace.clone()
        }
    }
}

/// A match in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Bytes of the file.
    pub range: Range<usize>,
    /// From 0.
    pub line: u32,
    /// From 0, in characters.
    pub column: u32,
    /// What replaces it.
    pub replacement: String,
}

/// Every match of `re` in `text`, at most `limit`, with what `query` puts in each one's place.
/// Empty matches are left out.
pub fn find(text: &str, query: &Query, re: &Regex, limit: usize) -> Vec<Hit> {
    let mut out = Vec::new();
    let (mut line, mut line_start, mut seen) = (0u32, 0usize, 0usize);
    for caps in re.captures_iter(text) {
        let m = caps.get(0).expect("group 0 is the match");
        if m.start() == m.end() {
            continue;
        }
        for (i, b) in text.as_bytes()[seen..m.start()].iter().enumerate() {
            if *b == b'\n' {
                line += 1;
                line_start = seen + i + 1;
            }
        }
        seen = m.start();
        out.push(Hit {
            range: m.range(),
            line,
            column: text[line_start..m.start()].chars().count() as u32,
            replacement: query.replacement(&caps),
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// `text` with the matches `hits` (in order, not overlapping) replaced.
pub fn replaced<'a>(text: &str, hits: impl IntoIterator<Item = &'a Hit>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for h in hits {
        if h.range.start < at {
            continue;
        }
        out.push_str(&text[at..h.range.start]);
        out.push_str(&h.replacement);
        at = h.range.end;
    }
    out.push_str(&text[at..]);
    out
}

/// Which files a search looks at, by name or path: `*.java, *.kt` takes those, `!*Test.java`
/// leaves those out. A pattern with a `/` is matched against the path from the project's
/// folder, any other against the file's name. Empty takes every file.
#[derive(Clone, Debug, Default)]
pub struct Mask {
    take: Vec<globset::GlobMatcher>,
    leave: Vec<globset::GlobMatcher>,
}

impl Mask {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut mask = Self::default();
        for part in text.split([',', ';']).map(str::trim).filter(|p| !p.is_empty()) {
            let (leave, pattern) = match part.strip_prefix('!') {
                Some(rest) => (true, rest.trim()),
                None => (false, part),
            };
            let glob = globset::GlobBuilder::new(pattern)
                .literal_separator(pattern.contains('/'))
                .build()
                .map_err(|e| format!("Not a file mask: {}", e.kind()))?
                .compile_matcher();
            if leave { mask.leave.push(glob) } else { mask.take.push(glob) }
        }
        Ok(mask)
    }

    /// Whether the file at `path` (from the project's folder) is searched.
    pub fn takes(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        let hit = |g: &globset::GlobMatcher| g.is_match(name) || g.is_match(path);
        (self.take.is_empty() || self.take.iter().any(hit)) && !self.leave.iter().any(hit)
    }
}

/// Where a search looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    Project,
    Module,
    Directory,
    OpenFiles,
}

impl Scope {
    pub const ALL: [Scope; 4] = [Scope::Project, Scope::Module, Scope::Directory, Scope::OpenFiles];

    pub fn label(self) -> &'static str {
        match self {
            Scope::Project => "Project",
            Scope::Module => "Module",
            Scope::Directory => "Directory",
            Scope::OpenFiles => "Open Files",
        }
    }
}

/// The files of a scope, as paths from the project's folder.
#[derive(Clone, Debug, Default)]
pub enum Within {
    #[default]
    All,
    /// Under one of the first folders, and under none of the second.
    Under(Vec<String>, Vec<String>),
    Files(HashSet<String>),
}

impl Within {
    pub fn takes(&self, path: &str) -> bool {
        let under = |dir: &String| dir.is_empty() || path.starts_with(&format!("{}/", dir.trim_end_matches('/')));
        match self {
            Within::All => true,
            Within::Under(take, leave) => take.iter().any(under) && !leave.iter().any(under),
            Within::Files(files) => files.contains(path),
        }
    }
}

/// The matches in one file.
#[derive(Clone, Debug)]
pub struct FileHits {
    /// From the project's folder.
    pub path: String,
    /// The text searched: the open editor's, or the file's.
    pub text: Arc<String>,
    pub hits: Vec<Hit>,
    /// Which of them are to be replaced.
    pub included: Vec<bool>,
    pub expanded: bool,
}

impl FileHits {
    /// The file's text with its ticked matches (those of `only`, when given) replaced.
    pub fn after(&self, only: Option<usize>) -> String {
        replaced(
            &self.text,
            self.hits
                .iter()
                .enumerate()
                .filter(|(i, _)| self.included[*i] && only.is_none_or(|o| o == *i))
                .map(|(_, h)| h),
        )
    }

    pub fn ticked(&self) -> usize {
        self.included.iter().filter(|i| **i).count()
    }
}

/// What a search found.
#[derive(Clone, Debug, Default)]
pub struct Found {
    pub files: Vec<FileHits>,
    /// It stopped at [`MAX_MATCHES`].
    pub truncated: bool,
}

/// Search `files` of `root` on `host` (those `within` and `mask` take) for `query`. The text
/// of a file open in an editor is taken from `open` (by path from `root`), as the editor has
/// it. A plain search asks the host which files hold the text first, in one call (a remote
/// machine answers it there), and reads only those.
pub fn search(
    host: &dyn Host,
    root: &Path,
    files: &[String],
    query: &Query,
    within: &Within,
    mask: &Mask,
    open: &HashMap<String, Arc<String>>,
) -> Result<Found, String> {
    let re = query.compile()?;
    let mut candidates: Vec<String> = files.iter().filter(|f| within.takes(f) && mask.takes(f)).cloned().collect();
    for f in open.keys() {
        if within.takes(f) && mask.takes(f) && !candidates.contains(f) {
            candidates.push(f.clone());
        }
    }
    // A plain text without line breaks: the host narrows the files down (case left aside).
    if !query.regex && !query.text.contains('\n') && candidates.len() > 1 {
        let needle = query.text.to_lowercase();
        let disk: Vec<String> = candidates.iter().filter(|f| !open.contains_key(*f)).cloned().collect();
        if let Ok(hits) = host.find_text(root, &disk, &needle, usize::MAX / 2) {
            let holding: HashSet<String> = hits.into_iter().map(|h| h.path).collect();
            candidates.retain(|f| open.contains_key(f) || holding.contains(f));
        }
    }
    candidates.sort();
    let mut found = Found::default();
    let mut total = 0;
    for path in candidates {
        let text = match open.get(&path) {
            Some(t) => t.clone(),
            None => match host.read(&root.join(&path)) {
                Ok(bytes) if bytes.len() <= MAX_FILE && !bytes.contains(&0) => match String::from_utf8(bytes) {
                    Ok(s) => Arc::new(s),
                    Err(_) => continue,
                },
                _ => continue,
            },
        };
        let hits = find(&text, query, &re, MAX_MATCHES - total);
        if hits.is_empty() {
            continue;
        }
        total += hits.len();
        let included = vec![true; hits.len()];
        found.files.push(FileHits { path, text, hits, included, expanded: true });
        if total >= MAX_MATCHES {
            found.truncated = true;
            break;
        }
    }
    Ok(found)
}

/// One line of the preview: its numbers in the old and the new text, and how it changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLine {
    pub old: Option<usize>,
    pub new: Option<usize>,
    /// `-` taken out, `+` put in, ` ` the same, `…` lines left out.
    pub sign: char,
    pub text: String,
}

/// The lines that change from `before` to `after`, with [`CONTEXT`] lines around each run.
pub fn preview(before: &str, after: &str, max: usize) -> Vec<PreviewLine> {
    let old: Vec<&str> = before.split_inclusive('\n').collect();
    let new: Vec<&str> = after.split_inclusive('\n').collect();
    let clean = |s: &str| s.trim_end_matches(['\n', '\r']).replace('\t', "    ");
    let mut out: Vec<PreviewLine> = Vec::new();
    let mut shown = 0usize; // old lines shown up to here
    for h in crate::claude_ide::review::hunks(before, after) {
        let from = h.old_lines.start.saturating_sub(CONTEXT).max(shown);
        if from > shown && !out.is_empty() {
            out.push(PreviewLine { old: None, new: None, sign: '…', text: String::new() });
        }
        let shift = h.new_lines.start as isize - h.old_lines.start as isize;
        let same = |i: usize, line: &str, shift: isize| PreviewLine {
            old: Some(i + 1),
            new: Some((i as isize + shift) as usize + 1),
            sign: ' ',
            text: clean(line),
        };
        for (i, line) in old.iter().enumerate().take(h.old_lines.start).skip(from) {
            out.push(same(i, line, shift));
        }
        for i in h.old_lines.clone() {
            out.push(PreviewLine { old: Some(i + 1), new: None, sign: '-', text: clean(old[i]) });
        }
        for i in h.new_lines.clone() {
            out.push(PreviewLine { old: None, new: Some(i + 1), sign: '+', text: clean(new[i]) });
        }
        let end = (h.old_lines.end + CONTEXT).min(old.len());
        let shift = h.new_lines.end as isize - h.old_lines.end as isize;
        for (i, line) in old.iter().enumerate().take(end).skip(h.old_lines.end) {
            out.push(same(i, line, shift));
        }
        shown = end;
        if out.len() >= max {
            out.truncate(max);
            break;
        }
    }
    out
}

/// What a match row shows: the line around the match, cut to fit, and where the match is in it.
pub fn snippet(text: &str, hit: &Hit) -> (String, Range<usize>) {
    let start = text[..hit.range.start].rfind('\n').map_or(0, |i| i + 1);
    let end = text[hit.range.start..].find('\n').map_or(text.len(), |i| hit.range.start + i);
    let end_of_match = hit.range.end.min(end);
    let mut from = start + (text[start..hit.range.start].len() - text[start..hit.range.start].trim_start().len());
    // Keep at most about 40 characters in front of the match.
    let before = &text[from..hit.range.start];
    if before.chars().count() > 40 {
        let cut = before.char_indices().rev().nth(39).map_or(0, |(i, _)| i);
        from += cut;
    }
    let ellipsis =
        from > start + (text[start..hit.range.start].len() - text[start..hit.range.start].trim_start().len());
    let mut to = end;
    if text[end_of_match..end].chars().count() > 120 {
        to = end_of_match + text[end_of_match..end].char_indices().nth(120).map_or(0, |(i, _)| i);
    }
    let lead = if ellipsis { "…" } else { "" };
    let line = format!("{lead}{}", &text[from..to]).replace('\t', " ");
    let m = lead.len() + (hit.range.start - from)..lead.len() + (end_of_match - from);
    (line, m)
}

/// A change Replace makes to one file: its text before (as the search read it) and after.
#[derive(Clone, Debug)]
pub struct FileChange {
    pub path: PathBuf,
    pub before: Arc<String>,
    pub after: Arc<String>,
    /// How many matches it replaces.
    pub count: usize,
}

/// The last replacement, to undo.
#[derive(Clone, Debug, Default)]
pub struct ReplaceUndo(pub Vec<FileChange>);

// ----- The window ---------------------------------------------------------------------------

/// A row of the tree: a file, or one of its matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    File(usize),
    Hit(usize, usize),
}

pub enum FindEvent {
    Close,
    /// Open the file at a line and column.
    Open(PathBuf, (u32, u32)),
    /// Write these changes.
    Replace(Vec<FileChange>),
}

/// The texts of the open editors, by path from the project's folder.
pub type OpenTexts = std::rc::Rc<dyn Fn(&App) -> HashMap<String, Arc<String>>>;

pub struct FindInFiles {
    pub root: PathBuf,
    host: HostRef,
    /// The replace field shows.
    pub replace_mode: bool,
    pub find: Entity<InputState>,
    pub replace: Entity<InputState>,
    pub mask: Entity<InputState>,
    pub directory: Entity<InputState>,
    pub case: bool,
    pub word: bool,
    pub regex: bool,
    pub scope: Scope,
    /// The modules (name, folder) and the one the Module scope takes.
    pub modules: Vec<(String, PathBuf)>,
    pub module: Option<usize>,
    open: OpenTexts,
    files: Arc<Vec<String>>,
    listed: bool,
    _listing: Task<()>,
    pub found: Found,
    pub rows: Vec<Row>,
    pub selected: usize,
    /// Why there is nothing to show: a pattern or mask that is not one.
    pub error: Option<String>,
    pub searching: bool,
    _search: Option<Task<()>>,
    /// What the last replacement did, for the line at the bottom.
    pub note: Option<String>,
    focused: bool,
    focus: FocusHandle,
}

impl EventEmitter<FindEvent> for FindInFiles {}

impl FindInFiles {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: PathBuf,
        replace_mode: bool,
        initial: Option<String>,
        modules: Vec<(String, PathBuf)>,
        module: Option<usize>,
        directory: Option<String>,
        open: OpenTexts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |hint: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(hint))
        };
        let find = field("Text to find", window, cx);
        let replace = field("Replace with", window, cx);
        let mask = field("File mask: *.java, !*Test.java", window, cx);
        let dir = field("Folder, from the project's", window, cx);
        if let Some(text) = initial.filter(|t| !t.is_empty() && !t.contains('\n')) {
            find.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        if let Some(d) = directory {
            dir.update(cx, |s, cx| s.set_value(d, window, cx));
        }
        for input in [&find, &replace, &mask, &dir] {
            cx.subscribe(input, |this, input, e: &InputEvent, cx| match e {
                InputEvent::Change => {
                    if input == this.replace {
                        this.replacement_changed(cx);
                    } else {
                        this.search(cx);
                    }
                }
                InputEvent::PressEnter { secondary, .. } => {
                    if *secondary && this.replace_mode {
                        this.replace_all(cx);
                    } else {
                        this.open_selected(cx);
                    }
                }
                _ => {}
            })
            .detach();
        }
        let host = crate::host::host(cx);
        let listing = {
            let list = cx.background_executor().spawn({
                let (host, root) = (host.clone(), root.clone());
                async move { crate::search::list_files(&*host, &root) }
            });
            cx.spawn(async move |this, cx| {
                let files = list.await;
                let _ = this.update(cx, |this, cx| {
                    this.files = Arc::new(files);
                    this.listed = true;
                    this.search(cx);
                });
            })
        };
        Self {
            root,
            host,
            replace_mode,
            find,
            replace,
            mask,
            directory: dir,
            case: false,
            word: false,
            regex: false,
            scope: Scope::Project,
            modules,
            module,
            open,
            files: Arc::new(Vec::new()),
            listed: false,
            _listing: listing,
            found: Found::default(),
            rows: Vec::new(),
            selected: 0,
            error: None,
            searching: false,
            _search: None,
            note: None,
            focused: false,
            focus: cx.focus_handle(),
        }
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.find.update(cx, |s, cx| s.focus(window, cx));
    }

    /// The query the fields make.
    pub fn query(&self, cx: &App) -> Query {
        Query {
            text: self.find.read(cx).value().to_string(),
            replace: self.replace.read(cx).value().to_string(),
            case: self.case,
            word: self.word,
            regex: self.regex,
        }
    }

    pub fn set_replace_mode(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_mode = on;
        if on {
            self.replace.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    pub fn toggle(&mut self, which: &str, cx: &mut Context<Self>) {
        match which {
            "case" => self.case = !self.case,
            "word" => self.word = !self.word,
            _ => self.regex = !self.regex,
        }
        self.search(cx);
    }

    pub fn set_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.scope = scope;
        self.search(cx);
    }

    pub fn set_module(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.module = Some(ix);
        self.scope = Scope::Module;
        self.search(cx);
    }

    /// The files the scope takes.
    fn within(&self, cx: &App) -> Within {
        let rel = |p: &Path| p.strip_prefix(&self.root).ok().map(|p| p.to_string_lossy().into_owned());
        match self.scope {
            Scope::Project => Within::All,
            Scope::Directory => {
                let d = self.directory.read(cx).value().trim().trim_matches('/').to_string();
                Within::Under(vec![d], Vec::new())
            }
            Scope::OpenFiles => Within::Files((self.open)(cx).into_keys().collect()),
            Scope::Module => {
                let Some((_, dir)) = self.module.and_then(|i| self.modules.get(i)) else { return Within::All };
                let Some(own) = rel(dir) else { return Within::Files(HashSet::new()) };
                // Not the modules inside it.
                let inner: Vec<String> = self
                    .modules
                    .iter()
                    .filter(|(_, d)| d != dir && d.starts_with(dir))
                    .filter_map(|(_, d)| rel(d))
                    .collect();
                Within::Under(vec![own], inner)
            }
        }
    }

    /// Search again for what the fields say, once the typing pauses.
    pub fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let mask = match Mask::parse(&self.mask.read(cx).value()) {
            Ok(m) => m,
            Err(e) => {
                self.show(Err(e), cx);
                return;
            }
        };
        if query.text.is_empty() {
            self.show(Ok(Found::default()), cx);
            return;
        }
        if let Err(e) = query.compile() {
            self.show(Err(e), cx);
            return;
        }
        if !self.listed {
            return;
        }
        let within = self.within(cx);
        let open = (self.open)(cx);
        let (host, root, files) = (self.host.clone(), self.root.clone(), self.files.clone());
        self.searching = true;
        cx.notify();
        self._search = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(150)).await;
            let found = cx
                .background_executor()
                .spawn(async move { search(&*host, &root, &files, &query, &within, &mask, &open) })
                .await;
            let _ = this.update(cx, |this, cx| this.show(found, cx));
        }));
    }

    /// The replacement changed: the matches stay, what replaces them is worked out again.
    fn replacement_changed(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let Ok(re) = query.compile() else { return };
        for f in &mut self.found.files {
            for h in &mut f.hits {
                if let Some(caps) =
                    re.captures_at(&f.text, h.range.start).filter(|c| c.get(0).unwrap().range() == h.range)
                {
                    h.replacement = query.replacement(&caps);
                }
            }
        }
        cx.notify();
    }

    fn show(&mut self, found: Result<Found, String>, cx: &mut Context<Self>) {
        self.searching = false;
        match found {
            Ok(found) => {
                // What was unticked stays unticked when the same match is found again.
                let before: HashMap<(String, usize), bool> = self
                    .found
                    .files
                    .iter()
                    .flat_map(|f| f.hits.iter().zip(&f.included).map(|(h, i)| ((f.path.clone(), h.range.start), *i)))
                    .collect();
                let closed: HashSet<String> =
                    self.found.files.iter().filter(|f| !f.expanded).map(|f| f.path.clone()).collect();
                self.found = found;
                for f in &mut self.found.files {
                    f.expanded = !closed.contains(&f.path);
                    for (h, inc) in f.hits.iter().zip(f.included.iter_mut()) {
                        if let Some(was) = before.get(&(f.path.clone(), h.range.start)) {
                            *inc = *was;
                        }
                    }
                }
                self.error = None;
            }
            Err(e) => {
                self.found = Found::default();
                self.error = Some(e).filter(|e| !e.is_empty());
            }
        }
        self.rebuild_rows();
        // The first match, rather than its file.
        self.selected = if self.rows.len() > 1 { 1 } else { 0 };
        cx.notify();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        for (fi, f) in self.found.files.iter().enumerate() {
            self.rows.push(Row::File(fi));
            if f.expanded {
                self.rows.extend((0..f.hits.len()).map(|hi| Row::Hit(fi, hi)));
            }
        }
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }

    pub fn match_count(&self) -> usize {
        self.found.files.iter().map(|f| f.hits.len()).sum()
    }

    pub fn ticked_count(&self) -> usize {
        self.found.files.iter().map(FileHits::ticked).sum()
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as isize;
        self.selected = (self.selected as isize + by).rem_euclid(n) as usize;
        cx.notify();
    }

    pub fn toggle_file(&mut self, fi: usize, cx: &mut Context<Self>) {
        if let Some(f) = self.found.files.get_mut(fi) {
            let on = f.ticked() < f.hits.len();
            f.included.iter_mut().for_each(|i| *i = on);
        }
        cx.notify();
    }

    pub fn toggle_hit(&mut self, fi: usize, hi: usize, cx: &mut Context<Self>) {
        if let Some(i) = self.found.files.get_mut(fi).and_then(|f| f.included.get_mut(hi)) {
            *i = !*i;
        }
        cx.notify();
    }

    pub fn toggle_expanded(&mut self, fi: usize, cx: &mut Context<Self>) {
        let row = self.rows.get(self.selected).copied();
        if let Some(f) = self.found.files.get_mut(fi) {
            f.expanded = !f.expanded;
        }
        self.rebuild_rows();
        // The selection stays on its row, or on the file of a match folded away.
        let keep = match row {
            Some(Row::Hit(f, _)) if f == fi => Row::File(fi),
            Some(r) => r,
            None => Row::File(fi),
        };
        self.selected = self.rows.iter().position(|r| *r == keep).unwrap_or(0);
        cx.notify();
    }

    pub fn open_selected(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(self.selected).copied() else { return };
        let (fi, hi) = match row {
            Row::File(fi) => (fi, 0),
            Row::Hit(fi, hi) => (fi, hi),
        };
        let Some(f) = self.found.files.get(fi) else { return };
        let at = f.hits.get(hi).map_or((0, 0), |h| (h.line, h.column));
        cx.emit(FindEvent::Open(self.root.join(&f.path), at));
    }

    /// The changes the selected row makes (a match, or a file's ticked matches).
    pub fn selected_changes(&self) -> Vec<FileChange> {
        let Some(row) = self.rows.get(self.selected).copied() else { return Vec::new() };
        let (fi, only) = match row {
            Row::File(fi) => (fi, None),
            Row::Hit(fi, hi) => (fi, Some(hi)),
        };
        let Some(f) = self.found.files.get(fi) else { return Vec::new() };
        let count = match only {
            Some(hi) => usize::from(f.included[hi]),
            None => f.ticked(),
        };
        if count == 0 {
            return Vec::new();
        }
        vec![FileChange {
            path: self.root.join(&f.path),
            before: f.text.clone(),
            after: Arc::new(f.after(only)),
            count,
        }]
    }

    /// The changes every ticked match makes.
    pub fn all_changes(&self) -> Vec<FileChange> {
        self.found
            .files
            .iter()
            .filter(|f| f.ticked() > 0)
            .map(|f| FileChange {
                path: self.root.join(&f.path),
                before: f.text.clone(),
                after: Arc::new(f.after(None)),
                count: f.ticked(),
            })
            .collect()
    }

    pub fn replace_selected(&mut self, cx: &mut Context<Self>) {
        let changes = self.selected_changes();
        if !changes.is_empty() {
            cx.emit(FindEvent::Replace(changes));
        }
    }

    pub fn replace_all(&mut self, cx: &mut Context<Self>) {
        let changes = self.all_changes();
        if !changes.is_empty() {
            cx.emit(FindEvent::Replace(changes));
        }
    }

    /// A replacement was written: say so, and search again.
    pub fn replaced(&mut self, note: String, cx: &mut Context<Self>) {
        self.note = Some(note);
        self.search(cx);
    }

    fn preview_lines(&self) -> Vec<PreviewLine> {
        let Some(row) = self.rows.get(self.selected).copied() else { return Vec::new() };
        let (fi, only) = match row {
            Row::File(fi) => (fi, None),
            Row::Hit(fi, hi) => (fi, Some(hi)),
        };
        let Some(f) = self.found.files.get(fi) else { return Vec::new() };
        if !self.replace_mode {
            // Find: the lines around the match.
            let Some(h) = f.hits.get(only.unwrap_or(0)) else { return Vec::new() };
            let lines: Vec<&str> = f.text.split_inclusive('\n').collect();
            let at = h.line as usize;
            return (at.saturating_sub(CONTEXT)..(at + CONTEXT + 1).min(lines.len()))
                .map(|i| PreviewLine {
                    old: Some(i + 1),
                    new: None,
                    sign: if i == at { '>' } else { ' ' },
                    text: lines[i].trim_end_matches(['\n', '\r']).replace('\t', "    "),
                })
                .collect();
        }
        // A match left out still shows what it would do.
        let mut f = f.clone();
        if let Some(hi) = only {
            f.included[hi] = true;
        }
        preview(&f.text, &f.after(only), 80)
    }
}

/// A toggle of the query: case, whole words, regular expression.
fn query_toggle(
    id: &'static str,
    icon: IconName,
    tip: &'static str,
    on: bool,
    view: Entity<FindInFiles>,
    which: &'static str,
) -> IconButton {
    IconButton::new(id, icon, tip)
        .size(26.)
        .selected(on)
        .on_click(move |_, _, cx| view.update(cx, |v, cx| v.toggle(which, cx)))
}

/// A match row's text: the match lit, or (replacing) struck through with its replacement after it.
fn hit_text(line: &str, m: Range<usize>, replacement: Option<&str>, p: &Palette) -> StyledText {
    match replacement {
        None => StyledText::new(line.to_owned()).with_highlights([(
            m,
            HighlightStyle { background_color: Some(p.wash(p.accent)), color: Some(p.fg), ..Default::default() },
        )]),
        Some(new) => {
            let new = new.replace('\n', "⏎");
            let mut text = String::with_capacity(line.len() + new.len());
            text.push_str(&line[..m.end]);
            text.push_str(&new);
            text.push_str(&line[m.end..]);
            let added = m.end..m.end + new.len();
            StyledText::new(text).with_highlights([
                (
                    m,
                    HighlightStyle {
                        background_color: Some(p.diff_del()),
                        color: Some(p.removed),
                        strikethrough: Some(StrikethroughStyle { thickness: px(1.), color: Some(p.removed) }),
                        ..Default::default()
                    },
                ),
                (
                    added,
                    HighlightStyle { background_color: Some(p.diff_add()), color: Some(p.fg), ..Default::default() },
                ),
            ])
        }
    }
}

impl Render for FindInFiles {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !std::mem::replace(&mut self.focused, true) {
            self.focus_input(window, cx);
        }
        let p = palette(cx);
        let view = cx.entity();
        let replacing = self.replace_mode;
        let matches = self.match_count();
        let files = self.found.files.len();
        let ticked = self.ticked_count();

        // The query: the field and its toggles, then the replacement.
        let toggles = div()
            .flex()
            .gap(px(2.))
            .child(query_toggle("fif-case", IconName::CaseSensitive, "Match Case", self.case, view.clone(), "case"))
            .child(query_toggle("fif-word", IconName::WholeWord, "Whole Words", self.word, view.clone(), "word"))
            .child(query_toggle(
                "fif-regex",
                IconName::Regex,
                "Regular Expression ($1 in the replacement names a group)",
                self.regex,
                view.clone(),
                "regex",
            ));
        let find_row = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().min_w_0().child(look::field(&self.find, true, None, &p)))
            .child(toggles)
            .child(
                IconButton::new(
                    "fif-replace-mode",
                    IconName::Replace,
                    if replacing {
                        "Hide Replace".to_owned()
                    } else {
                        crate::keymap::shown("Replace in Files (⇧⌘R)")
                    },
                )
                .size(26.)
                .selected(replacing)
                .on_click({
                    let view = view.clone();
                    move |_, w, cx| view.update(cx, |v, cx| v.set_replace_mode(!v.replace_mode, w, cx))
                }),
            );
        let replace_row = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().min_w_0().child(look::field(&self.replace, false, None, &p)))
            .child(div().w(px(110.)));

        // Where to look: the scope, and the file mask.
        let module_name = self.module.and_then(|i| self.modules.get(i)).map(|(n, _)| n.clone());
        let mut scope_row = div().flex().items_center().gap(px(4.));
        for s in Scope::ALL {
            let view = view.clone();
            let label: SharedString = match (s, &module_name) {
                (Scope::Module, Some(name)) if self.scope == Scope::Module => format!("Module: {name}").into(),
                _ => s.label().into(),
            };
            scope_row = scope_row.child(
                look::toggle(
                    SharedString::from(format!("fif-scope-{}", s.label())),
                    match s {
                        Scope::Project => IconName::FolderTree,
                        Scope::Module => IconName::Package,
                        Scope::Directory => IconName::Folder,
                        Scope::OpenFiles => IconName::Files,
                    },
                    label,
                    self.scope == s,
                    &p,
                )
                .debug_selector(move || format!("fif-scope-{}", s.label()))
                .text_size(px(12.))
                .on_click(move |_, _, cx| view.update(cx, |v, cx| v.set_scope(s, cx))),
            );
        }
        if self.scope == Scope::Module && self.modules.len() > 1 {
            let modules = self.modules.clone();
            let current = self.module;
            let view = view.clone();
            scope_row = scope_row.child(
                gpui_component::button::Button::new("fif-module")
                    .outline()
                    .icon(crate::components::menu_icon(crate::components::IconName::Package))
                    .label("Choose…")
                    .dropdown_menu(move |mut menu, _, _| {
                        for (i, (name, _)) in modules.iter().enumerate() {
                            let view = view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(name.clone())
                                    .checked(Some(i) == current)
                                    .on_click(move |_, _, cx| view.update(cx, |v, cx| v.set_module(i, cx))),
                            );
                        }
                        menu
                    }),
            );
        }
        let where_row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(scope_row)
            .when(self.scope == Scope::Directory, |d| {
                d.child(div().w(px(200.)).child(look::field(&self.directory, false, None, &p)))
            })
            .child(div().flex_1())
            .child(div().w(px(230.)).child(look::field(&self.mask, false, None, &p)));

        // The tree of matches.
        let rows = self.rows.clone();
        let found = self.found.clone();
        let selected = self.selected;
        let list_view = view.clone();
        let list = uniform_list("fif-rows", rows.len(), move |range, _, _| {
            range
                .map(|i| {
                    let row = rows[i];
                    let state = RowState::new(i == selected, true);
                    let view = list_view.clone();
                    let select = {
                        let view = view.clone();
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                            view.update(cx, |v, cx| {
                                v.selected = i;
                                cx.notify();
                            })
                        }
                    };
                    match row {
                        Row::File(fi) => {
                            let f = &found.files[fi];
                            let (dir, name) = match f.path.rsplit_once('/') {
                                Some((d, n)) => (d.to_owned(), n.to_owned()),
                                None => (String::new(), f.path.clone()),
                            };
                            let all = f.ticked() == f.hits.len();
                            let some = f.ticked() > 0;
                            let toggle_view = view.clone();
                            let fold_view = view.clone();
                            components::tree_row(("fif-file", i), state, &p)
                                .debug_selector(move || format!("fif-file-{fi}"))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .id(("fif-fold", i))
                                        .flex_none()
                                        .child(look::chevron(f.expanded, p.fg_dim))
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            fold_view.update(cx, |v, cx| v.toggle_expanded(fi, cx))
                                        }),
                                )
                                .when(replacing, |d| {
                                    d.child(tick(("fif-file-tick", i), all, some && !all, &p).on_click(
                                        move |_, _, cx| {
                                            cx.stop_propagation();
                                            toggle_view.update(cx, |v, cx| v.toggle_file(fi, cx))
                                        },
                                    ))
                                })
                                .child(components::file_label(name, &p).flex_none())
                                .child(components::path_text(("fif-dir", i), dir, p.fg_dim))
                                .child(components::badge(
                                    if replacing && !all {
                                        format!("{} of {}", f.ticked(), f.hits.len())
                                    } else {
                                        f.hits.len().to_string()
                                    },
                                    Tone::Neutral,
                                    &p,
                                ))
                                .on_click(select)
                                .into_any_element()
                        }
                        Row::Hit(fi, hi) => {
                            let f = &found.files[fi];
                            let h = &f.hits[hi];
                            let on = f.included[hi];
                            let (line, m) = snippet(&f.text, h);
                            let text = hit_text(&line, m, (replacing && on).then_some(h.replacement.as_str()), &p);
                            let toggle_view = view.clone();
                            let open_view = view.clone();
                            components::tree_row(("fif-hit", i), state, &p)
                                .debug_selector(move || format!("fif-hit-{fi}-{hi}"))
                                .pl(px(34.))
                                .cursor_pointer()
                                .when(replacing, |d| {
                                    d.child(tick(("fif-hit-tick", i), on, false, &p).on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        toggle_view.update(cx, |v, cx| v.toggle_hit(fi, hi, cx))
                                    }))
                                })
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(40.))
                                        .text_color(p.fg_dim)
                                        .text_size(px(12.))
                                        .child(format!("{}", h.line + 1)),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .font_family(look::MONO)
                                        .text_size(px(12.5))
                                        .when(replacing && !on, |d| d.text_color(p.fg_dim))
                                        .child(text),
                                )
                                .on_click(move |e, w, cx| {
                                    select(e, w, cx);
                                    if e.click_count() > 1 {
                                        open_view.update(cx, |v, cx| v.open_selected(cx));
                                    }
                                })
                                .into_any_element()
                        }
                    }
                })
                .collect()
        })
        .flex_1()
        .px(px(4.));

        // The preview of the selected row.
        let lines = self.preview_lines();
        let preview_el = div()
            .id("fif-preview")
            .debug_selector(|| "fif-preview".into())
            .flex_none()
            .h(px(150.))
            .mx(px(12.))
            .py(px(6.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.panel)
            .overflow_y_scroll()
            .font_family(look::MONO)
            .text_size(px(12.))
            .children(lines.into_iter().map(|l| {
                let (bg, sign_ink) = match l.sign {
                    '-' => (Some(p.diff_del()), p.removed),
                    '+' => (Some(p.diff_add()), p.added),
                    '>' => (Some(p.wash(p.accent)), p.accent),
                    _ => (None, p.fg_dim),
                };
                let num = |n: Option<usize>| n.map(|n| n.to_string()).unwrap_or_default();
                div()
                    .flex()
                    .px(px(8.))
                    .h(px(18.))
                    .items_center()
                    .when_some(bg, |d, bg| d.bg(bg))
                    .child(div().flex_none().w(px(36.)).text_color(p.fg_dim).child(num(l.old)))
                    .when(replacing, |d| d.child(div().flex_none().w(px(36.)).text_color(p.fg_dim).child(num(l.new))))
                    .child(div().flex_none().w(px(14.)).text_color(sign_ink).child(match l.sign {
                        '-' | '+' => l.sign.to_string(),
                        '…' => "⋯".to_owned(),
                        _ => String::new(),
                    }))
                    .child(div().whitespace_nowrap().child(l.text))
            }));

        let blank = self.find.read(cx).value().is_empty();
        let body: AnyElement = if let Some(e) = &self.error {
            div().flex_1().p(px(12.)).child(components::callout(Tone::Warning, e.clone(), &p)).into_any_element()
        } else if self.rows.is_empty() && blank {
            div()
                .flex_1()
                .child(
                    components::EmptyState::new(snout_core::art::spots::find_files(), "Find in the project's files")
                        .hint("Type a word, a phrase or a regular expression above.")
                        .on(p.card)
                        .compact(),
                )
                .into_any_element()
        } else if self.rows.is_empty() && !self.searching && self.listed {
            div()
                .flex_1()
                .child(
                    components::EmptyState::new(snout_core::art::spots::no_results(), "Nothing found")
                        .hint("Try fewer letters, another scope, or a wider file mask.")
                        .on(p.card)
                        .compact(),
                )
                .into_any_element()
        } else {
            div().flex_1().min_h_0().flex().flex_col().gap(px(8.)).child(list).child(preview_el).into_any_element()
        };

        let status = if self.searching || !self.listed {
            "Searching…".to_owned()
        } else if matches == 0 {
            self.note.clone().unwrap_or_default()
        } else {
            let mut s = format!(
                "{matches} {} in {files} {}",
                if matches == 1 { "match" } else { "matches" },
                if files == 1 { "file" } else { "files" }
            );
            if self.found.truncated {
                s.push_str(" (the first ones only)");
            }
            if replacing && ticked < matches {
                s.push_str(&format!(", {ticked} to replace"));
            }
            s
        };
        let buttons: Vec<AnyElement> = if replacing {
            vec![
                Button::new("fif-cancel", crate::components::CANCEL_ICON)
                    .label("Cancel")
                    .kbd("Esc")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(FindEvent::Close)))
                    .into_any_element(),
                Button::new("fif-replace-selected", IconName::Replace)
                    .label("Replace Selected")
                    .disabled(self.selected_changes().is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.replace_selected(cx)))
                    .into_any_element(),
                Button::new("fif-replace-all", IconName::ReplaceAll)
                    .label("Replace All")
                    .kbd(crate::keymap::shown("⌘⏎"))
                    .primary()
                    .disabled(ticked == 0)
                    .on_click(cx.listener(|this, _, _, cx| this.replace_all(cx)))
                    .into_any_element(),
            ]
        } else {
            vec![
                Button::new("fif-close", crate::components::CLOSE_ICON)
                    .label("Close")
                    .kbd("Esc")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(FindEvent::Close)))
                    .into_any_element(),
                Button::new("fif-open", IconName::FileInput)
                    .label("Open")
                    .kbd("⏎")
                    .primary()
                    .disabled(self.rows.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.open_selected(cx)))
                    .into_any_element(),
            ]
        };

        div()
            .id("find-in-files")
            .key_context("SearchBox")
            .debug_selector(|| "find-in-files".into())
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SearchUp, _, cx| this.step(-1, cx)))
            .on_action(cx.listener(|this, _: &SearchDown, _, cx| this.step(1, cx)))
            .on_action(cx.listener(|_, _: &SearchClose, _, cx| cx.emit(FindEvent::Close)))
            .occlude()
            .w(px(900.))
            .h(px(620.))
            .flex()
            .flex_col()
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .rounded(px(12.))
            .shadow_lg()
            .overflow_hidden()
            .font_family(look::ui_font(cx))
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(
                components::dialog_header_icon(
                    Tone::Info,
                    if replacing { IconName::Replace } else { IconName::FileSearch },
                    if replacing { "Replace in Files" } else { "Find in Files" },
                    None,
                    &p,
                )
                .pb(px(8.)),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(16.))
                    .pb(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(find_row)
                    .when(replacing, |d| d.child(replace_row))
                    .child(where_row),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .pb(px(10.))
                    .border_t_1()
                    .border_color(p.border)
                    .pt(px(6.))
                    .child(body),
            )
            .child(components::dialog_footer(Some((status, p.fg_dim)), buttons, &p))
    }
}

/// The tick box of a row: ticked, partly (some of a file's matches), or not.
fn tick(id: impl Into<gpui::ElementId>, on: bool, partly: bool, p: &Palette) -> gpui::Stateful<gpui::Div> {
    let (ground, ink) = if on || partly { (p.accent, p.on_accent) } else { (p.input, p.fg) };
    div()
        .id(id)
        .flex_none()
        .size(px(14.))
        .rounded(px(3.))
        .border_1()
        .border_color(if on || partly { p.accent } else { crate::theme::mix(p.fg, 0.25, p.input) })
        .bg(ground)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |d| d.child(components::icon(IconName::Check, 11., ink)))
        .when(partly && !on, |d| d.child(components::icon(IconName::Minus, 11., ink)))
}

// ----- Writing a replacement -------------------------------------------------------------------

/// Write `changes`: an open editor takes its new text as an edit (saved, unless it had
/// changes of its own), any other file is written through the host. A file whose text is no
/// longer what the search read is left alone. `undoing` says this puts a replacement back.
pub fn apply(
    ws: &mut Workspace,
    changes: Vec<FileChange>,
    undoing: bool,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let editors = ws.editors(cx);
    let mut done: Vec<FileChange> = Vec::new();
    let mut skipped: Vec<PathBuf> = Vec::new();
    let mut disk: Vec<FileChange> = Vec::new();
    for c in changes {
        match editors.iter().find(|e| e.read(cx).path == c.path && !e.read(cx).loading) {
            Some(editor) => {
                if editor.read(cx).text(cx) != *c.before {
                    skipped.push(c.path.clone());
                    continue;
                }
                let was_dirty = editor.read(cx).dirty;
                let after = c.after.clone();
                editor.update(cx, |e, cx| {
                    e.sync_from(&after, window, cx);
                    if !was_dirty {
                        e.save(cx).detach();
                    }
                });
                done.push(c);
            }
            None => disk.push(c),
        }
    }
    let host = crate::host::host(cx);
    let write = cx.background_executor().spawn(async move {
        let mut written = Vec::new();
        let mut left = Vec::new();
        for c in disk {
            match host.read_to_string(&c.path) {
                Ok(now) if now == *c.before => match host.write(&c.path, c.after.as_bytes()) {
                    Ok(()) => written.push(c),
                    Err(_) => left.push(c.path),
                },
                _ => left.push(c.path),
            }
        }
        (written, left)
    });
    cx.spawn_in(window, async move |this, cx| {
        let (written, left) = write.await;
        let _ = this.update_in(cx, |ws, _, cx| {
            done.extend(written);
            skipped.extend(left);
            let count: usize = done.iter().map(|c| c.count).sum();
            let files = done.len();
            let mut note = if undoing {
                format!("Put back {files} {}", if files == 1 { "file" } else { "files" })
            } else {
                format!(
                    "Replaced {count} {} in {files} {}",
                    if count == 1 { "match" } else { "matches" },
                    if files == 1 { "file" } else { "files" }
                )
            };
            if !skipped.is_empty() {
                note.push_str(&format!(
                    "; {} changed since and left alone",
                    if skipped.len() == 1 { "1 file".to_owned() } else { format!("{} files", skipped.len()) }
                ));
            }
            if !undoing && !done.is_empty() {
                note.push_str(". Edit ▸ Undo Replace in Files puts it back.");
            }
            ws.replace_undo = if undoing || done.is_empty() { None } else { Some(ReplaceUndo(done)) };
            ws.model.update(cx, |m, cx| {
                m.message = note.clone();
                cx.notify();
            });
            // The window asks the workspace for the open editors' texts: after this update.
            if let Some(view) = ws.find_in_files.clone() {
                cx.defer(move |cx| view.update(cx, |v, cx| v.replaced(note, cx)));
            }
            cx.notify();
        });
    })
    .detach();
}

/// Put the last replacement back.
pub fn undo(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(ReplaceUndo(changes)) = ws.replace_undo.take() else {
        ws.model.update(cx, |m, cx| {
            m.message = "No replacement to undo".into();
            cx.notify();
        });
        return;
    };
    let back = changes
        .into_iter()
        .map(|c| FileChange { path: c.path, before: c.after, after: c.before, count: c.count })
        .collect();
    apply(ws, back, true, window, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str, replace: &str) -> Query {
        Query { text: text.into(), replace: replace.into(), ..Default::default() }
    }

    #[test]
    fn plain_case_word_and_regex_matches_and_their_replacements() {
        let text = "Order order;\nORDER orders = order;\n";
        let re = q("order", "x").compile().unwrap();
        let hits = find(text, &q("order", "x"), &re, 100);
        assert_eq!(hits.len(), 5, "case left aside");
        assert_eq!((hits[2].line, hits[2].column), (1, 0));
        let cased = Query { case: true, ..q("order", "x") };
        assert_eq!(find(text, &cased, &cased.compile().unwrap(), 100).len(), 3);
        let word = Query { word: true, case: true, ..q("order", "item") };
        let hits = find(text, &word, &word.compile().unwrap(), 100);
        assert_eq!(hits.len(), 2, "orders is not the word");
        assert_eq!(replaced(text, &hits), "Order item;\nORDER orders = item;\n");
        let re = Query { regex: true, ..q(r"(\w+) (\w+) =", "$2: $1 =") };
        let hits = find(text, &re, &re.compile().unwrap(), 100);
        assert_eq!(hits[0].replacement, "orders: ORDER =");
        assert!(Query { regex: true, ..q("(", "") }.compile().unwrap_err().starts_with("Not a regular expression"));
        // Characters before the match are counted as characters.
        let hits = find("é = a;", &q("a", ""), &q("a", "").compile().unwrap(), 10);
        assert_eq!(hits[0].column, 4);
    }

    #[test]
    fn masks_and_scopes_take_the_files_asked_for() {
        let m = Mask::parse("*.java, !*Test.java").unwrap();
        assert!(m.takes("src/main/java/a/Order.java"));
        assert!(!m.takes("src/test/java/a/OrderTest.java"));
        assert!(!m.takes("build.gradle"));
        assert!(Mask::parse("").unwrap().takes("anything"));
        let m = Mask::parse("src/main/**").unwrap();
        assert!(m.takes("src/main/x/Y.java") && !m.takes("src/test/Y.java"));
        let w = Within::Under(vec!["shop".into()], vec!["shop/api".into()]);
        assert!(w.takes("shop/src/A.java") && !w.takes("shop/api/B.java") && !w.takes("shopping/C.java"));
        assert!(Within::Under(vec![String::new()], vec![]).takes("a.txt"));
    }

    #[test]
    fn a_search_reads_the_host_and_the_open_editors() {
        let dir = std::env::temp_dir().join(format!("snout-fif-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/A.java"), "class A { int total; }\n").unwrap();
        std::fs::write(dir.join("src/B.java"), "class B {}\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "total\n").unwrap();
        let host = snout_host::local();
        let files = crate::search::list_files(&*host, &dir);
        let mut open = HashMap::new();
        open.insert("src/B.java".to_owned(), Arc::new("class B { int total = 1; }\n".to_owned()));
        let mask = Mask::parse("*.java").unwrap();
        let found = search(&*host, &dir, &files, &q("total", "sum"), &Within::All, &mask, &open).unwrap();
        let paths: Vec<&str> = found.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/A.java", "src/B.java"], "the open editor's text is searched");
        assert_eq!(found.files[1].after(None), "class B { int sum = 1; }\n");
        let lines = preview(&found.files[0].text, &found.files[0].after(None), 50);
        assert_eq!(lines.iter().map(|l| l.sign).collect::<String>(), "-+");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_preview_shows_context_and_skips_between_runs() {
        let before: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let after = before.replace("line 3\n", "LINE 3\n").replace("line 15\n", "LINE 15\n");
        let lines = preview(&before, &after, 100);
        let signs: String = lines.iter().map(|l| l.sign).collect();
        assert_eq!(signs, "  -+  …  -+  ");
        assert_eq!(lines[0].old, Some(1));
        let (line, m) = snippet(
            "    let total = order.total();\n",
            &Hit { range: 8..13, line: 0, column: 8, replacement: String::new() },
        );
        assert_eq!(&line[m], "total");
        assert!(line.starts_with("let"));
    }
}
