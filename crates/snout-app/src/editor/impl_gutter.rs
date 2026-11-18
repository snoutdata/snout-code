//! The gutter's implementation marks, as in IntelliJ: a circle with an arrow down beside an
//! interface or abstract class something implements, and beside each of its methods
//! something implements. A click goes to the one implementation, or lists them.
//!
//! The declarations are found by tree-sitter in the editor's text; whether each has
//! implementations, the engine says (`textDocument/implementation`). Its answers are kept
//! until the file is opened or saved again, so typing only asks about what is new.

use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Context, IntoElement, Pixels, Point, Styled as _, Task, Window, canvas, point, px, size};
use gpui_component::input::Rope;

use super::EditorPanel;
use super::navigation::locations_of;
use super::run_gutter::{Lanes, visible_lines};
use crate::glyphs::Icon;
use crate::theme::palette;

/// A declaration that can have implementations: an interface or abstract class, or a method
/// one of them leaves to its implementations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overridable {
    /// Its line (from 0).
    pub line: u32,
    /// Its name, in bytes.
    pub name: Range<usize>,
    /// What names it across edits: its type's nesting (`Outer.Inner`), and for a method,
    /// its name and parameters after a `#`.
    pub key: String,
    /// The key of the type a method is declared in; empty for a type.
    pub owner: String,
}

impl Overridable {
    pub fn is_type(&self) -> bool {
        self.owner.is_empty()
    }
}

/// What a method's enclosing type leaves to others.
#[derive(Clone, Copy, PartialEq)]
enum Owner {
    /// Nothing: a class, an enum, a record, or no type.
    None,
    Interface,
    Abstract,
}

/// The declarations in the Java source `text` that can have implementations, in order.
pub fn overridables(text: &str) -> Vec<Overridable> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter::Language::new(tree_sitter_java::LANGUAGE);
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(text, None) else { return Vec::new() };
    let mut out = Vec::new();
    walk(tree.root_node(), text, "", Owner::None, &mut out);
    out
}

fn walk(node: tree_sitter::Node, text: &str, owner: &str, kind: Owner, out: &mut Vec<Overridable>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "interface_declaration" | "class_declaration" | "enum_declaration" | "record_declaration" => {
                let Some(name) = child.child_by_field_name("name") else { continue };
                let Ok(word) = name.utf8_text(text.as_bytes()) else { continue };
                let key = if owner.is_empty() { word.to_owned() } else { format!("{owner}.{word}") };
                let inner = match child.kind() {
                    "interface_declaration" => Owner::Interface,
                    "class_declaration" if has_modifier(child, "abstract") => Owner::Abstract,
                    _ => Owner::None,
                };
                if inner != Owner::None {
                    out.push(Overridable {
                        line: name.start_position().row as u32,
                        name: name.byte_range(),
                        key: key.clone(),
                        owner: String::new(),
                    });
                }
                if let Some(body) = child.child_by_field_name("body") {
                    walk(body, text, &key, inner, out);
                }
            }
            "method_declaration" => {
                let open = match kind {
                    Owner::Interface => !has_modifier(child, "static") && !has_modifier(child, "private"),
                    Owner::Abstract => has_modifier(child, "abstract"),
                    Owner::None => false,
                };
                if open
                    && let Some(name) = child.child_by_field_name("name")
                    && let Ok(word) = name.utf8_text(text.as_bytes())
                {
                    let params = child
                        .child_by_field_name("parameters")
                        .and_then(|p| p.utf8_text(text.as_bytes()).ok())
                        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
                        .unwrap_or_default();
                    out.push(Overridable {
                        line: name.start_position().row as u32,
                        name: name.byte_range(),
                        key: format!("{owner}#{word}{params}"),
                        owner: owner.to_owned(),
                    });
                }
                // Types declared in its body are no part of this one.
                walk(child, text, owner, Owner::None, out);
            }
            _ => walk(child, text, owner, kind, out),
        }
    }
}

/// Whether the declaration `node` has the modifier `word` (`abstract`, `static`, …).
fn has_modifier(node: tree_sitter::Node, word: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).filter(|c| c.kind() == "modifiers").any(|m| {
        let mut cursor = m.walk();
        m.children(&mut cursor).any(|c| c.kind() == word)
    })
}

/// How long typing pauses before the marks are looked for again.
const SETTLE: Duration = Duration::from_millis(900);
/// While the engine cannot answer yet, it is asked again this often, so many times.
const RETRY: Duration = Duration::from_millis(1500);
const TRIES: u32 = 80;
/// While the engine is busy (importing, indexing) "nothing implements it" may only mean it
/// does not know yet: such answers are asked again this often, so many times.
const BUSY_RETRY: Duration = Duration::from_secs(10);
const BUSY_TRIES: u32 = 12;
/// The most methods asked about in one file.
const MOST: usize = 300;

