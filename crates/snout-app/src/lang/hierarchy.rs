//! The Hierarchy tool window: a type's supertypes or subtypes (Type Hierarchy, ⌃H), or a
//! method's callers or callees (Call Hierarchy, ⌃⌥H), from the engine's `typeHierarchy` and
//! `callHierarchy` requests. The tree opens a level at a time, asking as it goes; a click
//! goes to the type, the method, or the call.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, px, uniform_list,
};
use lsp_types::{Position, SymbolKind, Uri};
use serde_json::{Value, json};

use super::providers::DocLink;
use super::{HubEvent, LangHub, symbols, text};
use crate::glyphs::{Icon, icon};
use crate::look;
use crate::theme::palette;

/// What the tree shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Supertypes,
    Subtypes,
    /// The methods that call it.
    Callers,
    /// The methods it calls.
    Callees,
}

impl Mode {
    pub fn is_type(self) -> bool {
        matches!(self, Mode::Supertypes | Mode::Subtypes)
    }

    fn method(self) -> &'static str {
        match self {
            Mode::Supertypes => "typeHierarchy/supertypes",
            Mode::Subtypes => "typeHierarchy/subtypes",
            Mode::Callers => "callHierarchy/incomingCalls",
            Mode::Callees => "callHierarchy/outgoingCalls",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Supertypes => "Supertypes",
            Mode::Subtypes => "Subtypes",
            Mode::Callers => "Callers",
            Mode::Callees => "Callees",
        }
    }
}

/// A type or a method of the tree.
#[derive(Clone, Debug)]
pub struct Node {
    /// The engine's item, to ask about it again.
    pub item: Value,
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
    /// Where a click goes: the declaration, or for a caller, its call.
    pub uri: Uri,
    pub at: Position,
    pub depth: usize,
    /// Its children (indexes into the window's nodes), once asked for.
    pub children: Option<Vec<usize>>,
    pub open: bool,
}

impl Node {
    fn of(item: Value, depth: usize, at: Option<Position>) -> Option<Self> {
        let uri: Uri = serde_json::from_value(item.get("uri")?.clone()).ok()?;
        let selection: lsp_types::Range = serde_json::from_value(item.get("selectionRange")?.clone()).ok()?;
        Some(Node {
            name: item.get("name")?.as_str()?.to_owned(),
            detail: item.get("detail").and_then(Value::as_str).unwrap_or_default().to_owned(),
            kind: serde_json::from_value(item.get("kind")?.clone()).unwrap_or(SymbolKind::CLASS),
            uri,
            at: at.unwrap_or(selection.start),
            depth,
            children: None,
            open: false,
            item,
        })
    }
}

/// The nodes of an answer to `mode`'s request about a node at `depth - 1`.
pub fn children_of(mode: Mode, v: &Value, depth: usize) -> Vec<Node> {
    let list = v.as_array().cloned().unwrap_or_default();
    list.into_iter()
        .filter_map(|entry| match mode {
            Mode::Supertypes | Mode::Subtypes => Node::of(entry, depth, None),
            // A caller: where it makes the call.
            Mode::Callers => {
                let at = entry
                    .get("fromRanges")
                    .and_then(|r| r.get(0))
                    .and_then(|r| serde_json::from_value::<lsp_types::Range>(r.clone()).ok())
                    .map(|r| r.start);
                Node::of(entry.get("from")?.clone(), depth, at)
            }
            Mode::Callees => Node::of(entry.get("to")?.clone(), depth, None),
        })
        .collect()
}

pub struct HierarchyWindow {
    hub: Entity<LangHub>,
    /// The engine the items are of.
    root: Option<PathBuf>,
    pub mode: Mode,
    /// Every node asked for so far; the first is the one the hierarchy is of.
    pub nodes: Vec<Node>,
    pub selected: Option<usize>,
    pub note: Option<String>,
    _tasks: Vec<Task<()>>,
    focus: FocusHandle,
}

impl HierarchyWindow {
    pub fn new(hub: Entity<LangHub>, cx: &mut Context<Self>) -> Self {
        Self {
            hub,
            root: None,
            mode: Mode::Subtypes,
            nodes: Vec::new(),
            selected: None,
            note: Some("Type Hierarchy (⌃H) and Call Hierarchy (⌃⌥H) show here".into()),
            _tasks: Vec::new(),
            focus: cx.focus_handle(),
        }
    }

