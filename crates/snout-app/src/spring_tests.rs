//! Spring in the window, headless: the gutter's marks beside the run icons, breakpoints
//! and problem dots, their tooltips and menus, the Spring tool window's tabs, and the
//! configuration files sent to the engine. The engine's answers come from
//! `tests/fixtures/spring-app.json` (recorded from the engine's `spring-app` fixture,
//! whose sources the tests copy). With the engine distribution and a JDK 17 (as
//! `lang_tests` needs), the last test asks the real engine.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};

use crate::editor::{EditorPanel, GutterMenu};
use crate::model::Action;
use crate::panels::Activity;
use crate::spring::window::{Column, Tab};
use crate::spring::{self, FileSpring};
use crate::tests::{act, git, open, scratch_dir, with_repo};
use crate::workspace::{Tool, Workspace};

/// The engine's Spring fixture as a repository.
fn fixture(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/lsp-driver/src/test/fixtures/spring-app");
    let ok = std::process::Command::new("cp").arg("-R").arg(&from).arg(&dir).status().unwrap().success();
    assert!(ok, "copy the fixture");
    let repo = dir.join("spring-app");
    std::fs::write(repo.join("gradle.properties"), "org.gradle.daemon.idletimeout=60000\n").unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.name=t", "-c", "user.email=t@example.com", "add", "-A"]);
    git(
        &repo,
        &["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", "commit", "-qm", "f"],
    );
    repo
}

fn java(repo: &Path, rel: &str) -> PathBuf {
    repo.join("src/main/java/demo").join(rel)
}

fn editor_of(ws: &Entity<Workspace>, path: &Path, cx: &mut VisualTestContext) -> Entity<EditorPanel> {
    cx.update(|_, cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == path)).expect("the file is open")
}

/// Give the model and an editor what the engine answered for the fixture.
fn feed(ws: &Entity<Workspace>, repo: &Path, editor: &Entity<EditorPanel>, file: &str, cx: &mut VisualTestContext) {
    let f = spring::tests::fixture(repo);
    let modules = spring::parse_beans(&f["beans"]);
    let endpoints = spring::parse_endpoints(&f["endpoints"]);
    let marks = FileSpring::parse(&f["injections"][file], &endpoints);
    let model = cx.update(|_, cx| ws.read(cx).spring.clone());
    cx.update(|_, cx| model.update(cx, |s, cx| s.set_model(modules, endpoints, cx)));
    cx.run_until_parked();
    cx.update(|_, cx| {
        editor.update(cx, |e, cx| {
            e.spring = Some(Rc::new(marks));
            cx.notify();
        })
    });
    cx.run_until_parked();
}

/// The point at the middle of `lane` on the line holding `find`.
fn lane_point(
    editor: &Entity<EditorPanel>,
    find: &str,
    lane: fn(&crate::editor::Lanes) -> (gpui::Pixels, gpui::Pixels),
    cx: &mut VisualTestContext,
) -> gpui::Point<gpui::Pixels> {
    cx.update(|_, cx| {
        let e = editor.read(cx);
        let l = e.lanes.get();
        let s = e.state.read(cx);
        let off = s.text().to_string().find(find).expect("the text is there");
        let b = s.range_to_bounds(&(off..off)).unwrap();
        let (a, z) = lane(&l);
        gpui::point((a + z) / 2., b.center().y)
    })
}

