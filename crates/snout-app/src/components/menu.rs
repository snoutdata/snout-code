//! The popup menu every right click opens: the editor's, the Project tree's, the Git view's
//! rows', the tabs'. Drawn by the app (not the system), so each item carries its Lucide icon,
//! its shortcut on the right (from the keymap, for an item that runs an action), separators
//! and submenus; the arrow keys move through it, Return picks, Esc closes. GPUI Kit's
//! `PopupMenu` does the drawing, in the palette's colours; the items are built here so they
//! all look alike. The macOS menu bar stays native.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Action, AnyElement, App, ClickEvent, Context, DismissEvent, Entity, FocusHandle, Focusable as _, IntoElement,
    ParentElement as _, Pixels, Point, Styled as _, Subscription, Window, anchored, deferred, div, px,
};
pub use gpui_component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};

use super::IconName;

/// An item: `label` and its icon, doing `f` when picked.
pub fn menu_item(
    label: impl Into<gpui::SharedString>,
    icon: IconName,
    f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> PopupMenuItem {
    PopupMenuItem::new(label).icon(gpui_component::Icon::new(icon)).on_click(f)
}

/// An item that runs `action`, its shortcut on the right.
pub fn action_item(label: impl Into<gpui::SharedString>, icon: IconName, action: Box<dyn Action>) -> PopupMenuItem {
    PopupMenuItem::new(label).icon(gpui_component::Icon::new(icon)).action(action)
}

/// A submenu: `label` and its icon, opening `menu` to its side.
pub fn submenu_item(label: impl Into<gpui::SharedString>, icon: IconName, menu: Entity<PopupMenu>) -> PopupMenuItem {
    PopupMenuItem::submenu(label, menu).icon(gpui_component::Icon::new(icon))
}

/// The menus' common measures: wide enough for a label and its shortcut.
pub fn styled(menu: PopupMenu) -> PopupMenu {
    menu.min_w(px(220.)).max_w(px(420.))
}

/// A popup menu a view opens from code at a point (where the editor's right click was).
/// Keep one in the view, [`MenuOverlay::show`] a menu, and add [`MenuOverlay::render`] to
/// the view's element; the menu closes itself when an item is picked, on Esc or on a click
/// elsewhere.
#[derive(Default)]
pub struct MenuOverlay {
    open: Option<(Entity<PopupMenu>, Point<Pixels>, Subscription)>,
}

impl MenuOverlay {
    /// Open `menu` at `at` (window coordinates), taking the keyboard; `slot` finds this
    /// overlay in the view, to forget the menu once it closes.
    pub fn show<V: 'static>(
        &mut self,
        menu: Entity<PopupMenu>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<V>,
        slot: fn(&mut V) -> &mut MenuOverlay,
    ) {
        let sub = cx.subscribe_in(&menu, window, move |this, _, _: &DismissEvent, _, cx| {
            slot(this).open = None;
            cx.notify();
        });
        menu.focus_handle(cx).focus(window, cx);
        self.open = Some((menu, at, sub));
        cx.notify();
    }

    /// Build a menu (its items in `build`) whose actions go to `target` (the focused editor),
    /// and open it at `at`.
    #[allow(clippy::too_many_arguments)]
    pub fn open<V: 'static>(
        &mut self,
        target: Option<FocusHandle>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<V>,
        slot: fn(&mut V) -> &mut MenuOverlay,
        build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu,
    ) {
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            let menu = styled(menu).when_some(target, |m, t| m.action_context(t));
            build(menu, window, cx)
        });
        self.show(menu, at, window, cx, slot);
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The open menu, if any, drawn over everything at its point, in the interface's type
    /// (whatever the view it opens from uses, the editor's monospace included).
    pub fn render(&self, cx: &App) -> Option<AnyElement> {
        let (menu, at, _) = self.open.as_ref()?;
        let font = crate::look::ui_font(cx);
        let size = crate::look::ui_size(cx);
        Some(
            deferred(
                anchored()
                    .position(*at)
                    .snap_to_window_with_margin(px(8.))
                    .child(div().font_family(font).text_size(px(size)).child(menu.clone())),
            )
            .with_priority(100)
            .into_any_element(),
        )
    }
}