    /// Show the hierarchy of the symbol at `offset` of `rope`, `link`'s document: its type
    /// hierarchy or its call hierarchy, by `mode`.
    pub fn start(
        &mut self,
        mode: Mode,
        link: &Rc<DocLink>,
        rope: &gpui_component::input::Rope,
        offset: usize,
        cx: &mut Context<Self>,
    ) {
        let prepare =
            if mode.is_type() { "textDocument/prepareTypeHierarchy" } else { "textDocument/prepareCallHierarchy" };
        let params = json!({ "textDocument": { "uri": link.uri.as_str() }, "position": text::to_lsp(rope, offset) });
        let call = link.ask(rope, prepare, params, cx);
        self.root = Some(link.root.clone());
        self.mode = mode;
        self.nodes.clear();
        self.selected = None;
        self.note = Some("Asking the Java engine".into());
        self._tasks = vec![cx.spawn(async move |this, cx| {
            let answer = call.await;
            let _ = this.update(cx, |this, cx| {
                let first = answer.ok().and_then(|v| v.as_array().and_then(|a| a.first().cloned()));
                match first.and_then(|item| Node::of(item, 0, None)) {
                    Some(node) => {
                        this.nodes = vec![node];
                        this.selected = Some(0);
                        this.note = None;
                        this.expand(0, cx);
                    }
                    None => {
                        this.note = Some(if mode.is_type() {
                            "Put the caret on a class or an interface for its hierarchy".into()
                        } else {
                            "Put the caret on a method for its calls".into()
                        })
                    }
                }
                cx.notify();
            });
        })];
        cx.notify();
    }