/// The file's implementation marks and what the engine said about them.
#[derive(Default)]
pub struct ImplMarks {
    /// The declarations something implements: the marks shown.
    pub marks: Rc<Vec<Overridable>>,
    /// Whether each declaration has implementations, by key, since the file was opened or
    /// saved.
    known: HashMap<String, bool>,
    tries: u32,
    busy_tries: u32,
    _task: Option<Task<()>>,
    _settle: Option<Task<()>>,
}

impl EditorPanel {
    /// Whether this tab has implementation marks: a Java source the engine serves.
    fn wants_impl_marks(&self) -> bool {
        self.class_file.is_none()
            && self.path.extension().is_some_and(|e| e == "java")
            && self.lang.as_ref().is_some_and(|l| !l.config)
    }

    /// Look for the file's implementation marks; `fresh` forgets what the engine said
    /// before (the file was opened or saved: implementations may have come or gone).
    pub(super) fn refresh_impls(&mut self, fresh: bool, cx: &mut Context<Self>) {
        if fresh {
            self.impls.known.clear();
            self.impls.tries = 0;
            self.impls.busy_tries = 0;
        }
        if !self.wants_impl_marks() {
            self.impls._task = None;
            if !self.impls.marks.is_empty() {
                self.impls.marks = Rc::new(Vec::new());
                cx.notify();
            }
            return;
        }
        let text = self.state.read(cx).text().to_string();
        let revision = self.revision;
        let parse = cx.background_executor().spawn(async move { overridables(&text) });
        self.impls._task = Some(cx.spawn(async move |this, cx| {
            let found = Rc::new(parse.await);
            // The types first: the methods of a type nothing implements are not asked about.
            for types in [true, false] {
                let Ok(Some(asks)) = this.update(cx, |this, cx| this.ask_impls(&found, types, revision, cx)) else {
                    return;
                };
                let mut answers = Vec::with_capacity(asks.len());
                for (key, ask) in asks {
                    answers.push((key, ask.await.map(|v| !locations_of(v).is_empty())));
                }
                let went = this.update(cx, |this, cx| {
                    if this.revision != revision {
                        return false;
                    }
                    let busy = this.engine_busy(cx);
                    let (mut failed, mut unsure) = (false, false);
                    for (key, answer) in answers {
                        match answer {
                            Ok(false) if busy => unsure = true,
                            Ok(has) => {
                                this.impls.known.insert(key, has);
                            }
                            Err(_) => failed = true,
                        }
                    }
                    // The engine is not ready for the file yet: again in a moment. Busy, it
                    // may not know of every implementation yet: again in a while.
                    let again = if failed && this.impls.tries < TRIES {
                        this.impls.tries += 1;
                        Some(RETRY)
                    } else if unsure && this.impls.busy_tries < BUSY_TRIES {
                        this.impls.busy_tries += 1;
                        Some(BUSY_RETRY)
                    } else {
                        None
                    };
                    if let Some(wait) = again {
                        this.impls._settle = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(wait).await;
                            let _ = this.update(cx, |this, cx| this.refresh_impls(false, cx));
                        }));
                    }
                    true
                });
                if !matches!(went, Ok(true)) {
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                let known = &this.impls.known;
                let has = |key: &str| known.get(key).copied().unwrap_or(false);
                let marks: Vec<Overridable> =
                    found.iter().filter(|o| has(&o.key) && (o.is_type() || has(&o.owner))).cloned().collect();
                if *this.impls.marks != marks {
                    this.impls.marks = Rc::new(marks);
                    cx.notify();
                }
            });
        }));
    }

    /// Whether the engine of this file's build is importing or indexing.
    fn engine_busy(&self, cx: &App) -> bool {
        let Some(link) = &self.lang else { return false };
        link.hub.upgrade().is_some_and(|h| h.read(cx).servers.get(&link.root).is_some_and(|s| s.busy()))
    }

    /// Ask the engine about the declarations of `found` (its types, or its methods) whose
    /// answer is not known yet. None when the text changed since it was read.
    fn ask_impls(
        &mut self,
        found: &[Overridable],
        types: bool,
        revision: u64,
        cx: &mut Context<Self>,
    ) -> Option<Vec<(String, Task<anyhow::Result<serde_json::Value>>)>> {
        if self.revision != revision {
            return None;
        }
        let link = self.lang.clone()?;
        let rope: Rope = self.state.read(cx).text().clone();
        let known = &self.impls.known;
        let wanted: Vec<&Overridable> = found
            .iter()
            .filter(|o| o.is_type() == types && !known.contains_key(&o.key))
            .filter(|o| types || known.get(&o.owner).copied().unwrap_or(false))
            .take(MOST)
            .collect();
        Some(
            wanted
                .into_iter()
                .map(|o| {
                    let params = link.at(&rope, o.name.start);
                    (o.key.clone(), link.ask(&rope, "textDocument/implementation", params, cx))
                })
                .collect(),
        )
    }

    /// The text changed: look again once typing pauses.
    pub(super) fn impls_changed(&mut self, cx: &mut Context<Self>) {
        if !self.wants_impl_marks() {
            return;
        }
        self.impls._settle = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, cx| this.refresh_impls(false, cx));
        }));
    }

    /// The implementation mark on `line`, if it has one. A line with a run icon shows that
    /// instead, so a click there runs, and its tooltip is the run icon's.
    pub fn impl_mark_on(&self, line: u32) -> Option<Overridable> {
        if self.runnables.as_ref().is_some_and(|r| r.items.iter().any(|i| i.line == line)) {
            return None;
        }
        self.impls.marks.iter().find(|o| o.line == line).cloned()
    }

    /// The implementation mark under `at`, in the run icons' lane.
    pub(super) fn impl_mark_at(&self, at: Point<Pixels>, cx: &App) -> Option<Overridable> {
        if self.impls.marks.is_empty() {
            return None;
        }
        let l = self.lanes.get();
        let (left, right) = l.run();
        if l.bounds.size.width <= px(0.) || at.x < left || at.x >= right {
            return None;
        }
        let s = self.state.read(cx);
        let line_height = s.line_height()?;
        let (line, _) = visible_lines(s).into_iter().find(|(_, top)| at.y >= *top && at.y < *top + line_height)?;
        self.impl_mark_on(line as u32)
    }

    /// A click on an implementation mark: to the implementation, or a list of them.
    pub(super) fn impl_press(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mark) = self.impl_mark_at(at, cx) else { return false };
        self.gutter_tip = None;
        self.implementations_at(mark.name.start, at, window, cx);
        true
    }

    /// The marks, in the run icons' lane (a line with a run icon keeps it).
    pub(super) fn impl_gutter(&self, cx: &App) -> impl IntoElement {
        let p = palette(cx);
        let marks = self.impls.marks.clone();
        let runnables = self.runnables.clone();
        let (state, pad) = (self.state.clone(), self.gutter_pad);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                if marks.is_empty() {
                    return;
                }
                let s = state.read(cx);
                let Some(line_height) = s.line_height() else { return };
                let visible = visible_lines(s);
                let (left, right) = Lanes::new(bounds, px(0.), pad).run();
                for m in marks.iter() {
                    if runnables.as_ref().is_some_and(|r| r.items.iter().any(|i| i.line == m.line)) {
                        continue;
                    }
                    let Some((_, top)) = visible.iter().find(|(l, _)| *l == m.line as usize) else { continue };
                    // A blue disc with a light arrow down: nothing like a problem's mark.
                    let r = px(6.);
                    let c = point((left + right) / 2., *top + line_height / 2.);
                    let disc = gpui::Bounds::new(point(c.x - r, c.y - r), size(r * 2., r * 2.));
                    window.paint_quad(gpui::fill(disc, p.hues[0]).corner_radii(r));
                    crate::glyphs::paint(Icon::Implemented, disc, p.on_badge, window);
                }
            },
        )
        .absolute()
        .inset_0()
    }
}

