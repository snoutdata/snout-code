//! Session restore: what the window had open comes back with the app and with each project.

use gpui::{AppContext as _, Entity, TestAppContext, VisualTestContext};

use crate::model::{Action, Model};
use crate::panels::Activity;
use crate::settings::{Settings, Store};
use crate::tests::{act, demo_repo, open, scratch_dir, with_repo};
use crate::workspace::Workspace;

/// A second window over `settings`, as the next launch opens it.
fn reopen(cx: &mut TestAppContext, settings: Settings) -> (Entity<Model>, Entity<Workspace>, &mut VisualTestContext) {
    let model = cx.update(|cx| cx.new(|cx| Model::new(settings, None, cx)));
    let mut ws = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
        ws = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    (model, ws.unwrap(), vcx)
}

fn names(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| ws.read(cx).editors(cx).iter().map(|e| e.read(cx).name().to_string()).collect())
}

fn has_welcome(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        let ws = ws.read(cx);
        ws.dock.read(cx).panel(ws.tabs.welcome).is_some()
    })
}

/// The editor tab in front, by file name.
fn front(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| {
        let ws = ws.read(cx);
        let fronts = ws.front_panels(cx);
        ws.editors(cx)
            .into_iter()
            .find(|e| fronts.contains(&gpui_component::dock::PanelId::from(e.entity_id())))
            .map(|e| e.read(cx).name().to_string())
    })
}

