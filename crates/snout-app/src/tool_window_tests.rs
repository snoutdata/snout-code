//! Every tool window draws one header (its title and its toolbar), and with nothing in it,
//! an illustrated empty state under that header. The sheets resize from their edges.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
use gpui_component::dock::DockPlacement;

use crate::panels::Activity;
use crate::tests::{demo_repo, open, with_repo};
use crate::workspace::Tool;

fn selector(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

#[gpui::test]
fn each_tool_window_has_one_header_and_an_illustrated_empty_state(cx: &mut TestAppContext) {
    let repo = demo_repo("tool-windows");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    cx.run_until_parked();
    // (the tool, whether its empty state shows here: a terminal runs a shell at once, and the
    // Processes window lists this very process)
    let tools = [
        (Tool::Run, true),
        (Tool::Debug, true),
        (Tool::Build, true),
        (Tool::Output, true),
        (Tool::Terminal, false),
        (Tool::Problems, true),
        (Tool::Gradle, true),
        (Tool::Maven, true),
        (Tool::Spring, true),
        (Tool::Notifications, true),
        (Tool::ClaudeChanges, true),
        (Tool::Structure, true),
        (Tool::Hierarchy, true),
        (Tool::Processes, false),
    ];
    for (tool, empty) in tools {
        cx.update(|_, cx| model.update(cx, |m, _| m.output.clear()));
        cx.update(|w, cx| {
            ws.update(cx, |ws, cx| {
                ws.hide_dock(DockPlacement::Bottom, w, cx);
                ws.hide_dock(DockPlacement::Right, w, cx);
                ws.show_tool(tool, w, cx);
            })
        });
        cx.run_until_parked();
        assert!(cx.update(|_, cx| ws.read(cx).tool_open(tool, cx)), "{tool:?} opens");
        crate::tabs::HEADERS.with(|h| h.borrow_mut().clear());
        cx.update(|w, _| w.refresh());
        cx.run_until_parked();
        let drawn = crate::tabs::HEADERS.with(|h| h.borrow().clone());
        let panel = panel_name(tool);
        let headers = drawn.iter().filter(|n| *n == panel).count();
        assert_eq!(headers, 1, "{tool:?} draws one header: {drawn:?}");
        let title = crate::tool_windows::title(panel);
        let header = cx
            .debug_bounds(selector(format!("tool-header-{title}")))
            .unwrap_or_else(|| panic!("{tool:?}'s header is drawn"));
        if empty {
            let art = cx.debug_bounds("empty-state-art").unwrap_or_else(|| panic!("{tool:?} shows its empty state"));
            assert!(
                art.top() > header.bottom()
                    && art.left() >= header.left() - px(12.)
                    && art.right() <= header.right() + px(12.),
                "{tool:?}'s drawing is in the window, under its header: {art:?} {header:?}"
            );
        }
    }
    // The side panel draws its header too: one.
    let drawn = crate::tabs::HEADERS.with(|h| h.borrow().clone());
    assert_eq!(drawn.iter().filter(|n| *n == "Side").count(), 1, "{drawn:?}");
    let _ = std::fs::remove_dir_all(repo);
}

/// The dock name of the panel a tool window is.
fn panel_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Run => "Run",
        Tool::Debug => "Debug",
        Tool::Build => "Build",
        Tool::Output => "Output",
        Tool::Terminal => "Terminal",
        Tool::Problems => "Problems",
        Tool::Claude => "Claude",
        Tool::Gradle => "Gradle",
        Tool::Maven => "Maven",
        Tool::Spring => "Spring",
        Tool::Notifications => "Notifications",
        Tool::ClaudeChanges => "Claude Changes",
        Tool::Structure => "Structure",
        Tool::Hierarchy => "Hierarchy",
        Tool::Processes => "Processes",
    }
}

fn drag(cx: &mut VisualTestContext, from: gpui::Point<gpui::Pixels>, to: gpui::Point<gpui::Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    // A drag starts once the pointer has moved a little.
    let mid = point(from.x + (to.x - from.x) / 2., from.y + (to.y - from.y) / 2.);
    cx.simulate_mouse_move(mid, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn the_settings_sheet_resizes_from_its_corner_and_keeps_its_size(cx: &mut TestAppContext) {
    use crate::components::sheet::SETTINGS;
    let repo = demo_repo("sheet-size");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_settings(w, cx)));
    cx.run_until_parked();
    let before = cx.debug_bounds("sheet-settings").expect("the sheet shows");
    let viewport = cx.update(|w, _| w.viewport_size());
    let want = crate::components::sheet::clamp(&SETTINGS, SETTINGS.default, viewport);
    assert_eq!((f32::from(before.size.width), f32::from(before.size.height)), want, "it opens at its default");
    // Smaller, from the bottom right corner: it stops at its least.
    let corner = cx.debug_bounds("sheet-resize-se").expect("the corner handle").center();
    drag(cx, corner, point(corner.x - px(400.), corner.y - px(400.)));
    let small = cx.debug_bounds("sheet-settings").unwrap();
    assert_eq!((f32::from(small.size.width), f32::from(small.size.height)), SETTINGS.min, "never under its least");
    assert_eq!(small.center(), before.center(), "it stays centred");
    // Wider, from the right edge alone: the height stays.
    let edge = cx.debug_bounds("sheet-resize-e").expect("the edge handle").center();
    drag(cx, edge, point(edge.x + px(60.), edge.y + px(200.)));
    let wide = cx.debug_bounds("sheet-settings").unwrap();
    assert!(wide.size.width > small.size.width + px(100.), "{wide:?}");
    assert_eq!(wide.size.height, small.size.height);
    let kept = cx.update(|_, cx| model.read(cx).settings.dialog_sizes.get("settings").copied());
    assert_eq!(kept, Some([f32::from(wide.size.width), f32::from(wide.size.height)]), "the size is kept");
    // Closed and opened again: the size it was left at.
    cx.update(|_, cx| {
        ws.update(cx, |ws, cx| {
            ws.settings_sheet = None;
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_settings(w, cx)));
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("sheet-settings").unwrap().size, wide.size);
    let _ = std::fs::remove_dir_all(repo);
}
