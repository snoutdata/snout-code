//! The dock's look: GPUI Kit's dock skin, wrapped. Each tab group sits in a rounded pane
//! on the ground colour, as the Git view's panes do, and the center's tab groups get the
//! editor's own tab bar: a file icon, the name (in italics for the preview tab), a dot
//! while the file has unsaved changes, a close button on hover, a double click to keep a
//! preview, and a middle click to close. A tab with changes Claude Code proposes carries a
//! badge with their count; one Claude changed while the editor did not follow, a small
//! accent dot.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AnyView, App, AppContext as _, Axis, ClickEvent, Context, Div, Entity, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use gpui_base::ResizeHandleContext;
use gpui_component::dock::{
    BasePanelView, DockArea, DockAreaRenderer, DockContext, DockSkin, DragPanel, DropIndicator, NodeId, PanelHandle,
    PanelId, PanelState, TabGroupContext, TabGroupRenderer,
};

use crate::components::{ContextMenuExt as _, PopupMenu};
use crate::editor::EditorPanel;
use crate::theme::palette;

/// The dock area in the app's look, and GPUI Kit's skin inside it (for its settings).
pub fn dock_area(
    id: &'static str,
    version: Option<usize>,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<DockArea>, Rc<DockSkin>) {
    let mut skin = None;
    let area = cx.new(|cx| {
        let inner = DockSkin::new(cx);
        skin = Some(inner.clone());
        DockArea::new(id, version, window, cx).with_renderer(Rc::new(Skin { inner }))
    });
    (area, skin.expect("the skin is made with the area"))
}

struct Skin {
    inner: Rc<DockSkin>,
}

impl DockAreaRenderer for Skin {
    fn frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.frame(window, cx)
    }

    fn split_frame(&self, node: NodeId, axis: Axis, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.split_frame(node, axis, window, cx).bg(palette(cx).ground)
    }

    fn center_frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.center_frame(window, cx).bg(palette(cx).ground)
    }

    fn render_split_handle(
        &self,
        handle: &ResizeHandleContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.inner.render_split_handle(handle, window, cx)
    }

    fn render_dock(&self, dock: &DockContext, content: AnyElement, window: &mut Window, cx: &mut App) -> AnyElement {
        // A closed dock is gone: its stripe button brings it back.
        if !dock.is_open() {
            return div().into_any_element();
        }
        self.inner.render_dock(dock, content, window, cx)
    }

    fn build_placeholder(
        &self,
        state: &PanelState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<dyn BasePanelView>> {
        self.inner.build_placeholder(state, window, cx)
    }

    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(Group { inner: self.inner.tab_group_renderer() })
    }
}

struct Group {
    inner: Rc<dyn TabGroupRenderer>,
}

/// Whether the group holds the center's tabs (files and the welcome page).
fn is_editor_group(group: &TabGroupContext, cx: &App) -> bool {
    group.panels().iter().any(|p| matches!(p.panel_name(cx), crate::editor::PANEL_NAME | "Welcome"))
}

/// Whether the group is the left dock's one panel, which draws its own panes.
fn is_side(group: &TabGroupContext, cx: &App) -> bool {
    group.panels().iter().any(|p| p.panel_name(cx) == "Side")
}

impl TabGroupRenderer for Group {
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        let p = palette(cx);
        let frame = self.inner.frame(group, window, cx);
        if is_side(group, cx) {
            return frame.bg(p.ground);
        }
        // Flush, on hairline separators: the editor on its own ground, tool windows on the
        // chrome's.
        let bg = if is_editor_group(group, cx) { p.panel } else { p.ground };
        frame.bg(bg).overflow_hidden()
    }

    fn content_frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.content_frame(group, window, cx)
    }

    fn render_tab_bar(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> AnyElement {
        if is_editor_group(group, cx) {
            tab_bar(group, window, cx)
        } else if is_side(group, cx) {
            self.inner.render_tab_bar(group, window, cx)
        } else {
            tool_header(group, cx)
        }
    }

    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.inner.render_active_panel(panel, group, window, cx)
    }

    fn render_drop_indicator(&self, indicator: DropIndicator, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        self.inner.render_drop_indicator(indicator, window, cx)
    }

    fn render_empty(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        self.inner.render_empty(group, window, cx)
    }
}

/// The editor tab a dock panel is, if it is one.
pub fn editor_of(panel: &Arc<dyn BasePanelView>) -> Option<Entity<EditorPanel>> {
    panel.view().downcast::<EditorPanel>().ok()
}

/// What follows the pointer while a tab is dragged.
struct TabDrag(SharedString);

impl Render for TabDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .px(px(10.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(p.card)
            .border_1()
            .border_color(p.border)
            .text_color(p.fg)
            .opacity(0.9)
            .child(self.0.clone())
    }
}

