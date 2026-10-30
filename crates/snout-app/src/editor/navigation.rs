//! Click navigation the IntelliJ way. While the navigation modifier is held (⌘, or Ctrl on
//! Linux and Windows; ⌥ when Settings ▸ Editor ▸ Navigation says so), the identifier under the
//! pointer is underlined as a link, with a pointing hand, as soon as the engine says it has a
//! definition. A click goes there; on the declaration itself, it lists its usages instead.
//! With ⌥ too (⌥⌘-click), it goes to the implementations. ⌘B and ⌥⌘B do the same at the caret.

use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, FocusHandle, InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Pixels,
    Point, StatefulInteractiveElement as _, Styled as _, Task, Window, canvas, div, point, px, size,
};
use gpui_component::input::RopeExt as _;
use lsp_types::{Location, Position, Uri};
use serde_json::json;

use super::{EditorDefaults, EditorPanel};
use crate::lang::text;
use crate::look;
use crate::settings::NavModifier;
use crate::theme::{mix, palette};

/// Whether `mods` hold the navigation modifier (and not the other one).
pub fn nav_held(mods: &Modifiers, nav: NavModifier, cx: &App) -> bool {
    match nav {
        NavModifier::Primary => mods.secondary(cx) && !mods.alt && !mods.shift,
        NavModifier::Alt => mods.alt && !mods.secondary(cx) && !mods.shift,
    }
}

/// Whether `mods` ask for the implementations: the navigation modifier and the other one
/// (⌥⌘ either way).
pub fn implementations_held(mods: &Modifiers, cx: &App) -> bool {
    mods.secondary(cx) && mods.alt && !mods.shift
}

/// A place the popup lists: the file, where, and the line's text there.
#[derive(Clone, Debug, PartialEq)]
pub struct NavItem {
    pub uri: Uri,
    pub at: Position,
    pub file: String,
    pub dir: String,
    pub preview: String,
}

/// The usages of a declaration, or the implementations of a method, to choose from.
pub struct NavPopup {
    pub title: String,
    pub at: Point<Pixels>,
    pub items: Vec<NavItem>,
    pub selected: usize,
    pub focus: FocusHandle,
}

#[derive(Default)]
pub struct NavState {
    /// The identifier underlined as a link (bytes), while the modifier is held over it.
    pub link: Option<Range<usize>>,
    /// The word last asked about, the text's revision then, and whether it has a definition.
    known: Option<(Range<usize>, u64, bool)>,
    _ask: Option<Task<()>>,
    pub popup: Option<NavPopup>,
    _popup: Option<Task<()>>,
}

/// Whether `range` (protocol positions) holds `at`.
fn contains(range: &lsp_types::Range, at: Position) -> bool {
    let key = |p: &Position| (p.line, p.character);
    key(&range.start) <= key(&at) && key(&at) <= key(&range.end)
}

/// The locations a definition or implementation answer names.
pub fn locations_of(v: serde_json::Value) -> Vec<Location> {
    if v.is_null() {
        return Vec::new();
    }
    match serde_json::from_value::<lsp_types::GotoDefinitionResponse>(v) {
        Ok(lsp_types::GotoDefinitionResponse::Scalar(l)) => vec![l],
        Ok(lsp_types::GotoDefinitionResponse::Array(ls)) => ls,
        Ok(lsp_types::GotoDefinitionResponse::Link(ls)) => {
            ls.into_iter().map(|l| Location { uri: l.target_uri, range: l.target_selection_range }).collect()
        }
        Err(_) => Vec::new(),
    }
}