#[gpui::test]
fn tabs_caret_activity_and_layout_come_back_with_the_app(cx: &mut TestAppContext) {
    use gpui_component::dock::DockPlacement;
    let repo = demo_repo("session-app");
    let long: String = (0..400).map(|i| format!("// line {i}\n")).collect();
    std::fs::write(repo.join("Long.java"), &long).unwrap();
    std::fs::write(repo.join("Short.java"), "class Short {}\n").unwrap();
    let (model, ws, vcx) = open(cx, with_repo(&repo));
    act(&model, vcx, Action::OpenFile(repo.join("Long.java"), Some((250, 4))));
    act(&model, vcx, Action::OpenFile(repo.join("Short.java"), None));
    act(&model, vcx, Action::OpenFile(repo.join("Long.java"), None));
    vcx.update(|w, cx| {
        ws.update(cx, |ws, cx| {
            ws.dock.update(cx, |d, cx| d.set_dock_size(DockPlacement::Bottom, gpui::px(222.), w, cx));
            ws.set_activity(Activity::Git, w, cx);
        })
    });
    vcx.executor().advance_clock(std::time::Duration::from_secs(1));
    vcx.run_until_parked();
    let saved = vcx.update(|_, cx| {
        ws.update(cx, |ws, cx| ws.save_layout(cx));
        model.read(cx).settings.clone()
    });

    // The next launch: the same tabs, Long.java in front with its caret where it was, the
    // Git activity, the bottom dock's size, and no Welcome page beside the files.
    let (_, ws2, vcx2) = reopen(cx, saved.clone());
    assert_eq!(names(&ws2, vcx2), ["Long.java", "Short.java"]);
    assert_eq!(front(&ws2, vcx2).as_deref(), Some("Long.java"));
    assert!(!has_welcome(&ws2, vcx2), "the Welcome page does not linger beside the files");
    vcx2.update(|_, cx| {
        let ws = ws2.read(cx);
        let long = ws.editors(cx).into_iter().find(|e| e.read(cx).name() == "Long.java").unwrap();
        let at = long.read(cx).state.read(cx).cursor_position();
        assert_eq!((at.line, at.character), (250, 4), "the caret is where it was");
        assert_eq!(ws.side.read(cx).activity, Activity::Git);
        assert_eq!(ws.dock.read(cx).dock_size(DockPlacement::Bottom), Some(gpui::px(222.)));
        assert_eq!(ws.status.read(cx).editor().map(|e| e.read(cx).name().to_string()).as_deref(), Some("Long.java"));
    });
    // The view scrolls back to where it was once the editor shows.
    vcx2.update(|w, cx| ws2.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    vcx2.run_until_parked();
    let scroll = vcx2.update(|_, cx| {
        let long = ws2.read(cx).editors(cx).into_iter().find(|e| e.read(cx).name() == "Long.java").unwrap();
        -f32::from(long.read(cx).state.read(cx).scroll_offset().y)
    });
    let files = saved.dock.as_ref().unwrap()["center"]["children"][0]["children"].clone();
    let was = files[0]["info"]["panel"]["scroll"].as_f64().unwrap() as f32;
    assert!(was > 1000. && (scroll - was).abs() < 1., "the view is where it was: {scroll}, {was}");

    // A file deleted since, and a panel this build does not have, are left out quietly.
    std::fs::remove_file(repo.join("Short.java")).unwrap();
    let mut odd = saved.clone();
    let text = odd.dock.take().unwrap().to_string().replace("\"Output\"", "\"PanelFromTheFuture\"");
    odd.dock = Some(serde_json::from_str(&text).unwrap());
    let (_, ws3, vcx3) = reopen(cx, odd);
    assert_eq!(names(&ws3, vcx3), ["Long.java"]);
    assert!(!vcx3.update(|_, cx| ws3.read(cx).dock.read(cx).dump(cx)).center.panel_name.is_empty());
    assert!(
        !serde_json::to_string(&vcx3.update(|_, cx| ws3.read(cx).dock.read(cx).dump(cx)))
            .unwrap()
            .contains("PanelFromTheFuture"),
        "no placeholder tab for the unknown panel"
    );

    // A layout of an earlier format falls back to the usual one, with its files open.
    let mut old = saved.clone();
    let mut dock = old.dock.take().unwrap();
    dock["version"] = 1.into();
    old.dock = Some(dock);
    let (_, ws4, vcx4) = reopen(cx, old);
    assert_eq!(names(&ws4, vcx4), ["Long.java"]);
    assert!(!has_welcome(&ws4, vcx4));

    // Closing the last tab brings the Welcome page back.
    vcx4.update(|w, cx| {
        ws4.update(cx, |ws, cx| {
            let e = ws.editors(cx)[0].clone();
            ws.dock.update(cx, |d, cx| d.remove_panel(e, w, cx));
        })
    });
    vcx4.run_until_parked();
    assert!(has_welcome(&ws4, vcx4), "the Welcome page fills an empty center");
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn each_project_gets_its_own_tabs_back(cx: &mut TestAppContext) {
    let a = demo_repo("session-a");
    let b = demo_repo("session-b");
    std::fs::write(a.join("A.java"), "class A {}\n").unwrap();
    std::fs::write(b.join("B.java"), "class B {}\n").unwrap();
    let (model, ws, vcx) = open(cx, with_repo(&a));
    act(&model, vcx, Action::OpenFile(a.join("A.java"), None));
    act(&model, vcx, Action::OpenFile(a.join("README.md"), None));
    assert_eq!(names(&ws, vcx), ["A.java", "README.md"]);

    // Another project: A's tabs go, and the Welcome page stands in (just once).
    act(&model, vcx, Action::OpenRepo(b.clone()));
    vcx.update(|_, cx| assert_eq!(model.read(cx).project_root().as_deref(), Some(b.as_path())));
    assert!(names(&ws, vcx).is_empty(), "the other project's tabs close");
    assert!(has_welcome(&ws, vcx));
    let welcomes = vcx.update(|_, cx| {
        serde_json::to_string(&ws.read(cx).dock.read(cx).dump(cx)).unwrap().matches("\"Welcome\"").count()
    });
    assert_eq!(welcomes, 1, "one Welcome page");
    act(&model, vcx, Action::OpenFile(b.join("B.java"), None));
    assert!(!has_welcome(&ws, vcx), "the first file takes the Welcome page's place");

    // Back to A: its tabs, README.md in front as it was; B's are kept for later.
    act(&model, vcx, Action::OpenRepo(a.clone()));
    assert_eq!(names(&ws, vcx), ["A.java", "README.md"]);
    assert_eq!(front(&ws, vcx).as_deref(), Some("README.md"));
    act(&model, vcx, Action::OpenRepo(b.clone()));
    assert_eq!(names(&ws, vcx), ["B.java"]);

    // Close Project: the Welcome page again, alone; reopening brings the tabs back.
    vcx.dispatch_action(crate::menus::CloseProject);
    vcx.run_until_parked();
    assert!(names(&ws, vcx).is_empty());
    assert!(has_welcome(&ws, vcx));
    act(&model, vcx, Action::OpenRepo(b.clone()));
    assert_eq!(names(&ws, vcx), ["B.java"]);
    assert!(!has_welcome(&ws, vcx));

    // A file of the other project opens in it, after the switch.
    act(&model, vcx, Action::OpenPath(a.join("A.java")));
    vcx.update(|_, cx| assert_eq!(model.read(cx).project_root().as_deref(), Some(a.as_path())));
    assert_eq!(front(&ws, vcx).as_deref(), Some("A.java"));
    let _ = std::fs::remove_dir_all(a);
    let _ = std::fs::remove_dir_all(b);
}

#[gpui::test]
fn breakpoints_and_run_configurations_come_back_with_the_project(cx: &mut TestAppContext) {
    use crate::run::config::{Breakpoint, ProjectSettings, RunConfig, project_file};
    let repo = demo_repo("session-runs");
    std::fs::write(repo.join("App.java"), "class App {\n  void main() {}\n}\n").unwrap();
    let home = scratch_dir("session-runs-home");
    let store = Store::in_home(&home);
    let support = store.file.parent().unwrap().to_owned();
    let mine = RunConfig { id: "user:1".into(), name: "Mine".into(), user: true, ..Default::default() };
    let saved = ProjectSettings {
        configurations: vec![mine],
        selected: Some("user:1".into()),
        breakpoints: vec![Breakpoint { path: repo.join("App.java"), line: 1, condition: None }],
    };
    saved.save(&project_file(&support, &repo)).unwrap();
    store.save(&with_repo(&repo)).unwrap();

    cx.update(|cx| crate::init(&Settings::default(), cx));
    let model = cx.update(|cx| crate::model(Some(store.clone()), None, cx));
    let mut ws = None;
    let (_, vcx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
        ws = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    vcx.run_until_parked();
    let ws = ws.unwrap();
    vcx.update(|_, cx| {
        let runs = ws.read(cx).runs.read(cx);
        assert_eq!(runs.project.breakpoints.len(), 1, "the breakpoint is back");
        assert_eq!(runs.project.selected.as_deref(), Some("user:1"), "and the chosen configuration");
        assert!(runs.project.configurations.iter().any(|c| c.name == "Mine"));
    });
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn settings_of_another_build_load_as_far_as_they_can() {
    let home = scratch_dir("session-settings");
    let store = Store::in_home(&home);
    std::fs::create_dir_all(store.file.parent().unwrap()).unwrap();
    // Fields this build does not know, values of another shape, a choice it lacks.
    let text = r#"{
        "repos": ["/w/a", 7, "/w/b"],
        "current": "/w/a",
        "theme": "Purple",
        "tab_size": "four",
        "editor_font_size": 400.0,
        "activity": "Search",
        "from_the_future": { "x": 1 },
        "soft_wrap": true
    }"#;
    std::fs::write(&store.file, text).unwrap();
    let s = store.load();
    assert_eq!(s.repos, [std::path::PathBuf::from("/w/a"), "/w/b".into()]);
    assert_eq!(s.current.as_deref(), Some(std::path::Path::new("/w/a")));
    let d = Settings::default();
    assert_eq!((s.theme, s.tab_size, s.editor_font_size, s.activity), (d.theme, d.tab_size, d.editor_font_size, None));
    assert!(s.soft_wrap, "what can be read is kept");

    // Not JSON at all: the defaults, and the file is kept aside.
    std::fs::write(&store.file, "{ not json").unwrap();
    assert_eq!(store.load(), Settings::default());
    assert!(store.file.with_extension("json.unreadable").exists());
    let _ = std::fs::remove_dir_all(home);
}
