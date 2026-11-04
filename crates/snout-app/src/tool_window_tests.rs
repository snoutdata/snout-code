//! Every tool window draws one header (its title and its toolbar), and with nothing in it,
//! an illustrated empty state under that header. The sheets resize from their edges.

use gpui::{AppContext as _, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};
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
        (Tool::Tests, true),
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
        Tool::Tests => "Tests",
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

/// Draw the window again.
fn draw(cx: &mut VisualTestContext) {
    cx.update(|w, _| w.refresh());
    cx.run_until_parked();
}

/// A tool window moves to another dock (Move to): its stripe button goes with it, its
/// header hides that dock, the Project button stands back while it is in front of the left
/// dock, and the next launch finds it where it was left.
#[gpui::test]
fn a_tool_window_moves_between_docks_and_stays_there(cx: &mut TestAppContext) {
    use crate::workspace::Side;
    let repo = demo_repo("tool-move");
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    vcx.run_until_parked();
    let place = |vcx: &mut VisualTestContext, t: Tool| vcx.update(|_, cx| ws.read(cx).placement_of(t, cx));
    assert_eq!(place(vcx, Tool::Claude), DockPlacement::Bottom);
    // A stripe button is on the left stripe when it lines up with Project's, else on the
    // right one.
    let on_left = |vcx: &mut VisualTestContext, item: &'static str| {
        draw(vcx);
        let (project, i) = (vcx.debug_bounds("act-project").unwrap(), vcx.debug_bounds(item).unwrap());
        (i.left() - project.left()).abs() < px(1.)
    };

    // To the right: shown there, its button on the right stripe, the menu checks Right.
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.move_tool(Tool::Claude, Side::Right, w, cx)));
    vcx.run_until_parked();
    assert_eq!(place(vcx, Tool::Claude), DockPlacement::Right);
    assert!(vcx.update(|_, cx| ws.read(cx).tool_open(Tool::Claude, cx)), "shown where it went");
    assert!(!on_left(vcx, "stripe-claude"), "on the right stripe");
    assert!(vcx.debug_bounds("tool-header-Console").is_some(), "its header, with the Move to menu");
    assert!(vcx.debug_bounds("tool-options").is_some());
    let places = vcx.update(|_, cx| cx.global::<crate::workspace::ToolPlaces>().0.clone());
    assert!(places.contains(&(Tool::Claude, Side::Right)));
    // Its stripe button toggles it in its new dock; the header's Hide hides that dock.
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.toggle_tool(Tool::Claude, w, cx)));
    vcx.run_until_parked();
    assert!(!vcx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(DockPlacement::Right)));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.toggle_tool(Tool::Claude, w, cx)));
    vcx.run_until_parked();
    vcx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::HideToolWindow("Claude")), cx));
    vcx.run_until_parked();
    assert!(!vcx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(DockPlacement::Right)));

    // To the left, through the action: in front of Project, whose button stands back;
    // Project comes back to the front with its button.
    // (The keyboard was in the console, hidden with its dock: the menu's click comes from
    // the window.)
    vcx.update(|w, cx| {
        let focus = ws.read(cx).focus.clone();
        w.focus(&focus, cx);
        w.dispatch_action(Box::new(crate::menus::MoveToolWindow(Tool::Claude, Side::Left)), cx)
    });
    vcx.run_until_parked();
    assert_eq!(place(vcx, Tool::Claude), DockPlacement::Left);
    assert!(vcx.update(|_, cx| ws.read(cx).tool_open(Tool::Claude, cx)));
    assert!(on_left(vcx, "stripe-claude"));
    assert!(!vcx.update(|_, cx| ws.read(cx).side_in_front(cx)));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.set_activity(Activity::Project, w, cx)));
    vcx.run_until_parked();
    assert!(vcx.update(|_, cx| ws.read(cx).side_in_front(cx)), "Project in front again");
    assert!(vcx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(DockPlacement::Left)), "and not hidden");
    assert!(!vcx.update(|_, cx| ws.read(cx).tool_open(Tool::Claude, cx)));
    // The Terminal still lives at the bottom.
    assert_eq!(place(vcx, Tool::Terminal), DockPlacement::Bottom);

    // The next launch: still on the left.
    let saved = vcx.update(|_, cx| {
        ws.update(cx, |ws, cx| ws.save_layout(cx));
        model.read(cx).settings.clone()
    });
    let model2 = cx.update(|cx| cx.new(|cx| crate::model::Model::new(saved, None, cx)));
    let mut ws2 = None;
    let (_, vcx2) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| crate::workspace::Workspace::new(model2.clone(), window, cx));
        ws2 = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx2.run_until_parked();
    let ws2 = ws2.unwrap();
    assert_eq!(vcx2.update(|_, cx| ws2.read(cx).placement_of(Tool::Claude, cx)), DockPlacement::Left);
    // And back to the bottom.
    vcx2.update(|w, cx| ws2.update(cx, |ws, cx| ws.move_tool(Tool::Claude, Side::Bottom, w, cx)));
    vcx2.run_until_parked();
    assert_eq!(vcx2.update(|_, cx| ws2.read(cx).placement_of(Tool::Claude, cx)), DockPlacement::Bottom);
    let _ = std::fs::remove_dir_all(repo);
}