    /// Show the other way (subtypes for supertypes, callees for callers) of the same symbol.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if mode == self.mode || mode.is_type() != self.mode.is_type() || self.nodes.is_empty() {
            return;
        }
        self.mode = mode;
        let mut root = self.nodes[0].clone();
        root.children = None;
        root.open = false;
        root.at = serde_json::from_value::<lsp_types::Range>(root.item["selectionRange"].clone())
            .map(|r| r.start)
            .unwrap_or(root.at);
        self.nodes = vec![root];
        self.selected = Some(0);
        self._tasks.clear();
        self.expand(0, cx);
    }

    /// Open node `i`, asking for its children the first time.
    pub fn expand(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(node) = self.nodes.get_mut(i) else { return };
        node.open = true;
        if node.children.is_some() {
            return cx.notify();
        }
        let Some(root) = self.root.clone() else { return };
        let (mode, depth) = (self.mode, node.depth + 1);
        let call = self.hub.read(cx).request(&root, mode.method(), json!({ "item": node.item }));
        self._tasks.push(cx.spawn(async move |this, cx| {
            let answer = call.await;
            let _ = this.update(cx, |this, cx| {
                if this.mode != mode {
                    return;
                }
                let found = match answer {
                    Ok(v) => children_of(mode, &v, depth),
                    Err(e) => {
                        eprintln!("[java] {e}");
                        Vec::new()
                    }
                };
                let from = this.nodes.len();
                this.nodes.extend(found);
                let to = this.nodes.len();
                if let Some(n) = this.nodes.get_mut(i) {
                    n.children = Some((from..to).collect());
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn toggle(&mut self, i: usize, cx: &mut Context<Self>) {
        match self.nodes.get_mut(i) {
            Some(n) if n.open => {
                n.open = false;
                cx.notify();
            }
            Some(_) => self.expand(i, cx),
            None => {}
        }
    }

    /// The nodes showing, in order: each open node's children below it.
    pub fn visible(&self) -> Vec<usize> {
        fn walk(nodes: &[Node], i: usize, out: &mut Vec<usize>) {
            out.push(i);
            if nodes[i].open
                && let Some(children) = &nodes[i].children
            {
                for &c in children {
                    walk(nodes, c, out);
                }
            }
        }
        let mut out = Vec::new();
        if !self.nodes.is_empty() {
            walk(&self.nodes, 0, &mut out);
        }
        out
    }

    /// Select node `i` and go to it.
    pub fn open(&mut self, i: usize, cx: &mut Context<Self>) {
        let (Some(node), Some(root)) = (self.nodes.get(i), self.root.clone()) else { return };
        self.selected = Some(i);
        let (uri, at) = (node.uri.clone(), node.at);
        self.hub.update(cx, |_, cx| cx.emit(HubEvent::Open { root, uri, at }));
        cx.notify();
    }
}

crate::panels::panel!(HierarchyWindow, "Hierarchy", |_, _| "Hierarchy".into(), true);

impl Render for HierarchyWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mode = self.mode;
        let hover = p.button_hover;
        let switch = |m: Mode, cx: &mut Context<Self>| {
            let on = m == mode;
            div()
                .id(m.label())
                .debug_selector(move || format!("hierarchy-{}", m.label().to_lowercase()))
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .cursor_pointer()
                .when(on, |d| d.bg(p.button).font_weight(FontWeight::MEDIUM))
                .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
                .child(m.label())
                .on_click(cx.listener(move |this, _, _, cx| this.set_mode(m, cx)))
        };
        let (first, second) =
            if mode.is_type() { (Mode::Supertypes, Mode::Subtypes) } else { (Mode::Callers, Mode::Callees) };
        let title = self.nodes.first().map(|n| {
            let what = if mode.is_type() { "Type hierarchy of" } else { "Calls of" };
            format!("{what} {}", n.name)
        });
        let toolbar = div()
            .flex_none()
            .px(px(8.))
            .py(px(4.))
            .flex()
            .items_center()
            .gap(px(4.))
            .child(switch(first, cx))
            .child(switch(second, cx))
            .child(div().flex_1())
            .when_some(title, |d, t| d.child(div().text_color(p.weak()).whitespace_nowrap().child(t)));
        let rows = Rc::new(self.visible());
        let nodes = Rc::new(self.nodes.clone());
        let selected = self.selected;
        let this = cx.entity();
        let body = match &self.note {
            Some(note) => look::centered_note(note.clone(), &p).into_any_element(),
            None => uniform_list("hierarchy-rows", rows.len(), move |range, _, _| {
                range
                    .map(|r| {
                        let i = rows[r];
                        let n = &nodes[i];
                        let leaf = n.children.as_ref().is_some_and(Vec::is_empty);
                        let (this, this2) = (this.clone(), this.clone());
                        let on = selected == Some(i);
                        div()
                            .id(("hierarchy-row", r))
                            .debug_selector(move || format!("hierarchy-row-{r}"))
                            .h(px(24.))
                            .w_full()
                            .pl(px(6. + 16. * n.depth as f32))
                            .pr(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .cursor_pointer()
                            .when(on, |d| d.bg(p.selection()))
                            .when(!on, |d| d.hover(move |s| s.bg(hover)))
                            .child(
                                div()
                                    .id(("hierarchy-fold", r))
                                    .flex_none()
                                    .w(px(12.))
                                    .when(!leaf, |d| {
                                        d.child(icon(
                                            if n.open { Icon::ChevronDown } else { Icon::ChevronRight },
                                            12.,
                                            p.fg_dim,
                                        ))
                                    })
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        this2.update(cx, |w, cx| w.toggle(i, cx));
                                    }),
                            )
                            .child(symbols::badge(n.kind, &p))
                            .child(div().flex_none().whitespace_nowrap().child(n.name.clone()))
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_color(p.weak())
                                    .child(n.detail.clone()),
                            )
                            .on_click(move |e, _, cx| {
                                this.update(cx, |w, cx| {
                                    w.open(i, cx);
                                    if e.click_count() >= 2 {
                                        w.toggle(i, cx);
                                    }
                                })
                            })
                    })
                    .collect()
            })
            .flex_1()
            .into_any_element(),
        };
        div()
            .id("hierarchy")
            .debug_selector(|| "hierarchy".into())
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.ground)
            .text_size(px(look::BODY))
            .text_color(p.fg)
            .when(!self.nodes.is_empty(), |d| d.child(toolbar))
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_engines_answers() {
        let item = |name: &str| {
            json!({ "name": name, "kind": 5, "detail": "demo.shop", "uri": "file:///w/A.java",
                "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 9, "character": 1 } },
                "selectionRange": { "start": { "line": 2, "character": 13 }, "end": { "line": 2, "character": 14 } } })
        };
        let subs = children_of(Mode::Subtypes, &json!([item("Circle"), item("Square")]), 1);
        assert_eq!(subs.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["Circle", "Square"]);
        assert_eq!((subs[0].at.line, subs[0].depth, subs[0].detail.as_str()), (2, 1, "demo.shop"));
        let callers = children_of(
            Mode::Callers,
            &json!([{ "from": item("main"), "fromRanges": [{ "start": { "line": 7, "character": 8 }, "end": { "line": 7, "character": 12 } }] }]),
            1,
        );
        assert_eq!((callers[0].name.as_str(), callers[0].at.line), ("main", 7), "a caller goes to its call");
        let callees = children_of(Mode::Callees, &json!([{ "to": item("area"), "fromRanges": [] }]), 2);
        assert_eq!((callees[0].name.as_str(), callees[0].at.line), ("area", 2));
    }
}
