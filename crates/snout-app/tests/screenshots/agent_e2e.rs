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
//!
//! `SNOUT_E2E_INSTALL=<codex, opencode or amp>` instead runs that agent's real installer from
//! the console, as its Install button does, on this machine, and shoots what follows.
//! `SNOUT_E2E_AGENT=<id>` runs Draft (`SNOUT_E2E_DRAFT=1`) with that agent. `SNOUT_E2E_SWAP=1`
//! swaps between the real agents, fast and mid-Draft, and reports what it finds.
//! `SNOUT_E2E_LOOK=1` shoots the Git view of the project, without starting an agent.

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
    let install = std::env::var("SNOUT_E2E_INSTALL").ok();
    let picked = install.clone().or_else(|| std::env::var("SNOUT_E2E_AGENT").ok());
    let console_agent = snout_app::agents::ConsoleAgent::ALL
        .into_iter()
        .find(|a| picked.as_deref() == Some(a.id()))
        .unwrap_or_default();
    let look = std::env::var_os("SNOUT_E2E_LOOK").is_some();
    let window_size = std::env::var("SNOUT_SHOT_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some(size(px(w.parse().ok()?), px(h.parse().ok()?)))
        })
        .unwrap_or(size(px(1400.), px(900.)));
    let settings = Settings {
        console_agent,
        theme: ThemeChoice::Light,
        repos: vec![project.clone()],
        current: Some(project.clone()),
        // Draft opens the console itself.
        console_open: std::env::var_os("SNOUT_E2E_DRAFT").is_none() && !look,
        activity: look.then_some(snout_app::panels::Activity::Git),
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
        .open_window(window_size, |window, cx| {
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

    // An agent's real installer, from the console's offer to what the agent asks first.
    if install.is_some() {
        let id = console_agent.id();
        let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        pump(&mut cx, 4_000);
        shoot(&mut cx, &format!("install-{id}-1-offer"));
        cx.update_window(window, |_, window, cx| {
            console.update(cx, |c, cx| c.install(&project, id, None, window, cx));
        })
        .unwrap();
        wait(&mut cx, "the installer to start", 30, &mut |cx| cx.update(|cx| console.read(cx).installing(&project)));
        let start = Instant::now();
        let mut frame = 0;
        while cx.update(|cx| console.read(cx).installing(&project)) {
            assert!(start.elapsed() < Duration::from_secs(420), "e2e: the installer is still at work");
            pump(&mut cx, 5_000);
            frame += 1;
            if frame == 2 {
                shoot(&mut cx, &format!("install-{id}-2-working"));
            }
            if frame % 6 == 0 {
                println!("e2e: installing, {:?}:\n{}", start.elapsed(), session(&mut cx));
            }
            let text = session(&mut cx);
            if text.contains("ended") || text.contains("exited") {
                println!("e2e: the installer says:\n{text}");
            }
        }
        println!("e2e: the installer finished after {:?}", start.elapsed());
        pump(&mut cx, 12_000);
        shoot(&mut cx, &format!("install-{id}-3-after"));
        println!("e2e: the console now:\n{}", session(&mut cx));
        return;
    }

    // The project's Git view, as it looks: long names, many files.
    if look {
        use snout_app::model::Action;
        pump(&mut cx, 3_000);
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
        wait(&mut cx, "the files", 30, &mut |cx| cx.update(|cx| !model.read(cx).status.files.is_empty()));
        pump(&mut cx, 1_500);
        let first = cx.update(|cx| model.read(cx).status.files[0].path.clone());
        cx.update(|cx| model.update(cx, |m, cx| m.click_file(&first, false, false, cx)));
        pump(&mut cx, 1_500);
        let frame = Instant::now();
        shoot(&mut cx, "look-git");
        println!(
            "e2e: a frame and its image took {:?}, {} files",
            frame.elapsed(),
            cx.update(|cx| model.read(cx).status.files.len())
        );
        let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        cx.update(|cx| console.update(cx, |c, _| c.program = Some(PathBuf::from("/bin/cat"))));
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ToggleConsole, cx)));
        pump(&mut cx, 1_500);
        shoot(&mut cx, "look-git-console");
        let id = cx.update(|cx| model.read(cx).log.last().map(|c| c.id.clone())).unwrap();
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::ShowCommit(id), cx)));
        pump(&mut cx, 2_500);
        shoot(&mut cx, "look-commit-console");
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::CloseCommit, cx)));
        cx.update_window(window, |_, window, cx| {
            ws.update(cx, |w, cx| w.set_activity(snout_app::panels::Activity::Project, window, cx));
        })
        .unwrap();
        pump(&mut cx, 2_000);
        shoot(&mut cx, "look-project");
        return;
    }

    // Swapping between the real agents, fast and in the middle of a Draft. It goes through it
    // all and then says what went wrong.
    if std::env::var_os("SNOUT_E2E_SWAP").is_some() {
        use snout_app::agents::ConsoleAgent;
        use snout_app::model::Action;
        let console = cx.update(|cx| snout_app::console::console_of(ws.read(cx), cx)).unwrap();
        let mut trouble: Vec<String> = Vec::new();
        let agent_named = |id: &str| ConsoleAgent::ALL.into_iter().find(|a| a.id() == id).unwrap();
        // The agents this window's consoles run, by what the system says.
        let children = || {
            let me = std::process::id().to_string();
            let out = std::process::Command::new("pgrep").args(["-P", &me, "-l"]).output().unwrap();
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| l.split_whitespace().nth(1).map(str::to_owned))
                // Claude's process goes by its version number.
                .filter(|n| {
                    n.starts_with(|c: char| c.is_ascii_digit())
                        || ["claude", "codex", "node", "opencode", "amp"].iter().any(|a| n.contains(a))
                })
                .collect::<Vec<_>>()
        };
        let swap = |cx: &mut HeadlessAppContext, id: &str| {
            let which = agent_named(id);
            cx.update_window(window, |_, window, cx| {
                console.update(cx, |c, cx| c.switch_agent(which, window, cx));
            })
            .unwrap();
        };

        pump(&mut cx, 3_000);
        wait(&mut cx, "the first agent", 60, &mut |cx| !session(cx).trim().is_empty());

        // 1. Fast: one pick after another, none waited for. The last one counts.
        for id in ["codex", "amp", "opencode", "claude", "amp", "codex", "opencode"] {
            swap(&mut cx, id);
            pump(&mut cx, 150);
        }
        pump(&mut cx, 12_000);
        shoot(&mut cx, "swap-1-fast");
        let now = cx.update(|cx| console.read(cx).agent(cx).id);
        let kids = children();
        println!("e2e: after the fast swaps: {now}, running {kids:?}\n{}", session(&mut cx));
        if now != "opencode" {
            trouble.push(format!("fast swaps ended on {now}, not opencode"));
        }
        if kids.len() != 1 {
            trouble.push(format!("fast swaps left {kids:?} running"));
        }

        // 2. Each in turn, given time to come up.
        for id in ["claude", "codex", "amp", "opencode"] {
            swap(&mut cx, id);
            pump(&mut cx, 9_000);
            shoot(&mut cx, &format!("swap-2-{id}"));
            let (now, kids) = (cx.update(|cx| console.read(cx).agent(cx).id), children());
            let text = session(&mut cx);
            println!("e2e: {id}: {now}, running {kids:?}, {} characters on screen", text.len());
            if now != id || kids.len() != 1 || text.trim().is_empty() {
                trouble.push(format!("{id}: shows {now}, running {kids:?}, screen empty: {}", text.trim().is_empty()));
            }
        }

        // 3. Draft with opencode, Stop at once: nothing in the box, and the prompt taken back.
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::Refresh, cx)));
        wait(&mut cx, "the change", 20, &mut |cx| cx.update(|cx| model.read(cx).can_draft()));
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::DraftMessage, cx)));
        pump(&mut cx, 300);
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::CancelDraft, cx)));
        pump(&mut cx, 25_000);
        let (drafting, message) = cx.update(|cx| (model.read(cx).draft.is_some(), model.read(cx).commit_msg.clone()));
        shoot(&mut cx, "swap-3-stop");
        println!("e2e: stopped: drafting {drafting}, message {message:?}");
        if drafting || !message.trim().is_empty() {
            trouble.push(format!("after Stop: drafting {drafting}, message {message:?}"));
        }

        // 4. Draft, then swap to another agent while it waits: it ends or goes on, never hangs.
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::DraftMessage, cx)));
        pump(&mut cx, 500);
        swap(&mut cx, "amp");
        let start = Instant::now();
        while cx.update(|cx| model.read(cx).draft.is_some()) && start.elapsed() < Duration::from_secs(120) {
            pump(&mut cx, 500);
        }
        shoot(&mut cx, "swap-4-mid-draft");
        let (drafting, message, status) = cx.update(|cx| {
            let m = model.read(cx);
            (m.draft.is_some(), m.commit_msg.clone(), m.message.clone())
        });
        println!(
            "e2e: swapped mid-Draft after {:?}: drafting {drafting}, message {message:?}, status {status:?}",
            start.elapsed()
        );
        if drafting {
            trouble.push("a Draft whose agent was swapped away still waits after 2 minutes".into());
        }
        cx.update(|cx| model.update(cx, |m, _| m.commit_msg.clear()));

        // 5. Draft, then end the session: the Draft ends with it.
        swap(&mut cx, "opencode");
        pump(&mut cx, 6_000);
        cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::DraftMessage, cx)));
        pump(&mut cx, 400);
        cx.update(|cx| console.update(cx, |c, cx| c.end_session(&project, cx)));
        pump(&mut cx, 3_000);
        let (drafting, status) = cx.update(|cx| (model.read(cx).draft.is_some(), model.read(cx).message.clone()));
        shoot(&mut cx, "swap-5-end-session");
        println!("e2e: session ended mid-Draft: drafting {drafting}, status {status:?}");
        if drafting {
            trouble.push("a Draft outlived the session it was waiting on".into());
        }

        // 6. Draft, then open another project: the message must not land in that one's box.
        if let Some(other) = std::env::var_os("SNOUT_E2E_OTHER").map(PathBuf::from) {
            cx.update_window(window, |_, window, cx| {
                console.update(cx, |c, cx| c.ensure_session(&project, window, cx));
            })
            .unwrap();
            pump(&mut cx, 6_000);
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::DraftMessage, cx)));
            pump(&mut cx, 500);
            cx.update(|cx| model.update(cx, |m, cx| m.handle(Action::OpenRepo(other.clone()), cx)));
            let start = Instant::now();
            while cx.update(|cx| model.read(cx).draft.is_some()) && start.elapsed() < Duration::from_secs(60) {
                pump(&mut cx, 500);
            }
            pump(&mut cx, 25_000);
            let (root, drafting, message) = cx.update(|cx| {
                let m = model.read(cx);
                (m.project_root(), m.draft.is_some(), m.commit_msg.clone())
            });
            shoot(&mut cx, "swap-6-other-project");
            println!("e2e: other project {root:?}: drafting {drafting}, message {message:?}");
            if drafting || !message.trim().is_empty() {
                trouble.push(format!("after changing project: drafting {drafting}, message {message:?}"));
            }
        }

        println!("e2e: trouble: {trouble:#?}");
        return;
    }

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
            println!("e2e: it asks:\n{}", session(&mut cx));
            // Answer as the user would. Claude's first answer is "No": down to "Yes", Return.
            if console_agent.id() == "claude" {
                cx.update(|cx| console.read(cx).send_keys(&project, b"\x1b[B"));
                pump(&mut cx, 500);
            }
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
            let quiet = cx.update(|cx| console.read(cx).quiet_for(&project));
            if start.elapsed().as_secs().is_multiple_of(10) {
                println!("e2e: {:?} in, quiet for {quiet:?}", start.elapsed());
            }
            // `SNOUT_E2E_ANSWER=1`: say yes as the user would, and go on.
            if std::env::var_os("SNOUT_E2E_ANSWER").is_some() && snout_app::console::asks_the_user(&session(&mut cx)) {
                println!("e2e: answering yes after {:?}:\n{}", start.elapsed(), session(&mut cx));
                shoot(&mut cx, &format!("e2e-draft-asks-{}", start.elapsed().as_secs()));
                cx.update(|cx| console.read(cx).send_keys(&project, b"\r"));
                pump(&mut cx, 2_000);
                continue;
            }
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