/// The console's own hide (its ×, ⌘J) hides the console where it is: moved to the right,
/// it closes the right dock and leaves the bottom one (the Terminal) as it was; ⌘J brings
/// it back; on the left, hiding it brings Project back.
#[gpui::test]
fn the_console_hides_in_its_own_dock(cx: &mut TestAppContext) {
    use crate::model::Action;
    use crate::workspace::Side;
    let repo = demo_repo("console-hide");
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_tool(Tool::Terminal, w, cx)));
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.move_tool(Tool::Claude, Side::Right, w, cx)));
    vcx.run_until_parked();
    let open_at =
        |vcx: &mut VisualTestContext, p: DockPlacement| vcx.update(|_, cx| ws.read(cx).dock.read(cx).is_dock_open(p));
    let toggle = |vcx: &mut VisualTestContext| {
        vcx.update(|_, cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
        vcx.run_until_parked();
    };
    let shows = |vcx: &mut VisualTestContext| vcx.update(|_, cx| ws.read(cx).tool_open(Tool::Claude, cx));
    assert!(open_at(vcx, DockPlacement::Bottom) && open_at(vcx, DockPlacement::Right) && shows(vcx));
    assert!(vcx.update(|_, cx| model.read(cx).settings.console_open));

    // Its × (Toggle Console): the right dock closes, the Terminal stays.
    toggle(vcx);
    assert!(!open_at(vcx, DockPlacement::Right), "the console's dock closes");
    assert!(open_at(vcx, DockPlacement::Bottom), "the bottom dock stays open");
    assert!(vcx.update(|_, cx| ws.read(cx).tool_open(Tool::Terminal, cx)));
    assert!(!vcx.update(|_, cx| model.read(cx).settings.console_open));
    toggle(vcx);
    assert!(shows(vcx) && open_at(vcx, DockPlacement::Bottom));
    assert!(vcx.update(|_, cx| model.read(cx).settings.console_open));

    // Behind another window of its dock, ⌘J brings it forward rather than hide the dock.
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_tool(Tool::Structure, w, cx)));
    vcx.run_until_parked();
    assert!(!shows(vcx));
    toggle(vcx);
    assert!(shows(vcx), "in front of its dock again");

    // On the left: hiding it brings Project back, the left dock stays.
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.move_tool(Tool::Claude, Side::Left, w, cx)));
    vcx.run_until_parked();
    assert!(shows(vcx));
    toggle(vcx);
    assert!(open_at(vcx, DockPlacement::Left) && vcx.update(|_, cx| ws.read(cx).side_in_front(cx)));
    assert!(open_at(vcx, DockPlacement::Bottom), "the bottom dock untouched");
    // Its stripe button shows it again, and its session may start.
    vcx.update(|w, cx| ws.update(cx, |ws, cx| ws.toggle_tool(Tool::Claude, w, cx)));
    vcx.run_until_parked();
    assert!(shows(vcx) && vcx.update(|_, cx| model.read(cx).settings.console_open));
    let _ = std::fs::remove_dir_all(repo);
}