/// Close the tab of `panel` in `group`: an editor asks first if it has unsaved changes.
fn close(group: &TabGroupContext, panel: &Arc<dyn BasePanelView>, window: &mut Window, cx: &mut App) {
    let id = panel.panel_id(cx);
    if !group.is_panel_closable(id, cx) {
        return;
    }
    let group = group.clone();
    crate::editor::confirm_close(editor_of(panel), window, cx, move |window, cx| group.close(id, window, cx));
}

/// A tab's right-click menu: closing it, the others, those to its right or all of them;
/// keeping a preview open; its file's path and place in the Finder.
fn tab_menu(
    menu: PopupMenu,
    group: &TabGroupContext,
    panel: &Arc<dyn BasePanelView>,
    ix: usize,
    path: Option<&std::path::Path>,
    preview: bool,
    cx: &App,
) -> PopupMenu {
    use crate::components::{IconName as I, menu_item};
    let panels: Vec<Arc<dyn BasePanelView>> = group.panels().to_vec();
    let id = panel.panel_id(cx);
    let others: Vec<_> = panels.iter().filter(|q| q.panel_id(cx) != id).cloned().collect();
    let right: Vec<_> = panels.iter().skip(ix + 1).cloned().collect();
    let close_all = |which: Vec<Arc<dyn BasePanelView>>| {
        let group = group.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            for q in &which {
                close(&group, q, window, cx);
            }
        }
    };
    let mut menu = crate::components::styled(menu)
        .item(menu_item("Close", I::X, close_all(vec![panel.clone()])))
        .item(menu_item("Close Others", I::ListX, close_all(others.clone())).disabled(others.is_empty()))
        .item(
            menu_item("Close Tabs to the Right", I::ArrowRightToLine, close_all(right.clone()))
                .disabled(right.is_empty()),
        )
        .item(menu_item("Close All", I::CircleX, close_all(panels)));
    if let Some(editor) = editor_of(panel).filter(|_| preview) {
        menu = menu.separator().item(menu_item("Keep Open", I::Pin, move |_, _, cx| {
            editor.update(cx, |e, cx| e.pin(cx));
        }));
    }
    if let Some(path) = path {
        let (copy, reveal, terminal) = (path.to_owned(), path.to_owned(), path.to_owned());
        menu = menu
            .separator()
            .item(menu_item("Copy Path", I::Copy, move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.display().to_string()));
            }))
            .item(menu_item("Reveal in Finder", I::FolderOpen, move |_, _, cx| cx.reveal_path(&reveal)))
            .item(menu_item("Open in External Terminal", I::SquareTerminal, move |_, window, cx| {
                window.dispatch_action(Box::new(crate::menus::OpenInTerminalAt(terminal.clone())), cx)
            }));
    }
    menu
}

/// Height of the editor tab bar.
pub const TAB_H: f32 = 32.;

