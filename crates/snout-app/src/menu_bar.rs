//! The menu bar drawn in the window's title bar, where the system has no global one (Linux).
//! It shows the menus the app hands GPUI (`menus::bar`, the same definition macOS's menu bar
//! is made from), with the icons `menu_icons` gives the Mac's items (as Lucide icons) and
//! each command's shortcut. A click opens a menu; while one is open, moving to another
//! opens that one; Esc, a click elsewhere or a command closes it. The keyboard goes back to
//! where it was, so the command runs there (the editor's Undo in the editor).
//!
//! Modelled on GPUI Kit's `AppMenuBar`, which draws no icons.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    OwnedMenu, OwnedMenuItem, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, anchored, deferred, div, px,
};
use gpui::{AppContext as _, Focusable as _};
use gpui_component::menu::PopupMenu;

use crate::components::menu_icon;
use crate::theme::palette;

/// The in-window menu bar.
pub struct MenuBar {
    menus: Vec<OwnedMenu>,
    /// The menu open, by its place in the bar, and its popup.
    open: Option<(usize, Entity<PopupMenu>)>,
    /// What had the keyboard before a menu opened: commands run there.
    action_context: Option<FocusHandle>,
    _dismiss: Option<Subscription>,
}

impl MenuBar {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let mut bar = Self { menus: Vec::new(), open: None, action_context: None, _dismiss: None };
            bar.reload(cx);
            bar
        })
    }

    /// Take the menus as the app last handed them to GPUI.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.menus = cx.get_menus().unwrap_or_default();
        self.open = None;
        cx.notify();
    }

    /// The names of the menus, for tests.
    pub fn names(&self) -> Vec<String> {
        self.menus.iter().map(|m| m.name.to_string()).collect()
    }

    /// Which menu is open, if one is.
    pub fn open_menu(&self) -> Option<usize> {
        self.open.as_ref().map(|(i, _)| *i)
    }

    /// Open the menu at `ix` (closing another), or close it if it is the one open.
    pub fn toggle(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_menu() == Some(ix) {
            self.close(window, cx);
        } else {
            self.open(ix, window, cx);
        }
    }

    /// Open the menu at `ix`.
    pub fn open(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menus.get(ix) else { return };
        if self.open.is_none() {
            self.action_context = window.focused(cx);
        }
        let items = menu.items.clone();
        let context = self.action_context.clone();
        let popup = PopupMenu::build(window, cx, move |m, window, cx| {
            let m = match context {
                Some(handle) => m.action_context(handle),
                None => m,
            };
            with_items(m, &items, window, cx)
        });
        popup.read(cx).focus_handle(cx).focus(window, cx);
        self._dismiss = Some(cx.subscribe_in(&popup, window, |this, _, _: &DismissEvent, window, cx| {
            this.close(window, cx);
        }));
        self.open = Some((ix, popup));
        cx.notify();
    }

    /// Close the open menu; the keyboard goes back where it was.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = None;
        self._dismiss = None;
        if let Some(handle) = self.action_context.take() {
            handle.focus(window, cx);
        }
        cx.notify();
    }
}

/// `menu` with `items`: each with its icon and shortcut, a checked one with its check.
fn with_items(
    mut menu: PopupMenu,
    items: &[OwnedMenuItem],
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    for item in items {
        menu = match item {
            OwnedMenuItem::Separator => menu.separator(),
            OwnedMenuItem::Action { name, action, checked, disabled, .. } => {
                let icon = crate::menu_icons::for_action(action.name());
                match icon {
                    _ if *checked => {
                        menu.menu_with_check_and_disabled(name.clone(), true, action.boxed_clone(), *disabled)
                    }
                    Some(icon) => menu.menu_with_icon_and_disabled(
                        name.clone(),
                        menu_icon(icon.lucide),
                        action.boxed_clone(),
                        *disabled,
                    ),
                    None => menu.menu_with_disabled(name.clone(), action.boxed_clone(), *disabled),
                }
            }
            OwnedMenuItem::Submenu(sub) => {
                let icon = crate::menu_icons::for_submenu(&sub.name).map(|i| menu_icon(i.lucide));
                let sub_items = sub.items.clone();
                menu.submenu_with_icon(icon, sub.name.clone(), window, cx, move |m, window, cx| {
                    with_items(m, &sub_items, window, cx)
                })
            }
            OwnedMenuItem::SystemMenu(_) => menu,
        };
    }
    menu
}

impl Render for MenuBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let open = self.open.clone();
        let entity = cx.entity();
        let buttons: Vec<AnyElement> = self
            .menus
            .iter()
            .enumerate()
            .map(|(ix, menu)| {
                let is_open = open.as_ref().is_some_and(|(i, _)| *i == ix);
                let (hover, on) = (p.button_hover, p.button_hover);
                let toggle = entity.clone();
                let hovered = entity.clone();
                div()
                    .relative()
                    .flex_none()
                    .child(
                        div()
                            .id(("menu-bar", ix))
                            .debug_selector(move || format!("menu-bar-{ix}"))
                            .h(px(24.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .rounded(px(5.))
                            .cursor_pointer()
                            .when(is_open, |d| d.bg(on))
                            .when(!is_open, |d| d.hover(move |s| s.bg(hover)))
                            .child(menu.name.clone())
                            // Down, not up: a press in the title bar would move the window.
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                window.prevent_default();
                                cx.stop_propagation();
                                toggle.update(cx, |bar, cx| bar.toggle(ix, window, cx));
                            })
                            .on_hover(move |&over, window, cx| {
                                if over {
                                    hovered.update(cx, |bar, cx| {
                                        if bar.open.as_ref().is_some_and(|(i, _)| *i != ix) {
                                            bar.open(ix, window, cx);
                                        }
                                    });
                                }
                            }),
                    )
                    .when_some(open.as_ref().filter(|(i, _)| *i == ix).map(|(_, m)| m.clone()), |d, popup| {
                        d.child(deferred(
                            anchored()
                                .anchor(gpui::Anchor::TopLeft)
                                .snap_to_window_with_margin(px(8.))
                                .child(div().occlude().mt(px(26.)).child(popup)),
                        ))
                    })
                    .into_any_element()
            })
            .collect();
        div()
            .id("menu-bar")
            .debug_selector(|| "menu-bar".into())
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .gap(px(1.))
            .text_size(px(crate::look::BODY))
            .text_color(p.fg)
            .children(buttons)
    }
}