/// The marks of an editor, for tests.
pub fn impl_marks_of(e: &EditorPanel) -> Rc<Vec<Overridable>> {
    e.impls.marks.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Vec<(u32, String)> {
        overridables(text).into_iter().map(|o| (o.line, o.key)).collect()
    }

    #[test]
    fn interfaces_and_their_open_methods_are_found() {
        let text = "\
public interface UserService {
    UserTransfer changePassword(UserTransfer user)
        throws GenericException;
    default String name() { return \"x\"; }
    static UserService none() { return null; }
    private void helper() {}
}
";
        assert_eq!(
            keys(text),
            [
                (0, "UserService".to_owned()),
                (1, "UserService#changePassword(UserTransfer user)".to_owned()),
                (3, "UserService#name()".to_owned()),
            ]
        );
        let o = &overridables(text)[1];
        assert_eq!(&text[o.name.clone()], "changePassword");
        assert_eq!(o.owner, "UserService");
    }

    #[test]
    fn abstract_classes_mark_only_their_abstract_methods() {
        let text = "\
abstract class Shape {
    abstract double area(int scale);
    public String label() { return \"\"; }
    class Plain { void draw() {} }
    interface Visitor { void visit(Shape s); }
}
class Square extends Shape {
    double area(int scale) { return 1; }
}
";
        assert_eq!(
            keys(text),
            [
                (0, "Shape".to_owned()),
                (1, "Shape#area(int scale)".to_owned()),
                (4, "Shape.Visitor".to_owned()),
                (4, "Shape.Visitor#visit(Shape s)".to_owned()),
            ]
        );
    }
}
