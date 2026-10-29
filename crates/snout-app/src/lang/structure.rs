//! The Structure tool window (⌘7): the symbols of the Java file in front as a tree (types,
//! fields, methods, each with its kind's badge and its visibility), the one the caret is in
//! picked out as the caret moves. A click goes to a symbol; a filter narrows the tree; the
//! tree is in the file's order or sorted by name.

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, ScrollStrategy, StatefulInteractiveElement as _, Styled as _, Subscription, Task, UniformListScrollHandle,
    WeakEntity, Window, div, px, uniform_list,
};
use gpui_component::input::{InputEvent, InputState};
use serde_json::json;

use super::symbols::{self, Row, Sort, Symbol};
use super::text;
use crate::editor::{EditorEvent, EditorPanel};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::palette;

/// How long typing pauses before the symbols are asked for again.
const SETTLE: Duration = Duration::from_millis(500);

pub struct StructureWindow {
    /// The editor whose file the tree shows.
    editor: Option<WeakEntity<EditorPanel>>,
    _editor_events: Option<Subscription>,
    pub symbols: Rc<Vec<Symbol>>,
    pub sort: Sort,
    /// The symbols folded, by path.
    closed: HashSet<Vec<usize>>,
    /// The symbol the caret is in.
    pub current: Option<Vec<usize>>,
    pub filter: Entity<InputState>,
    scroll: UniformListScrollHandle,
    /// The file has no symbols to show (no Java editor in front), or they are on their way.
    pub note: Option<&'static str>,
    _fetch: Option<Task<()>>,
    _settle: Option<Task<()>>,
    focus: FocusHandle,
}