/// The popup's rows for `locations`: the file's name, its folder relative to `root`, and the
/// line's text (read from `texts`, by path).
pub fn nav_items(
    locations: &[Location],
    root: Option<&std::path::Path>,
    text_of: impl Fn(&PathBuf) -> Option<String>,
) -> Vec<NavItem> {
    locations
        .iter()
        .map(|l| {
            let path = snout_lang::uri::to_path(&l.uri).unwrap_or_default();
            let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| l.uri.to_string());
            let dir = path
                .parent()
                .map(|d| root.and_then(|r| d.strip_prefix(r).ok()).unwrap_or(d).to_string_lossy().into_owned())
                .unwrap_or_default();
            let preview = text_of(&path)
                .and_then(|t| t.lines().nth(l.range.start.line as usize).map(|s| s.trim().to_owned()))
                .unwrap_or_default();
            NavItem { uri: l.uri.clone(), at: l.range.start, file, dir, preview }
        })
        .collect()
}

impl EditorPanel {
    /// The modifier held or let go, or the pointer moved: underline the identifier under it
    /// when the engine has a definition for it.
    pub(super) fn nav_hover(
        &mut self,
        at: Point<Pixels>,
        mods: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let nav = EditorDefaults::now(cx).nav_modifier;
        let held = nav_held(&mods, nav, cx) || implementations_held(&mods, cx);
        let word = held
            .then(|| {
                let s = self.state.read(cx);
                let offset = super::lang_features::offset_at(s, at)?;
                let rope = s.text();
                let w = rope.word_range(offset).filter(|w| !w.is_empty())?;
                let word = rope.slice(w.clone()).to_string();
                word.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_').then_some(w)
            })
            .flatten();
        let Some(word) = word else {
            if self.nav.link.take().is_some() {
                cx.notify();
            }
            return;
        };
        if let Some((known, rev, has)) = &self.nav.known
            && *known == word
            && *rev == self.revision
        {
            let link = has.then(|| word.clone());
            if self.nav.link != link {
                self.nav.link = link;
                cx.notify();
            }
            return;
        }
        // Ask at once: no hover delay.
        let state = self.state.clone();
        let Some(provider) = state.update(cx, |s, _| s.lsp_mut().definition_provider.clone()) else { return };
        let rope = state.read(cx).text().clone();
        let task = provider.definitions(&rope, word.start, window, cx);
        let revision = self.revision;
        self.nav.known = Some((word.clone(), revision, false));
        self.nav._ask = Some(cx.spawn(async move |this, cx| {
            let has = task.await.is_ok_and(|l| !l.is_empty());
            let _ = this.update(cx, |this, cx| {
                if this.nav.known.as_ref().is_some_and(|(w, r, _)| *w == word && *r == revision) {
                    this.nav.known = Some((word.clone(), revision, has));
                    this.nav.link = has.then_some(word);
                    cx.notify();
                }
            });
        }));
    }