fn tab_bar(group: &TabGroupContext, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = palette(cx);
    let active = group.active_panel().map(|panel| panel.panel_id(cx));
    let droppable = group.is_droppable();
    let count = group.panels().len();
    let mut tabs: Vec<AnyElement> = Vec::new();
    for (ix, panel) in group.panels().iter().enumerate() {
        if panel.visible(cx) {
            let selected = active == Some(panel.panel_id(cx));
            tabs.push(tab(group, ix, panel, selected, droppable, window, cx));
        }
    }
    let node = group.node();
    div()
        .id(("editor-tabs", node.as_u64()))
        .flex_none()
        .h(px(TAB_H))
        .w_full()
        .flex()
        .items_end()
        .bg(p.ground)
        .border_b_1()
        .border_color(p.border)
        .child(
            div()
                .id("editor-tab-strip")
                .flex()
                .items_end()
                .h_full()
                .min_w_0()
                .flex_shrink(1.)
                .overflow_x_scroll()
                .children(tabs),
        )
        .child(div().id("tab-bar-empty-space").flex_1().h_full().when(droppable, |d| {
            let group = group.clone();
            d.drag_over::<DragPanel>(move |s, _, _, _| s.bg(p.wash(p.accent))).on_drop(
                move |drag: &DragPanel, window, cx| {
                    let ix = (drag.source() == node).then(|| count.saturating_sub(1));
                    group.drop_panel(drag.clone(), ix, false, window, cx);
                },
            )
        }))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn tab(
    group: &TabGroupContext,
    ix: usize,
    panel: &Arc<dyn BasePanelView>,
    selected: bool,
    droppable: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = palette(cx);
    let editor = editor_of(panel);
    let (name, dirty, preview, path, reviews, touched) = match &editor {
        Some(e) => {
            let e = e.read(cx);
            (e.name(), e.dirty, e.preview, Some(e.path.clone()), e.review_count(), e.touched)
        }
        None => (
            PanelHandle::of(panel).and_then(|h| h.tab_name(cx)).unwrap_or_else(|| panel.panel_name(cx).into()),
            false,
            false,
            None,
            0,
            false,
        ),
    };
    let closable = group.is_panel_closable(panel.panel_id(cx), cx);
    let hover_bg = p.button_hover;
    let group_name = SharedString::from(format!("tab-{ix}"));
    let drag = group.is_draggable().then(|| group.drag_panel(ix, cx)).flatten();
    let _ = window;
    let close_button = div()
        .id(("close-tab", ix))
        .debug_selector(move || format!("close-tab-{ix}"))
        .flex_none()
        .size(px(16.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(crate::glyphs::icon(crate::glyphs::Icon::Close, 12., p.fg_dim))
        .on_click({
            let (group, panel) = (group.clone(), panel.clone());
            move |_, window, cx| {
                cx.stop_propagation();
                close(&group, &panel, window, cx);
            }
        });
    // The slot after the name: the dot while unsaved, the close button under the pointer
    // (and always on the tab in front).
    let trailing = div()
        .flex_none()
        .size(px(16.))
        .relative()
        .when(dirty, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .group_hover(group_name.clone(), |s| s.opacity(0.))
                    .child(div().size(px(7.)).rounded_full().bg(p.fg)),
            )
        })
        .when(closable, |d| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .when(dirty || !selected, |d| d.opacity(0.).group_hover(group_name.clone(), |s| s.opacity(1.)))
                    .child(close_button),
            )
        });
    let ink = if selected { p.fg } else { p.fg_dim };
    div()
        .id(("editor-tab", ix))
        .debug_selector(move || format!("editor-tab-{ix}"))
        .group(group_name.clone())
        .relative()
        // Tabs shrink (their names cut short) before the strip has to scroll.
        .flex_shrink(1.)
        .min_w(px(if selected { 88. } else { 60. }))
        .h_full()
        .pl(px(10.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(ink)
        .text_size(px(12.5))
        .cursor_pointer()
        .when(!selected, |d| d.hover(move |s| s.bg(hover_bg)))
        .when_some(path.as_ref(), |d, path| d.child(crate::icons::file_icon(path, 13., &p)))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .when(preview, |d| d.italic())
                .child(name.clone()),
        )
        .when(touched && !selected, |d| {
            d.child(
                div()
                    .debug_selector(move || format!("touched-{ix}"))
                    .flex_none()
                    .size(px(5.))
                    .rounded_full()
                    .bg(p.claude().mark),
            )
        })
        .when(reviews > 0, |d| {
            d.child(
                div()
                    .debug_selector(move || format!("review-badge-{ix}"))
                    .flex_none()
                    .min_w(px(16.))
                    .h(px(16.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .bg(p.accent)
                    .text_color(p.on_accent)
                    .text_size(px(10.5))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(reviews.to_string()),
            )
        })
        .child(trailing)
        // The tab in front is underlined in the accent colour.
        .when(selected, |d| {
            d.when(p.tinted, |d| d.bg(p.tab_selected())).child(
                div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(p.accent).shadow(p.glow_of(p.accent)),
            )
        })
        .on_click({
            let (group, editor) = (group.clone(), editor.clone());
            move |e: &ClickEvent, window, cx| {
                group.select_tab(ix, window, cx);
                if let Some(editor) = &editor
                    && editor.read(cx).touched
                {
                    editor.update(cx, |e, cx| {
                        e.touched = false;
                        cx.notify();
                    });
                }
                if e.click_count() >= 2
                    && let Some(editor) = &editor
                {
                    editor.update(cx, |e, cx| e.pin(cx));
                }
            }
        })
        .on_aux_click({
            let (group, panel) = (group.clone(), panel.clone());
            move |e: &ClickEvent, window, cx| {
                if e.is_middle_click() {
                    close(&group, &panel, window, cx);
                }
            }
        })
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .when_some(drag, |d, drag| {
            d.on_drag(drag, move |drag, offset, _, cx| {
                cx.stop_propagation();
                drag.set_drag_offset(offset);
                drag.set_preview_size(gpui::size(px(96.), px(28.)));
                cx.new(|_| TabDrag(name.clone()))
            })
        })
        .when(droppable, |d| {
            let accent = p.accent;
            let group = group.clone();
            d.drag_over::<DragPanel>(move |s, _, _, _| s.border_l_2().border_color(accent))
                .on_drop(move |drag: &DragPanel, window, cx| group.drop_panel(drag.clone(), Some(ix), true, window, cx))
        })
        .context_menu({
            let (group, panel, path) = (group.clone(), panel.clone(), path.clone());
            move |menu, _, cx| tab_menu(menu, &group, &panel, ix, path.as_deref(), preview, cx)
        })
        .into_any_element()
}

/// Tool windows that show as tabs of one header when either is in front.
const TABBED: [&str; 2] = ["Problems", "References"];

/// A small icon button of a tool window's header: quiet until the pointer is on it.
pub fn tool_button(
    id: &'static str,
    g: crate::glyphs::Icon,
    tip: &'static str,
    p: &crate::theme::Palette,
) -> Stateful<Div> {
    let (hover, pressed) = (p.button_hover, p.active);
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex_none()
        .size(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .active(move |s| s.bg(pressed))
        .tooltip(move |w, cx| gpui_component::tooltip::Tooltip::new(tip).build(w, cx))
        .child(crate::glyphs::icon(g, 14., crate::theme::mix(p.fg, 0.7, p.fg_dim)))
}

/// A tool window's header, in place of a tab bar (the stripes open tool windows): its
/// title, tabs where two windows belong together, and its toolbar.
fn tool_header(group: &TabGroupContext, cx: &mut App) -> AnyElement {
    use crate::glyphs::Icon;
    let p = palette(cx);
    let active = group.active_panel().map(|panel| panel.panel_name(cx)).unwrap_or_default();
    let right = matches!(active, "Gradle" | "Spring" | "Notifications" | "Claude Changes" | "Structure" | "Hierarchy");
    let mut row = div()
        .id(("tool-header", group.node().as_u64()))
        .debug_selector(|| "tool-header".into())
        .flex_none()
        .h(px(32.))
        .w_full()
        .pl(px(12.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(4.))
        .bg(p.ground)
        .text_size(px(crate::look::BODY))
        .text_color(p.fg);
    if TABBED.contains(&active) {
        for (ix, panel) in group.panels().iter().enumerate() {
            let name = panel.panel_name(cx);
            if !TABBED.contains(&name) {
                continue;
            }
            let on = name == active;
            let hover = p.button_hover;
            let group = group.clone();
            row = row.child(
                div()
                    .id(("tool-tab", ix))
                    .debug_selector(move || format!("tool-tab-{name}"))
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.button).font_weight(gpui::FontWeight::MEDIUM))
                    .when(!on, |d| d.text_color(p.fg_dim).hover(move |s| s.bg(hover)))
                    .child(name)
                    .on_click(move |_, window, cx| group.select_tab(ix, window, cx)),
            );
        }
    } else {
        let tool = crate::workspace::Tool::of_panel(active);
        row = row
            .when_some(tool, |d, t| d.child(div().mr(px(2.)).child(crate::glyphs::icon(t.icon(), 14., p.hue(t.hue())))))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(crate::tool_windows::title(active).to_owned()));
    }
    row = row.child(div().flex_1());
    match active {
        "Output" => {
            row = row.child(
                tool_button("tool-clear-output", Icon::Close, "Clear", &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::ClearOutput), cx)),
            )
        }
        "Gradle" => {
            row = row
                .child(
                    tool_button("tool-sync-gradle", Icon::Refresh, "Sync the Gradle Project", &p)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::SyncGradle), cx)),
                )
                .child(
                    tool_button("tool-refresh-gradle", Icon::Fetch, "Read the Tasks Again", &p)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::RefreshGradle), cx)),
                )
                .child(
                    tool_button("tool-collapse-gradle", Icon::CollapseAll, "Collapse All", &p)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::CollapseGradle), cx)),
                )
        }
        "Processes" => {
            row = row.child(
                tool_button("tool-stop-idle", Icon::Stop, "Stop Idle Processes", &p)
                    .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::StopIdleProcesses), cx)),
            )
        }
        "Spring" => {
            row = row
                .child(
                    tool_button("tool-refresh-spring", Icon::Refresh, "Read the Spring Model Again", &p)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::RefreshSpring), cx)),
                )
                .child(
                    tool_button("tool-collapse-spring", Icon::CollapseAll, "Collapse All", &p)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::menus::CollapseSpring), cx)),
                )
        }
        _ => {}
    }
    row.child(div().mx(px(2.)).w(px(1.)).h(px(16.)).bg(p.border))
        .child(
            tool_button("tool-hide", Icon::Minus, "Hide", &p)
                .on_click(move |_, w, cx| w.dispatch_action(Box::new(crate::menus::HideToolWindow(right)), cx)),
        )
        .into_any_element()
}

/// The id a panel has in the dock.
pub fn panel_id<T: 'static>(entity: &Entity<T>) -> PanelId {
    PanelId::from(entity.entity_id())
}
