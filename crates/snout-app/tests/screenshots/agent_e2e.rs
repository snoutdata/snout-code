//! The real `claude` against the real app, rendered offscreen: the window over a project
//! with Claude Code in its console, connected to the window's IDE server with its hooks.
//! It sends a selection to Claude (⌥⌘K), types a prompt after the mention, and shoots
//! frames while Claude's edit types itself in (its caret and flag), then the window after
//! (the gutter's lane, the tint, the dots, the status bar's note, the card of a mark, Annotate
//! with Agents and the Agent Changes window).
//!
//! Needs a signed-in `claude`, so it runs only when asked:
//!
//! ```text
//! SNOUT_E2E_CLAUDE=<claude, or a wrapper that answers its trust question>
//! SNOUT_E2E_PROJECT=<a scratch project with src/main/java/com/example/shop/PriceCalculator.java>
//! SNOUT_E2E_LOCK_DIR=<the folder claude reads lock files from, ~/.claude/ide>
//! SNOUT_SCREENSHOT=<where the frames go>
//! cargo test -p snout-app --test screenshots
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext as _, HeadlessAppContext, px, size};
use snout_app::settings::{Settings, ThemeChoice};
use snout_app::workspace::Workspace;

pub fn run(claude: PathBuf, project: PathBuf, lock_dir: PathBuf, out: PathBuf) {
    std::fs::create_dir_all(&out).unwrap();
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::current_platform(true).text_system(),
        Arc::new(gpui_kit_assets::AllAssets),
        gpui_platform::current_headless_renderer,
    );
    let settings = Settings {
        theme: ThemeChoice::Light,
        repos: vec![project.clone()],
        current: Some(project.clone()),
        // Draft opens the console itself.
        console_open: std::env::var_os("SNOUT_E2E_DRAFT").is_none(),
        follow_claude: true,
        claude_path: Some(claude.clone()),
        ..Settings::default()
    };
    cx.update(|cx| {
        snout_app::init(&settings, cx);
        cx.set_global(snout_app::claude_ide::IdeConfig { lock_dir });
    });
    let model = cx.update(|cx| {
        let settings = settings.clone();
        cx.new(|cx| snout_app::model::Model::new(settings, None, cx))
    });
    let mut ws = None;
    let handle = cx
        .open_window(size(px(1400.), px(900.)), |window, cx| {
            let view = cx.new(|cx| Workspace::new(model.clone(), window, cx));
            ws = Some(view.clone());
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        })
        .unwrap();
    let ws = ws.unwrap();
    cx.allow_parking();
    let window: gpui::AnyWindowHandle = handle.into();

    let pump = |cx: &mut HeadlessAppContext, ms: u64| {
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            cx.run_until_parked();
            cx.advance_clock(Duration::from_millis(16));
            std::thread::sleep(Duration::from_millis(8));
            if Instant::now() >= end {
                break;
            }
        }
    };
    let shoot = |cx: &mut HeadlessAppContext, name: &str| {
        cx.update_window(window, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
        let image = cx.capture_screenshot(window).expect("offscreen rendering");
        image.save(out.join(format!("{name}.png"))).unwrap();
        println!("e2e: {name}");
    };
    let session = |cx: &mut HeadlessAppContext| {
        let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        cx.update(|cx| console.read(cx).session_text(&project)).unwrap_or_default()
    };
    let wait =
        |cx: &mut HeadlessAppContext, what: &str, secs: u64, done: &mut dyn FnMut(&mut HeadlessAppContext) -> bool| {
            let start = Instant::now();
            while !done(cx) {
                if start.elapsed() > Duration::from_secs(secs) {
                    panic!("e2e: timed out waiting for {what}");
                }
                cx.run_until_parked();
                cx.advance_clock(Duration::from_millis(16));
                std::thread::sleep(Duration::from_millis(15));
            }
        };

    // Draft, with the real agent: `SNOUT_E2E_DRAFT=1`, a project with something to commit, and
    // a folder Claude has not been told to trust, so its question comes first. Draft must wait
    // for the answer, then get the message through the editor's API. The console starts closed.
    if std::env::var_os("SNOUT_E2E_DRAFT").is_some() {
        use snout_app::model::Action;
        pump(&mut cx, 1_500);
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
        wait(&mut cx, "the change", 20, &mut |cx| cx.update(|cx| model.read(cx).can_draft()));
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::DraftMessage, cx)));
        wait(&mut cx, "the agent to print", 60, &mut |cx| !session(cx).trim().is_empty());
        pump(&mut cx, 4_000);
        shoot(&mut cx, "e2e-draft-1-waiting");
        let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        if snout_app::console::asks_the_user(&session(&mut cx)) {
            println!("e2e: it asks first, and the prompt has not gone");
            assert!(!cx.update(|cx| model.read(cx).draft.as_ref().is_some_and(|d| d.sent)));
            // Answer as the user would: down to "Yes, I trust this folder", Return.
            cx.update(|cx| console.read(cx).send_keys(&project, b"\x1b[B"));
            pump(&mut cx, 500);
            cx.update(|cx| console.read(cx).send_keys(&project, b"\r"));
            pump(&mut cx, 3_000);
        }
        let start = Instant::now();
        while cx.update(|cx| model.read(cx).draft.is_some()) {
            if start.elapsed() > Duration::from_secs(180) {
                shoot(&mut cx, "e2e-draft-timeout");
                println!("{}", session(&mut cx));
                panic!("e2e: no message after 180 s");
            }
            pump(&mut cx, 500);
            // A permission question would be the user's to answer: say so, it is what we test.
            if snout_app::console::asks_the_user(&session(&mut cx)) {
                println!("e2e: the agent asks:\n{}", session(&mut cx));
                shoot(&mut cx, "e2e-draft-asks");
                panic!("e2e: the agent asked for leave");
            }
        }
        pump(&mut cx, 800);
        shoot(&mut cx, "e2e-draft-2-done");
        let (message, status) = cx.update(|cx| (model.read(cx).commit_msg.clone(), model.read(cx).message.clone()));
        println!("e2e: status: {status}\ne2e: message:\n{message}\n---\n{}", session(&mut cx));
        assert!(!message.trim().is_empty(), "the message is in the box");
        return;
    }

    // Claude starts in the console and connects.
    wait(&mut cx, "Claude to connect", 120, &mut |cx| {
        cx.update(|cx| cx.try_global::<snout_app::claude_ide::IdeStatus>().is_some_and(|s| s.clients > 0))
            && session(cx).contains('❯')
    });
    pump(&mut cx, 3_000);
    println!("e2e: connected");

    // Open the file, select total(), and send it to Claude (⌥⌘K's action).
    let file = project.join("src/main/java/com/example/shop/PriceCalculator.java");
    cx.update_window(window, |_, window, cx| ws.update(cx, |w, cx| w.open_file(file.clone(), None, true, window, cx)))
        .unwrap();
    pump(&mut cx, 1_500);
    let editor = cx.update(|cx| ws.read(cx).editors(cx).into_iter().find(|e| e.read(cx).path == file)).unwrap();
    cx.update_window(window, |_, window, cx| {
        let state = editor.read(cx).state.clone();
        let text = state.read(cx).text().to_string();
        let a = text.find("    public BigDecimal total").unwrap();
        let b = text[a..].find("\n    }\n").map(|i| a + i + 7).unwrap();
        state.update(cx, |s, cx| {
            s.set_selected_range(a..b, cx);
            s.focus(window, cx);
        });
        ws.update(cx, |w, cx| snout_app::claude_ide::bridge::send_selection(w, window, cx));
    })
    .unwrap();
    wait(&mut cx, "the mention in Claude's prompt", 30, &mut |cx| session(cx).contains("PriceCalculator.java#L"));
    pump(&mut cx, 300);
    shoot(&mut cx, "e2e-01-sent");
    let text = session(&mut cx);
    let mention = text.lines().find(|l| l.contains("#L")).unwrap_or_default().trim().to_owned();
    println!("e2e: the prompt shows {mention}");

    // The prompt, after the mention.
    let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
    let prompt = "add a method discountFor(int items) right before total(), returning BigDecimal 0.10 for 10 or \
                  more items and BigDecimal.ZERO otherwise, and make total() take that discount off the sum \
                  before tax (the count is prices.size()). Give the class a one line Javadoc too. Use the Edit \
                  tool and run no commands.";
    cx.update(|cx| console.read(cx).type_text(&project, prompt));
    pump(&mut cx, 600);
    cx.update(|cx| console.read(cx).send_keys(&project, b"\r"));
    println!("e2e: prompt sent");

    // Frames while the edits type themselves in.
    let mut frames = 0;
    let mut last = Instant::now() - Duration::from_secs(1);
    let start = Instant::now();
    let mut quiet_since: Option<Instant> = None;
    loop {
        cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(16));
        std::thread::sleep(Duration::from_millis(10));
        let live = cx.update(|cx| ws.read(cx).editors(cx).iter().any(|e| e.read(cx).live.is_some()));
        if live && frames < 12 && last.elapsed() > Duration::from_millis(110) {
            frames += 1;
            last = Instant::now();
            shoot(&mut cx, &format!("e2e-02-live-{frames:02}"));
        }
        let listed = cx.update(|cx| {
            let c = snout_app::claude_ide::bridge::changes_of(ws.read(cx), cx);
            !c.read(cx).set.files.is_empty() && c.read(cx).busy().is_none()
        });
        let working = cx.update(|cx| cx.try_global::<snout_app::agents::ConsoleTurn>().is_some_and(|t| t.active));
        if listed && !working && !live {
            let q = *quiet_since.get_or_insert_with(Instant::now);
            if q.elapsed() > Duration::from_secs(8) {
                break;
            }
        } else {
            quiet_since = None;
        }
        assert!(start.elapsed() < Duration::from_secs(420), "e2e: Claude took too long:\n{}", session(&mut cx));
    }
    println!("e2e: {frames} live frames");
    pump(&mut cx, 2_500);

    shoot(&mut cx, "e2e-03-after");

    // The card of the first mark in the lane, and Annotate with Agents.
    cx.update(|cx| {
        editor.update(cx, |e, cx| {
            if let Some(run) = e.agent_lane(cx).into_iter().find(|r| !r.lines.is_empty()) {
                e.agent_card = Some(snout_app::editor::AgentCard {
                    lines: run.lines.clone(),
                    edit: run.edit,
                    at: gpui::point(px(600.), px(140.)),
                });
            }
            e.toggle_annotate(cx);
        })
    });
    pump(&mut cx, 500);
    shoot(&mut cx, "e2e-04-card-annotate");
    cx.update_window(window, |_, window, cx| {
        ws.update(cx, |w, cx| w.show_tool(snout_app::workspace::Tool::ClaudeChanges, window, cx))
    })
    .unwrap();
    pump(&mut cx, 800);
    shoot(&mut cx, "e2e-05-changes");
    let report = cx.update(|cx| {
        let c = snout_app::claude_ide::bridge::changes_of(ws.read(cx), cx);
        let c = c.read(cx);
        let files: Vec<String> =
            c.set.files.iter().map(|f| format!("{} +{} -{}", f.path.display(), f.added, f.removed)).collect();
        let turn = c.set.current().map(|t| (t.agent, t.prompt.clone()));
        let runs = c.ledger.owners(&file).map(|o| o.runs()).unwrap_or_default();
        format!("files {files:?}, turn {turn:?}, lines {runs:?}, hooks {}", c.hooks_heard)
    });
    println!("e2e: {report}");
}