    /// The link under the pointer: underlined in the accent, the pointer a hand over it.
    pub(super) fn nav_link_view(&self, cx: &App) -> Option<AnyElement> {
        let range = self.nav.link.clone()?;
        let s = self.state.read(cx);
        let b = s.range_to_bounds(&range)?;
        let p = palette(cx);
        let color = p.accent;
        Some(
            gpui::deferred(
                gpui::anchored().position(b.origin).child(
                    div()
                        .id("nav-link")
                        .debug_selector(|| "nav-link".into())
                        .w(b.size.width)
                        .h(b.size.height)
                        .cursor_pointer()
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    let line = gpui::Bounds::new(
                                        point(bounds.left(), bounds.bottom() - px(2.)),
                                        size(bounds.size.width, px(1.)),
                                    );
                                    window.paint_quad(gpui::fill(line, color));
                                },
                            )
                            .size_full(),
                        ),
                ),
            )
            .into_any_element(),
        )
    }

    /// A click with the navigation modifier at `offset`: to the definition, or from the
    /// declaration itself to its usages; with ⌥ too, to the implementations. True when the
    /// click was navigation's.
    pub(super) fn nav_click(&mut self, e: &gpui::MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if e.button != gpui::MouseButton::Left {
            return false;
        }
        let nav = EditorDefaults::now(cx).nav_modifier;
        let implementations = implementations_held(&e.modifiers, cx);
        if !implementations && !nav_held(&e.modifiers, nav, cx) {
            return false;
        }
        let state = self.state.clone();
        if !state.update(cx, |s, _| s.lsp_mut().definition_provider.is_some()) {
            return false;
        }
        let Some(at) = super::lang_features::offset_at(state.read(cx), e.position) else { return false };
        state.update(cx, |s, cx| {
            s.dismiss_lsp_overlays(cx);
            s.set_selected_range(at..at, cx);
            s.focus(window, cx);
        });
        self.nav.link = None;
        if implementations {
            self.go_to_implementations(Some(e.position), window, cx);
        } else {
            self.go_to_declaration(Some(e.position), window, cx);
        }
        true
    }

    /// Where a popup for the caret goes: under the caret.
    fn caret_point(&self, cx: &App) -> Point<Pixels> {
        let s = self.state.read(cx);
        let c = s.cursor();
        s.range_to_bounds(&(c..c)).map_or(point(px(200.), px(200.)), |b| b.bottom_left())
    }

    /// ⌘B (and ⌘-click): go to the declaration of the symbol at the caret; at the declaration
    /// itself, list its usages.
    pub fn go_to_declaration(&mut self, at: Option<Point<Pixels>>, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let Some(provider) = state.update(cx, |s, _| s.lsp_mut().definition_provider.clone()) else {
            return self.show_hint("Go to Declaration needs the Java engine for this file.", cx);
        };
        let (rope, caret) = {
            let s = state.read(cx);
            (s.text().clone(), s.cursor())
        };
        let here = text::to_kit(&rope, text::to_lsp(&rope, caret));
        let own = self.lang.as_ref().map(|l| l.uri.clone());
        let task = provider.definitions(&rope, caret, window, cx);
        let at = at.unwrap_or_else(|| self.caret_point(cx));
        self.nav._popup = Some(cx.spawn_in(window, async move |this, cx| {
            let Ok(links) = task.await else { return };
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(first) = links.first() else {
                    return this.show_hint("No declaration found here.", cx);
                };
                let at_declaration = own.as_ref().is_some_and(|u| *u == first.target_uri)
                    && contains(&first.target_selection_range, here);
                if at_declaration {
                    this.show_usages(at, window, cx);
                } else {
                    this.follow_link(first, window, cx);
                }
            });
        }));
    }

    /// Go where a definition answer points: another document through the editor's handler
    /// (it opens the file, or a library's class), else a place in this one.
    fn follow_link(&mut self, link: &lsp_types::LocationLink, window: &mut Window, cx: &mut Context<Self>) {
        let handler = self.state.update(cx, |s, _| s.lsp_mut().show_document.clone());
        let params = lsp_types::ShowDocumentParams {
            uri: link.target_uri.clone(),
            external: Some(false),
            take_focus: Some(true),
            selection: Some(link.target_selection_range),
        };
        if handler.is_some_and(|h| h(&params, window, cx)) {
            return;
        }
        // In this document the answer's positions count characters, as the editor does.
        let at = link.target_selection_range.start;
        self.go_to(at.line, at.character, window, cx);
    }

    /// The usages of the declaration at the caret, in a popup (one: straight there).
    fn show_usages(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let word = {
            let s = self.state.read(cx);
            let r = s.text().word_range(s.cursor()).unwrap_or_default();
            s.text().slice(r).to_string()
        };
        self.ask_places(
            "textDocument/references",
            json!({ "context": { "includeDeclaration": false } }),
            format!("Usages of {word}"),
            format!("No usages of {word} found."),
            at,
            window,
            cx,
        );
    }

    /// ⌥⌘B (and ⌥⌘-click): the implementations of the symbol at the caret, a class or
    /// interface's or a method's (one: straight there).
    pub fn go_to_implementations(&mut self, at: Option<Point<Pixels>>, window: &mut Window, cx: &mut Context<Self>) {
        let word = {
            let s = self.state.read(cx);
            let r = s.text().word_range(s.cursor()).unwrap_or_default();
            s.text().slice(r).to_string()
        };
        let at = at.unwrap_or_else(|| self.caret_point(cx));
        self.ask_places(
            "textDocument/implementation",
            json!({}),
            format!("Implementations of {word}"),
            format!("No implementations of {word} found."),
            at,
            window,
            cx,
        );
    }

    /// Ask the engine for places (`method` at the caret, `extra` merged into the request),
    /// then go to the one, or list them under `title`, or say `none`.
    #[allow(clippy::too_many_arguments)]
    fn ask_places(
        &mut self,
        method: &'static str,
        extra: serde_json::Value,
        title: String,
        none: String,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(link) = self.lang.clone() else {
            return self.show_hint("This needs the Java engine for this file.", cx);
        };
        let (rope, caret) = {
            let s = self.state.read(cx);
            (s.text().clone(), s.cursor())
        };
        let mut params = link.at(&rope, caret);
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        let call = link.ask(&rope, method, params, cx);
        self.nav._popup = Some(cx.spawn_in(window, async move |this, cx| {
            let found = match call.await {
                Ok(v) => locations_of(v),
                Err(e) => {
                    eprintln!("[java] {method}: {e}");
                    Vec::new()
                }
            };
            let _ = this.update_in(cx, |this, window, cx| this.show_places(found, title, none, at, window, cx));
        }));
    }

    /// Go to the one place of `found`, or list them under `title` (with each one's line), or
    /// say `none`.
    fn show_places(
        &mut self,
        found: Vec<Location>,
        title: String,
        none: String,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let host = crate::host::host(cx);
        let (root, own_path, own_text) = (self.root.clone(), self.path.clone(), self.state.read(cx).text().to_string());
        self.nav._popup = Some(cx.spawn_in(window, async move |this, cx| {
            let items = cx
                .background_executor()
                .spawn(async move {
                    nav_items(&found, root.as_deref(), |path| {
                        if *path == own_path { Some(own_text.clone()) } else { host.read_to_string(path).ok() }
                    })
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match items.len() {
                0 => this.show_hint(none, cx),
                1 => this.open_nav_item(&items[0], window, cx),
                _ => {
                    let focus = cx.focus_handle();
                    focus.focus(window, cx);
                    this.nav.popup = Some(NavPopup { title, at, items, selected: 0, focus });
                    cx.notify();
                }
            });
        }));
    }

    /// Go to a place the popup lists.
    pub fn open_nav_item(&mut self, item: &NavItem, window: &mut Window, cx: &mut Context<Self>) {
        self.close_nav_popup(window, cx);
        match &self.lang {
            Some(link) if link.uri == item.uri => self.go_to_lsp(item.at, window, cx),
            Some(link) => {
                let event =
                    crate::lang::hub::HubEvent::Open { root: link.root.clone(), uri: item.uri.clone(), at: item.at };
                link.emit(event, cx);
            }
            None => {}
        }
    }

    fn close_nav_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.nav.popup.take().is_some() {
            self.state.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
        }
    }

    /// The popup of usages or implementations: the file, its folder and the line; ↑ ↓ pick,
    /// ↩ goes, Esc closes.
    pub(super) fn nav_popup_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let popup = self.nav.popup.as_ref()?;
        let p = palette(cx);
        let items = Rc::new(popup.items.clone());
        let selected = popup.selected;
        let mono = look::mono_font(cx);
        let body = div()
            .id("nav-popup")
            .debug_selector(|| "nav-popup".into())
            .track_focus(&popup.focus)
            .occlude()
            .w(px(520.))
            .max_h(px(360.))
            .p(px(6.))
            .flex()
            .flex_col()
            .rounded(px(8.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .shadow_lg()
            .font_family(look::ui_font(cx))
            .text_size(px(12.5))
            .text_color(p.fg)
            .on_mouse_down_out(cx.listener(|this, _, w, cx| this.close_nav_popup(w, cx)))
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, w, cx| {
                let Some(popup) = this.nav.popup.as_mut() else { return };
                let n = popup.items.len();
                match e.keystroke.key.as_str() {
                    "up" => popup.selected = (popup.selected + n - 1) % n,
                    "down" => popup.selected = (popup.selected + 1) % n,
                    "enter" => {
                        let item = popup.items[popup.selected].clone();
                        this.open_nav_item(&item, w, cx);
                    }
                    "escape" => this.close_nav_popup(w, cx),
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(6.))
                    .py(px(4.))
                    .child(crate::components::icon(crate::components::IconName::Search, 13., p.accent))
                    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(popup.title.clone()))
                    .child(div().flex_1())
                    .child(div().text_size(px(11.5)).text_color(p.fg_dim).child(format!("{} found", items.len()))),
            )
            .child(div().id("nav-popup-list").flex().flex_col().overflow_y_scroll().children(
                items.iter().enumerate().map(|(i, item)| {
                    let it = item.clone();
                    let on = i == selected;
                    div()
                        .id(("nav-item", i))
                        .debug_selector(move || format!("nav-item-{i}"))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(6.))
                        .py(px(4.))
                        .rounded(px(5.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(mix(p.accent, p.sel_mix, p.card)))
                        .hover(move |s| s.bg(p.button_hover))
                        .on_click(cx.listener(move |this, _, w, cx| this.open_nav_item(&it, w, cx)))
                        .child(crate::icons::file_icon(std::path::Path::new(&item.file), 14., &p))
                        .child(div().flex_none().font_weight(gpui::FontWeight::MEDIUM).child(item.file.clone()))
                        .child(div().flex_none().text_color(p.fg_dim).child(format!(":{}", item.at.line + 1)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .font_family(mono.clone())
                                .text_size(px(11.5))
                                .text_color(p.weak())
                                .child(item.preview.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .max_w(px(140.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_size(px(11.))
                                .text_color(p.fg_dim)
                                .child(item.dir.clone()),
                        )
                }),
            ));
        Some(
            gpui::deferred(gpui::anchored().position(popup.at).snap_to_window_with_margin(px(8.)).child(body))
                .with_priority(3)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_become_places_with_their_lines() {
        let v = json!([
            { "uri": "file:///p/src/A.java", "range": { "start": { "line": 1, "character": 4 }, "end": { "line": 1, "character": 9 } } },
            { "uri": "file:///p/src/b/B.java", "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } } }
        ]);
        let locations = locations_of(v);
        assert_eq!(locations.len(), 2);
        let items = nav_items(&locations, Some(std::path::Path::new("/p")), |path| {
            Some(if path.ends_with("A.java") {
                "class A {\n    void total() {}\n}\n".into()
            } else {
                "class B {}\n".into()
            })
        });
        assert_eq!(items[0].file, "A.java");
        assert_eq!(items[0].dir, "src");
        assert_eq!(items[0].preview, "void total() {}");
        assert_eq!(items[1].dir, "src/b");
        assert!(locations_of(serde_json::Value::Null).is_empty());
        let link = json!([{ "targetUri": "file:///p/C.java", "targetRange": { "start": { "line": 2, "character": 0 }, "end": { "line": 4, "character": 1 } },
            "targetSelectionRange": { "start": { "line": 2, "character": 6 }, "end": { "line": 2, "character": 7 } } }]);
        assert_eq!(locations_of(link)[0].range.start, Position::new(2, 6));
        assert!(contains(&lsp_types::Range::new(Position::new(2, 6), Position::new(2, 11)), Position::new(2, 8)));
        assert!(!contains(&lsp_types::Range::new(Position::new(2, 6), Position::new(2, 11)), Position::new(3, 0)));
    }
}