#[gpui::test]
fn the_gutter_marks_beans_injections_and_endpoints(cx: &mut TestAppContext) {
    let repo = fixture("spring-gutter");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let file = java(&repo, "web/GreetingController.java");
    act(&model, cx, Action::OpenFile(file.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &file, cx);
    feed(&ws, &repo, &editor, "GreetingController", cx);
    // The lanes sit side by side, left of the line numbers, the breakpoints over them.
    let l = cx.update(|_, cx| editor.read(cx).lanes.get());
    let (p, r, s, b) = (l.problems(), l.run(), l.spring(), l.breakpoints());
    assert!(p.1 <= r.0 && r.1 <= s.0 && s.1 <= b.0, "{p:?} {r:?} {s:?} {b:?}");
    let pad = cx.update(|_, cx| editor.read(cx).gutter_pad);
    assert_eq!(pad, crate::editor::CODE_PAD);
    // A problem and a breakpoint on a marked line: each keeps its own lane.
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    cx.update(|_, cx| runs.update(cx, |r, cx| r.toggle_breakpoint(&file, 32, cx)));
    cx.run_until_parked();

    // The pointer on the endpoint's mark: its method and path.
    let at = lane_point(&editor, "public String hello", crate::editor::Lanes::spring, cx);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
    let tip = cx.update(|_, cx| editor.read(cx).gutter_tip.clone()).expect("a tooltip");
    assert_eq!(tip.2, "GET /api/hello");
    assert!(cx.debug_bounds("spring-tooltip").is_some(), "the tooltip shows");

    // The ambiguous injection: a chooser; picking one opens its class.
    let at = lane_point(&editor, "public GreetingController(", crate::editor::Lanes::spring, cx);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let menu = cx.update(|_, cx| matches!(editor.read(cx).gutter_menu, Some(GutterMenu::Spring { .. })));
    assert!(menu, "the chooser shows");
    assert!(cx.debug_bounds("gutter-menu").is_some());
    let english = cx.update(|_, cx| {
        let Some(GutterMenu::Spring { items, .. }) = &editor.read(cx).gutter_menu else { unreachable!() };
        items
            .iter()
            .position(
                |i| matches!(i, crate::editor::SpringItem::Go { label, .. } if label.starts_with("englishGreeting")),
            )
            .unwrap()
    });
    let row = cx.debug_bounds(Box::leak(format!("spring-go-{english}").into_boxed_str())).expect("the bean's row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    let target = java(&repo, "greeting/EnglishGreeting.java");
    let opened = editor_of(&ws, &target, cx);
    let line = cx.update(|_, cx| opened.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(line, 5, "the caret is on the class");
    // The breakpoint is still there, in its own lane.
    let bps = cx.update(|_, cx| runs.read(cx).breakpoints_in(&file).len());
    assert_eq!(bps, 1, "a click on a Spring mark sets no breakpoint");

    // A bean's mark lists where it is injected.
    let config = java(&repo, "config/AppConfig.java");
    act(&model, cx, Action::OpenFile(config.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &config, cx);
    feed(&ws, &repo, &editor, "AppConfig", cx);
    let at = lane_point(&editor, "public Clock clock()", crate::editor::Lanes::spring, cx);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let row = cx.debug_bounds("spring-go-1").expect("the injection site");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    let controller = editor_of(&ws, &file, cx);
    let line = cx.update(|_, cx| controller.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(line, 32, "the constructor that takes the clock");
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

#[gpui::test]
fn the_spring_window_lists_beans_and_endpoints(cx: &mut TestAppContext) {
    let repo = fixture("spring-window");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let file = java(&repo, "web/StatusController.java");
    act(&model, cx, Action::OpenFile(file.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &file, cx);
    feed(&ws, &repo, &editor, "GreetingController", cx);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.toggle_tool(Tool::Spring, w, cx)));
    cx.run_until_parked();
    assert!(cx.update(|_, cx| ws.read(cx).tool_open(Tool::Spring, cx)), "the Spring window shows");
    assert!(cx.debug_bounds("spring-tab-beans").is_some());
    let win = cx.update(|_, cx| ws.read(cx).spring_window.clone());
    // One module; its groups open with a click.
    let rows = cx.update(|_, cx| win.read(cx).bean_rows(cx).len());
    assert_eq!(rows, 5, "the module and its four packages");
    let row = cx.debug_bounds("spring-bean-row-3").expect("a package row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| win.read(cx).bean_rows(cx).len()), 7, "demo.greeting's two beans show");
    // A bean's row goes to its class.
    let row = cx.debug_bounds("spring-bean-row-4").expect("a bean row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    let english = java(&repo, "greeting/EnglishGreeting.java");
    assert!(cx.update(|_, cx| ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path == english)));
    // The stereotype filter, from its menu.
    let chip = cx.debug_bounds("spring-filter-stereotype").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let services = cx
        .update(|_, cx| spring::spring(cx).unwrap().read(cx).stereotypes())
        .iter()
        .position(|s| s == "Service")
        .unwrap();
    let item = cx.debug_bounds(Box::leak(format!("spring-menu-item-{}", services + 1).into_boxed_str())).unwrap();
    cx.simulate_click(item.center(), Modifiers::none());
    cx.run_until_parked();
    let names: Vec<String> = cx.update(|_, cx| {
        win.read(cx)
            .bean_rows(cx)
            .into_iter()
            .filter_map(|r| match r {
                crate::spring::window::BeanRow::Bean(b) => Some(b.name.clone()),
                _ => None,
            })
            .collect()
    });
    assert_eq!(names, ["englishGreeting", "frenchGreeting"]);

    // Endpoints: the table, sorted by a header's click, filtered by method.
    let tab = cx.debug_bounds("spring-tab-endpoints").unwrap();
    cx.simulate_click(tab.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| win.read(cx).tab), Tab::Endpoints);
    let header = cx.debug_bounds("spring-sort-handler").unwrap();
    cx.simulate_click(header.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| win.read(cx).endpoints.sort), Column::Handler);
    let paths = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| win.read(cx).endpoint_rows(cx).into_iter().map(|e| e.path).collect::<Vec<_>>())
    };
    assert_eq!(paths(cx), ["/api/echo", "/api/hello", "/status"]);
    let post = cx.debug_bounds("spring-method-POST").unwrap();
    cx.simulate_click(post.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(paths(cx), ["/api/echo"]);
    // Copy Path, then a click on the row goes to the handler.
    let copy = cx.debug_bounds("spring-copy-path-0").unwrap();
    cx.simulate_click(copy.center(), Modifiers::none());
    cx.run_until_parked();
    let clip = cx.update(|_, cx| cx.read_from_clipboard().and_then(|c| c.text()));
    assert_eq!(clip.as_deref(), Some("/api/echo"));
    let row = cx.debug_bounds("spring-endpoint-row-0").unwrap();
    cx.simulate_click(gpui::point(row.left() + gpui::px(20.), row.center().y), Modifiers::none());
    cx.run_until_parked();
    let controller = java(&repo, "web/GreetingController.java");
    let opened = editor_of(&ws, &controller, cx);
    let line = cx.update(|_, cx| opened.read(cx).state.read(cx).cursor_position().line);
    assert_eq!(line, 44, "the caret is on the handler");
    // Show in Endpoints, from a gutter menu, searches for the path.
    let model_entity = cx.update(|_, cx| ws.read(cx).spring.clone());
    cx.update(|_, cx| model_entity.update(cx, |s, cx| s.show_endpoint("/status".into(), cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| win.read(cx).endpoints.query.clone()), "/status");
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

#[gpui::test]
fn configuration_files_go_to_the_engine_as_spring_boot_files(cx: &mut TestAppContext) {
    let repo = fixture("spring-config");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    for (rel, id) in [
        ("src/main/resources/application.properties", "spring-boot-properties"),
        ("src/main/resources/application-dev.yml", "spring-boot-properties-yaml"),
    ] {
        let file = repo.join(rel);
        act(&model, cx, Action::OpenFile(file.clone(), Some((0, 0))));
        let editor = editor_of(&ws, &file, cx);
        let (uri, config, pad) = cx.update(|_, cx| {
            let e = editor.read(cx);
            let link = e.lang.as_ref().expect("the engine serves it");
            (link.uri.clone(), link.config, e.gutter_pad)
        });
        assert!(config);
        assert_eq!(pad, crate::editor::PLAIN_PAD, "no run or Spring lanes in a configuration file");
        assert_eq!(cx.update(|_, cx| hub.read(cx).language_of(&uri)), Some(id));
    }
    // Other properties files are only highlighted.
    let other = repo.join("src/main/resources/messages.properties");
    std::fs::write(&other, "greeting=Hello\n").unwrap();
    act(&model, cx, Action::OpenFile(other.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &other, cx);
    assert!(cx.update(|_, cx| editor.read(cx).lang.is_none()));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}

/// The JDK 17 the fixture builds with, if this machine has one.
fn jdk17() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("SNOUT_TEST_JDK17").map(PathBuf::from) {
        let jdk = crate::lang::java::jdk_at(&home).or_else(|| crate::lang::java::jdk_at(&home.join("Contents/Home")));
        return jdk.map(|j| j.home);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    crate::lang::java::discover(&home).into_iter().find(|j| j.major == 17).map(|j| j.home)
}

fn pump_until(cx: &mut VisualTestContext, secs: u64, mut done: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        if done(cx) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    false
}

#[gpui::test]
fn the_engine_feeds_the_gutter_the_window_hints_fixes_and_properties(cx: &mut TestAppContext) {
    let (Some(_), Some(jdk)) = (crate::lang::java::engine_dist(), jdk17()) else {
        println!("spring engine: skipped; needs engine/build/dist and a JDK 17 (SNOUT_TEST_JDK17)");
        return;
    };
    cx.executor().allow_parking();
    let _engine = crate::tests::engine_lock();
    let repo = fixture("spring-engine");
    let mut settings = with_repo(&repo);
    settings.project_jdks.insert(repo.clone(), jdk);
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, settings);
    cx.update(|w, cx| ws.update(cx, |ws, cx| ws.show_activity(Activity::Project, w, cx)));
    let file = java(&repo, "web/GreetingController.java");
    act(&model, cx, Action::OpenFile(file.clone(), Some((0, 0))));
    let editor = editor_of(&ws, &file, cx);
    let spring_model = cx.update(|_, cx| ws.read(cx).spring.clone());
    let loaded = pump_until(cx, 300, |cx| cx.update(|_, cx| spring_model.read(cx).loaded));
    assert!(loaded, "the Spring model is read once the engine is ready");
    let (beans, endpoints) =
        cx.update(|_, cx| (spring_model.read(cx).bean_count(), spring_model.read(cx).endpoints.len()));
    assert_eq!((beans, endpoints), (9, 3));
    // The gutter's marks, with the endpoints.
    let marked = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            editor.read(cx).spring.as_ref().is_some_and(|s| s.line(39).is_some_and(|l| !l.endpoints.is_empty()))
        })
    });
    assert!(marked, "the gutter has the file's marks");
    let kinds: Vec<u32> =
        cx.update(|_, cx| editor.read(cx).spring.as_ref().unwrap().lines.iter().map(|l| l.line).collect());
    assert_eq!(kinds, [18, 24, 27, 30, 32, 39, 44]);
    // The @Value hints.
    let hinted = pump_until(cx, 60, |cx| cx.update(|_, cx| editor.read(cx).hints.iter().any(|(_, t)| t == "= Howdy")));
    assert!(hinted, "the @Value hint shows the value");
    // The missing and ambiguous beans are warned about, with quick fixes.
    let warned = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            editor.read(cx).problems.iter().filter(|d| d.source.as_deref() == Some("spring")).count() == 2
        })
    });
    assert!(warned, "both Spring warnings are reported");
    let at = cx.update(|_, cx| {
        let t = editor.read(cx).text(cx);
        t.find("GreetingService greetings,").unwrap() + "GreetingService ".len() + 2
    });
    cx.update(|window, cx| {
        editor.update(cx, |e, cx| {
            e.state.update(cx, |s, cx| {
                s.set_selected_range(at..at, cx);
                s.focus(window, cx);
            })
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-.");
    let offered = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            editor
                .read(cx)
                .state
                .read(cx)
                .code_action_menu_state()
                .items
                .iter()
                .any(|i| i.action.title.contains("with @Qualifier"))
        })
    });
    assert!(offered, "⌘. offers to inject one with @Qualifier");
    cx.simulate_keystrokes("enter");
    let fixed = pump_until(cx, 30, |cx| cx.update(|_, cx| editor.read(cx).text(cx).contains("@Qualifier(\"")));
    assert!(fixed, "the quick fix adds the qualifier");
    assert!(cx.update(|_, cx| {
        editor.read(cx).text(cx).contains("import org.springframework.beans.factory.annotation.Qualifier;")
    }));
    // application.properties: completion of keys from the configuration properties.
    let props = repo.join("src/main/resources/application.properties");
    act(&model, cx, Action::OpenFile(props.clone(), Some((7, 0))));
    let props_editor = editor_of(&ws, &props, cx);
    let validated = pump_until(cx, 60, |cx| cx.update(|_, cx| !props_editor.read(cx).problems.is_empty()));
    assert!(validated, "the unknown key and the bad values are warned about");
    cx.update(|window, cx| {
        props_editor.update(cx, |e, cx| {
            e.state.update(cx, |s, cx| {
                let end = s.text().len();
                s.set_selected_range(end..end, cx);
                s.focus(window, cx);
            })
        })
    });
    cx.simulate_input("demo.ti");
    let completed = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            props_editor.read(cx).state.read(cx).completion_menu_state().items.iter().any(|i| i.label == "demo.timeout")
        })
    });
    assert!(completed, "completion offers demo.timeout");
    cx.simulate_keystrokes("escape");
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
    let _ = std::fs::remove_dir_all(repo.parent().unwrap());
}