impl StructureWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by name"));
        cx.subscribe(&filter, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                this.reveal_current(cx);
                cx.notify();
            }
        })
        .detach();
        Self {
            editor: None,
            _editor_events: None,
            symbols: Rc::new(Vec::new()),
            sort: Sort::Position,
            closed: HashSet::new(),
            current: None,
            filter,
            scroll: UniformListScrollHandle::new(),
            note: Some(NO_FILE),
            _fetch: None,
            _settle: None,
            focus: cx.focus_handle(),
        }
    }

    /// Show the structure of `editor`'s file (or nothing, with none in front).
    pub fn follow(&mut self, editor: Option<&Entity<EditorPanel>>, cx: &mut Context<Self>) {
        let same = match (editor, &self.editor) {
            (Some(e), Some(w)) => w.entity_id() == e.entity_id(),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.editor = editor.map(|e| e.downgrade());
        self._editor_events = editor.map(|e| {
            cx.subscribe(e, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::Changed | EditorEvent::Reloaded(_) => this.fetch_soon(cx),
                EditorEvent::CaretMoved => this.follow_caret(cx),
                _ => {}
            })
        });
        self.symbols = Rc::new(Vec::new());
        self.closed.clear();
        self.current = None;
        self.fetch(cx);
    }

    fn editor(&self) -> Option<Entity<EditorPanel>> {
        self.editor.as_ref().and_then(WeakEntity::upgrade)
    }

    fn fetch_soon(&mut self, cx: &mut Context<Self>) {
        self._settle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.fetch(cx));
        }));
    }

    /// Ask the engine for the file's symbols.
    pub fn fetch(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor() else {
            self.note = Some(NO_FILE);
            return cx.notify();
        };
        let e = editor.read(cx);
        if e.loading {
            // Its file is being read: the language server gets it after.
            return self.fetch_soon(cx);
        }
        let Some(link) = e.lang.clone().filter(|l| !l.config) else {
            self.note = Some(NO_FILE);
            self.symbols = Rc::new(Vec::new());
            return cx.notify();
        };
        let rope = e.state.read(cx).text().clone();
        if self.symbols.is_empty() {
            self.note = Some(LOADING);
        }
        let call =
            link.ask(&rope, "textDocument/documentSymbol", json!({ "textDocument": { "uri": link.uri.as_str() } }), cx);
        let id = editor.entity_id();
        self._fetch = Some(cx.spawn(async move |this, cx| {
            let answer = call.await;
            let _ = this.update(cx, |this, cx| {
                if this.editor.as_ref().map(|e| e.entity_id()) != Some(id) {
                    return;
                }
                match answer {
                    Ok(v) => {
                        this.symbols = Rc::new(symbols::parse(&v, &rope.to_string()));
                        this.note = None;
                        this.follow_caret(cx);
                    }
                    // Not ready yet: again in a moment.
                    Err(_) => this.fetch_soon(cx),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Pick out the symbol the caret is in.
    fn follow_caret(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor() else { return };
        let s = editor.read(cx).state.read(cx);
        let at = text::to_lsp(s.text(), s.cursor());
        let current = symbols::path_at(&self.symbols, at);
        if current != self.current {
            self.current = current;
            self.reveal_current(cx);
            cx.notify();
        }
    }

    /// The rows as they show now.
    pub fn rows(&self, cx: &App) -> Vec<Row> {
        let query = self.filter.read(cx).value().to_string();
        symbols::rows(&self.symbols, &query, self.sort, &|p| self.closed.contains(p))
    }

    /// Scroll the current symbol's row into view (opening what holds it).
    fn reveal_current(&mut self, cx: &mut Context<Self>) {
        let Some(current) = self.current.clone() else { return };
        for n in 1..current.len() {
            self.closed.remove(&current[..n]);
        }
        if let Some(i) = self.rows(cx).iter().position(|r| r.path == current) {
            self.scroll.scroll_to_item(i, ScrollStrategy::Center);
        }
    }

    pub fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        self.sort = sort;
        self.reveal_current(cx);
        cx.notify();
    }

    fn toggle(&mut self, path: Vec<usize>, cx: &mut Context<Self>) {
        if !self.closed.remove(&path) {
            self.closed.insert(path);
        }
        cx.notify();
    }

    /// Go to the symbol of `row` in the editor.
    pub fn open(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.editor() else { return };
        self.current = Some(row.path.clone());
        let at = row.symbol.selection.start;
        editor.update(cx, |e, cx| e.go_to_lsp(at, window, cx));
        cx.notify();
    }
}

const NO_FILE: &str = "Open a Java file to see its structure.";
const LOADING: &str = "Reading the file's structure";

crate::panels::panel!(StructureWindow, "Structure", |_, _| "Structure".into(), true);

/// A symbol's row: `lead` (a fold arrow), its badge, its visibility, its name and what the
/// engine says of it.
pub fn symbol_row(row: &Row, selected: bool, lead: gpui::AnyElement, p: &crate::theme::Palette) -> gpui::Div {
    let s = &row.symbol;
    let hover = p.button_hover;
    let detail = s.detail.trim().to_owned();
    div()
        .h(px(24.))
        .w_full()
        .pl(px(6. + 16. * row.depth as f32))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(5.))
        .cursor_pointer()
        .when(selected, |d| d.bg(p.selection()))
        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
        .child(lead)
        .child(symbols::badge(s.kind, p))
        .child(symbols::visibility_mark(s.visibility, p))
        .child(div().flex_none().whitespace_nowrap().child(s.name.clone()))
        .when(!detail.is_empty(), |d| {
            d.child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_color(p.weak()).child(detail))
        })
}

impl Render for StructureWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let rows = Rc::new(self.rows(cx));
        let count = rows.len();
        let current = self.current.clone();
        let this = cx.entity();
        let alphabetical = self.sort == Sort::Name;
        let this2 = this.clone();
        let sort = crate::components::IconButton::new(
            "structure-sort",
            Icon::Sort,
            if alphabetical { "Sorted by Name (click for File Order)" } else { "Sort by Name" },
        )
        .selected(alphabetical)
        .on_click(move |_, _, cx| {
            this2.update(cx, |this, cx| this.set_sort(if alphabetical { Sort::Position } else { Sort::Name }, cx))
        });
        let toolbar = div()
            .flex_none()
            .px(px(8.))
            .py(px(6.))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(div().flex_1().child(look::field(&self.filter, true, None, &p)))
            .child(sort);
        let empty = |title: &'static str, hint: &'static str| {
            crate::components::EmptyState::new(snout_core::art::spots::no_results(), title)
                .hint(hint)
                .on(p.panel)
                .compact()
                .into_any_element()
        };
        let body = match (self.note, count) {
            (Some(NO_FILE), _) => empty("No structure", NO_FILE),
            (Some(note), _) => look::centered_note(note, &p).into_any_element(),
            (None, 0) if !self.filter.read(cx).value().is_empty() => {
                empty("No symbol matches", "Try fewer letters of the name.")
            }
            (None, 0) => empty("Nothing declared", "The file declares no class, field or method."),
            (None, _) => uniform_list("structure-rows", count, move |range, _, _| {
                range
                    .map(|i| {
                        let row = rows[i].clone();
                        let selected = current.as_ref() == Some(&row.path);
                        let (this, this2) = (this.clone(), this.clone());
                        let path = row.path.clone();
                        let chevron = div().flex_none().w(px(12.)).when(row.has_children, |d| {
                            d.child(
                                div()
                                    .id(("structure-fold", i))
                                    .child(icon(
                                        if row.open { Icon::ChevronDown } else { Icon::ChevronRight },
                                        12.,
                                        p.fg_dim,
                                    ))
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        this2.update(cx, |w, cx| w.toggle(path.clone(), cx));
                                    }),
                            )
                        });
                        div()
                            .id(("structure-row", i))
                            .debug_selector(move || format!("structure-row-{i}"))
                            .child(symbol_row(&row, selected, chevron.into_any_element(), &p))
                            .on_click(move |_, window, cx| this.update(cx, |w, cx| w.open(&row, window, cx)))
                    })
                    .collect()
            })
            .track_scroll(&self.scroll)
            .flex_1()
            .into_any_element(),
        };
        div()
            .id("structure")
            .debug_selector(|| "structure".into())
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .child(toolbar)
            .child(body)
    }
}
