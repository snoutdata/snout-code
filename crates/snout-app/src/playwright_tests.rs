//! Playwright projects and Node: how a project is recognised, what it asks of Node, and
//! which install is chosen.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::lang::node::{self, Node, PackageManager, Wanted};
use crate::playwright;
use crate::tests::scratch_dir;

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn n(version: &str) -> Node {
    Node {
        bin: PathBuf::from(format!("/n/{version}/bin/node")),
        version: version.into(),
        major: node::major_of(version).unwrap(),
    }
}

#[test]
fn a_playwright_repo_is_recognised_by_its_config_or_its_package() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-detect");
    assert!(!playwright::is_root(&*host, &dir), "an empty folder is not one");

    write(&dir, "package.json", r#"{ "name": "shop-e2e", "devDependencies": { "@playwright/test": "^1.48.0" } }"#);
    let p = playwright::project_at(&*host, &dir).expect("the package makes it one");
    assert_eq!(p.declared.as_deref(), Some("^1.48.0"));
    assert_eq!(p.config, None);
    assert!(!p.installed(), "nothing is installed yet");
    assert_eq!(p.package_manager, PackageManager::Npm);

    write(&dir, "playwright.config.ts", "export default {};\n");
    write(&dir, "node_modules/@playwright/test/package.json", r#"{ "version": "1.48.2" }"#);
    write(&dir, "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    let p = playwright::project_at(&*host, &dir).unwrap();
    assert_eq!(p.config, Some(dir.join("playwright.config.ts")));
    assert_eq!(p.installed.as_deref(), Some("1.48.2"));
    assert_eq!(p.package_manager, PackageManager::Pnpm);

    write(&dir, "tests/checkout/pay.spec.ts", "");
    assert_eq!(playwright::root_of(&*host, &dir.join("tests/checkout/pay.spec.ts"), None), Some(dir.clone()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_package_without_playwright_is_not_a_playwright_project() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-not");
    write(&dir, "package.json", r#"{ "name": "web", "dependencies": { "react": "^19.0.0" } }"#);
    assert!(!playwright::is_root(&*host, &dir));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_files_follow_playwrights_default_match() {
    for yes in ["login.spec.ts", "a/b/cart.test.js", "pay.spec.tsx", "x.spec.mjs"] {
        assert!(playwright::is_test_file(Path::new(yes)), "{yes}");
    }
    for no in ["playwright.config.ts", "pages/login.ts", "spec.ts", "notes.spec.md"] {
        assert!(!playwright::is_test_file(Path::new(no)), "{no}");
    }
}

#[test]
fn node_version_specs_read_as_nvm_volta_and_engines_write_them() {
    assert_eq!(node::parse_spec("v22.11.0").exact.as_deref(), Some("22.11.0"));
    assert_eq!(node::parse_spec("22").major, Some(22));
    assert_eq!(node::parse_spec("20.9").major, Some(20));
    assert!(node::parse_spec("lts/*").lts);
    assert!(node::parse_spec("lts/jod").lts);
    assert_eq!(node::parse_spec(">=18").min_major, Some(18));
    assert_eq!(node::parse_spec(">=18.0.0 <23").min_major, Some(18));
    assert_eq!(node::parse_spec("^20.9.0").min_major, Some(20));
    assert_eq!(node::parse_spec("18.x || 20.x").min_major, Some(18));
    assert_eq!(node::parse_spec("node"), Wanted::default());
}

#[test]
fn the_nvmrc_wins_over_package_json_and_is_looked_for_above() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-node-wanted");
    write(&dir, "e2e/package.json", r#"{ "engines": { "node": ">=20" } }"#);
    let w = node::wanted(&*host, &dir.join("e2e"), None);
    assert_eq!((w.min_major, w.source.as_deref()), (Some(20), Some("package.json engines")));
    write(&dir, "e2e/package.json", r#"{ "volta": { "node": "22.11.0" }, "engines": { "node": ">=20" } }"#);
    assert_eq!(node::wanted(&*host, &dir.join("e2e"), None).exact.as_deref(), Some("22.11.0"));
    write(&dir, "e2e/.nvmrc", "# pinned\nv20\n");
    let w = node::wanted(&*host, &dir.join("e2e"), None);
    assert_eq!((w.major, w.source.as_deref()), (Some(20), Some(".nvmrc")));
    std::fs::remove_file(dir.join("e2e/.nvmrc")).unwrap();
    std::fs::remove_file(dir.join("e2e/package.json")).unwrap();
    write(&dir, ".node-version", "18\n");
    assert_eq!(node::wanted(&*host, &dir.join("e2e"), None).major, Some(18), "a folder above counts");
    assert_eq!(node::wanted(&*host, &dir.join("e2e"), Some(&dir.join("e2e"))), Wanted::default(), "not above the stop");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_newest_node_that_fits_is_chosen() {
    let all = [n("24.13.0"), n("22.11.0"), n("22.3.0"), n("20.18.1"), n("18.20.4")];
    let pick = |spec: &str| node::choose(&node::parse_spec(spec), &all).map(|n| n.version.clone());
    assert_eq!(pick("").as_deref(), Some("24.13.0"));
    assert_eq!(pick("22").as_deref(), Some("22.11.0"));
    assert_eq!(pick("22.3.0").as_deref(), Some("22.3.0"));
    assert_eq!(pick("22.4.1").as_deref(), Some("22.11.0"), "an exact version not there takes its major");
    assert_eq!(pick(">=19").as_deref(), Some("24.13.0"));
    assert_eq!(pick("lts/*").as_deref(), Some("24.13.0"));
    assert_eq!(pick("16"), None);
    assert_eq!(node::parse_spec("16").download_major(), 16);
    assert_eq!(node::parse_spec("").download_major(), node::LTS_MAJOR);
}

#[test]
fn this_machines_node_is_found_when_it_has_one() {
    let host = snout_host::local();
    let all = node::discover(&*host);
    for node in &all {
        assert!(node.bin.is_file() && node.major > 0, "{node:?}");
    }
    assert!(all.windows(2).all(|w| w[0].major >= w[1].major), "newest first");
}

#[gpui::test]
fn typescript_files_get_vtsls_and_no_java(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::{TestWorkspace, caret_at, editor_of, pump_until};
    use crate::model::Action;
    use crate::tests::{act, open, with_repo};
    if node::discover(&*snout_host::local()).is_empty() {
        println!("typescript features: skipped; needs Node");
        return;
    }
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-ts");
    write(&repo, "package.json", r#"{ "name": "shop-e2e", "devDependencies": { "@playwright/test": "^1.48.0" } }"#);
    write(
        &repo,
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true, "target": "ES2022", "module": "ESNext" } }"#,
    );
    write(&repo, "tests/cart.spec.ts", "export const count: number = 'three';\nconst label = 'cart';\n\n");
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, with_repo(&repo));
    cx.update(|_, cx| cx.set_global(TestWorkspace(ws.clone())));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    // vtsls goes into a folder of its own that runs share, never the app's data folder.
    let install = std::env::temp_dir().join("snout-test-vtsls").join(crate::lang::typescript::VTSLS_VERSION);
    cx.update(|_, cx| hub.update(cx, |h, _| h.ts_install_dir = Some(install)));
    let spec = repo.join("tests/cart.spec.ts");
    act(&model, cx, Action::OpenFile(spec.clone(), None));
    let editor = editor_of(&ws, &spec, cx).expect("the spec opens");

    let started = Instant::now();
    let flagged = pump_until(cx, 180, |cx| cx.update(|_, cx| !editor.read(cx).problems.is_empty()));
    let stage = cx.update(|_, cx| hub.read(cx).ts.get(&repo).map(|s| s.stage.label()));
    assert!(flagged, "vtsls reports the type error within three minutes ({stage:?})");
    println!("typescript features: first diagnostics after {:.1} s", started.elapsed().as_secs_f32());
    let message = cx.update(|_, cx| editor.read(cx).problems.first().map(|d| d.message.clone()).unwrap_or_default());
    assert!(message.contains("not assignable to type 'number'"), "{message}");
    let status = cx.update(|_, cx| ws.read(cx).status.read(cx).java_status(cx)).expect("a status");
    assert!(status.0.starts_with("TypeScript: "), "{status:?}");
    assert!(cx.update(|_, cx| hub.read(cx).servers.is_empty()), "no Java engine started");
    let problems = cx.update(|_, cx| hub.read(cx).all_problems());
    assert!(problems.iter().any(|(uri, _)| uri.ends_with("cart.spec.ts")), "the Problems window lists it");

    // Completion on a string.
    caret_at(&editor, "\n\n", 0, 2, cx);
    cx.simulate_input("label.toUpper");
    let completed = pump_until(cx, 60, |cx| {
        cx.update(|_, cx| {
            editor
                .read(cx)
                .state
                .read(cx)
                .completion_menu_state()
                .items
                .iter()
                .any(|i| i.label.starts_with("toUpperCase"))
        })
    });
    assert!(completed, "completion offers toUpperCase");
    cx.simulate_keystrokes("escape");

    // Quick fixes from the Problems window go to the TypeScript server too.
    let uri = snout_lang::uri::from_path(&spec);
    let call = cx.update(|_, cx| {
        let d = editor.read(cx).problems.first().cloned().expect("a problem");
        let params = serde_json::json!({
            "textDocument": { "uri": uri.as_str() }, "range": d.range, "context": { "diagnostics": [d] }
        });
        hub.read(cx).request_doc(&uri, &repo, "textDocument/codeAction", params)
    });
    assert!(futures::executor::block_on(call).is_ok(), "vtsls answers code actions");
    cx.update(|_, cx| hub.update(cx, |h, _| h.shutdown_all()));
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn the_node_download_is_the_release_asked_for() {
    let index = serde_json::json!([
        { "version": "v24.13.0", "files": ["osx-arm64-tar", "linux-x64", "win-x64-zip"] },
        { "version": "v22.12.0", "files": ["osx-arm64-tar", "linux-x64", "win-x64-zip"] },
        { "version": "v22.11.0", "files": ["osx-arm64-tar", "linux-x64", "win-x64-zip"] },
        { "version": "v20.18.1", "files": ["linux-x64"] }
    ]);
    let pick = |spec: &str, platform: &str| node::pick_release(&index, &node::parse_spec(spec), platform);
    assert_eq!(pick("22", "darwin-arm64").as_deref(), Some("22.12.0"), "the newest of the major");
    assert_eq!(pick("22.11.0", "darwin-arm64").as_deref(), Some("22.11.0"));
    assert_eq!(pick("", "linux-x64").as_deref(), Some("22.12.0"), "nothing asked: the long-term support major");
    assert_eq!(pick("20", "darwin-arm64"), None, "no build for this machine");
    assert_eq!(pick("20", "linux-x64").as_deref(), Some("20.18.1"));
    assert_eq!(pick("24", "win-x64").as_deref(), Some("24.13.0"));
}

/// Downloads Node from nodejs.org (about 50 MB): run by hand with `--ignored`.
#[test]
#[ignore]
fn node_downloads_from_nodejs_org() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-node-download");
    let progress = crate::lang::java::DownloadProgress::default();
    let n = node::download(&*host, &node::parse_spec("22"), &dir, &progress).expect("Node downloads");
    assert_eq!(n.major, 22);
    assert!(n.bin.starts_with(&dir), "{n:?}");
    assert!(crate::lang::typescript::npm_cli(&*host, &n).is_some(), "npm comes with it");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_project_asking_for_a_node_this_machine_lacks_offers_to_download_it(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::{editor_of, pump_until};
    use crate::model::Action;
    use crate::tests::{act, open, with_repo};
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-needs-node");
    // A version no one has.
    write(&repo, ".nvmrc", "v3.1.4\n");
    write(&repo, "package.json", r#"{ "devDependencies": { "@playwright/test": "^1.48.0" } }"#);
    write(&repo, "tests/a.spec.ts", "export {};\n");
    cx.update(|cx| cx.set_global(crate::lang::hub::LangEnabled(true)));
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let hub = cx.update(|_, cx| ws.read(cx).hub.clone());
    let spec = repo.join("tests/a.spec.ts");
    act(&model, cx, Action::OpenFile(spec.clone(), None));
    assert!(editor_of(&ws, &spec, cx).is_some());
    let offered = pump_until(cx, 30, |cx| cx.update(|_, cx| hub.read(cx).node_prompt.is_some()));
    assert!(offered, "the Node bar shows");
    let (label, stage) = cx.update(|_, cx| {
        let h = hub.read(cx);
        (h.node_prompt.as_ref().unwrap().label.clone(), h.ts.get(&repo).map(|s| s.status()))
    });
    assert_eq!(label, "3.1.4");
    assert_eq!(stage.as_deref(), Some("TypeScript: Needs Node 3.1.4"));
    assert!(cx.debug_bounds("node-bar").is_some(), "the bar is drawn");
    cx.update(|_, cx| hub.update(cx, |h, cx| h.dismiss_node(cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("node-bar").is_none(), "Not now hides it");
    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn browsers_count_as_installed_by_their_revision_folders() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-browsers");
    let manifest = serde_json::json!({ "browsers": [
        { "name": "chromium", "revision": "1248", "installByDefault": true },
        { "name": "chromium-headless-shell", "revision": "1248", "installByDefault": true },
        { "name": "webkit", "revision": "2370", "installByDefault": true, "revisionOverrides": { "mac14": "2251" } },
        { "name": "android", "revision": "1001", "installByDefault": false }
    ]});
    let missing = |cx: &Path| crate::playwright::setup::missing_browsers(&*host, &manifest, cx);
    assert_eq!(missing(&dir), ["Chromium", "Chromium Headless Shell", "WebKit"]);
    std::fs::create_dir_all(dir.join("chromium-1248")).unwrap();
    std::fs::create_dir_all(dir.join("chromium_headless_shell-1248")).unwrap();
    std::fs::create_dir_all(dir.join("webkit-2251")).unwrap();
    std::fs::create_dir_all(dir.join("chromium-1100")).unwrap();
    assert!(missing(&dir).is_empty(), "an override's revision counts; an older one is no harm");
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn a_playwright_project_without_its_packages_offers_to_install_them(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::playwright::setup::Need;
    use crate::tests::{open, with_repo};
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-setup");
    write(&repo, "package.json", r#"{ "devDependencies": { "@playwright/test": "1.64.1" } }"#);
    write(&repo, "playwright.config.ts", "export default {};\n");
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let setup = cx.update(|_, cx| ws.read(cx).pw_setup.clone());
    let found = pump_until(cx, 10, |cx| cx.update(|_, cx| setup.read(cx).need.is_some()));
    assert!(found, "the project is looked at");
    let need = cx.update(|_, cx| setup.read(cx).need.clone());
    assert_eq!(need, Some(Need::Packages { manager: PackageManager::Npm, dir: None }));
    assert!(cx.debug_bounds("playwright-bar").is_some() && cx.debug_bounds("playwright-install").is_some());
    let summary = cx.update(|_, cx| setup.read(cx).summary.clone()).unwrap_or_default();
    assert!(summary.starts_with("Playwright 1.64.1, not installed · npm · "), "{summary}");
    assert!(cx.debug_bounds("project-model-note").is_some(), "the Project view says it");

    // Installed, with browsers of revisions no machine has: the browsers are next.
    write(&repo, "node_modules/@playwright/test/package.json", r#"{ "version": "1.64.1" }"#);
    write(&repo, "node_modules/playwright-core/package.json", r#"{ "version": "1.64.1" }"#);
    write(
        &repo,
        "node_modules/playwright-core/browsers.json",
        r#"{ "browsers": [{ "name": "firefox", "revision": "999999", "installByDefault": true }] }"#,
    );
    cx.update(|_, cx| setup.update(cx, |s, cx| s.look(cx)));
    let next = pump_until(cx, 10, |cx| cx.update(|_, cx| matches!(setup.read(cx).need, Some(Need::Browsers { .. }))));
    assert!(next, "{:?}", cx.update(|_, cx| setup.read(cx).need.clone()));
    let need = cx.update(|_, cx| setup.read(cx).need.clone());
    assert_eq!(need, Some(Need::Browsers { version: "1.64.1".into(), missing: vec!["Firefox".into()] }));
    cx.update(|_, cx| setup.update(cx, |s, cx| s.dismiss(cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("playwright-bar").is_none(), "Not now hides it");

    // A folder that is not a Playwright project needs nothing.
    std::fs::remove_file(repo.join("package.json")).unwrap();
    std::fs::remove_file(repo.join("playwright.config.ts")).unwrap();
    cx.update(|_, cx| setup.update(cx, |s, cx| s.look(cx)));
    let none = pump_until(cx, 10, |cx| cx.update(|_, cx| setup.read(cx).need.is_none()));
    assert!(none);
    let _ = std::fs::remove_dir_all(repo);
}

/// Installs @playwright/test from npm through the bar (network): run by hand with `--ignored`.
#[gpui::test]
#[ignore]
fn the_bar_installs_the_packages_in_the_build_window(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::playwright::setup::Need;
    use crate::tests::{open, with_repo};
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-setup-install");
    write(
        &repo,
        "package.json",
        r#"{ "name": "e2e", "private": true, "devDependencies": { "@playwright/test": "1.64.1" } }"#,
    );
    let (_, ws, cx) = open(cx, with_repo(&repo));
    let (setup, runs) = cx.update(|_, cx| (ws.read(cx).pw_setup.clone(), ws.read(cx).runs.clone()));
    assert!(pump_until(cx, 10, |cx| cx.update(|_, cx| setup.read(cx).need.is_some())));
    cx.update(|_, cx| setup.update(cx, |s, cx| s.install(cx)));
    assert!(pump_until(cx, 10, |cx| cx.update(|_, cx| setup.read(cx).installing.is_some())), "it starts");
    let id = cx.update(|_, cx| setup.read(cx).installing.unwrap());
    let done = pump_until(cx, 240, |cx| cx.update(|_, cx| setup.read(cx).installing.is_none()));
    let text = cx.update(|_, cx| runs.read(cx).session(id).map(|s| s.console_text()).unwrap_or_default());
    assert!(done, "{text}");
    println!("{text}");
    let next = pump_until(cx, 10, |cx| cx.update(|_, cx| !matches!(setup.read(cx).need, Some(Need::Packages { .. }))));
    assert!(next, "the packages are there now");
    assert!(repo.join("node_modules/@playwright/test/cli.js").is_file());
    let _ = std::fs::remove_dir_all(repo);
}

#[gpui::test]
fn the_new_project_dialog_makes_a_playwright_project(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::playwright::setup::Need;
    use crate::tests::{open, with_repo};
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-new-from");
    let place = scratch_dir("pw-new-into");
    let (model, ws, cx) = open(cx, with_repo(&repo));
    let click = |cx: &mut gpui::VisualTestContext, id: &'static str| {
        let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} is drawn")).center();
        cx.simulate_click(at, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    cx.dispatch_action(crate::menus::NewProject);
    cx.run_until_parked();
    click(cx, "new-project-kind-playwright");
    assert!(cx.debug_bounds("new-project-groovy").is_none(), "no build tool to pick");
    let set = |cx: &mut gpui::VisualTestContext, url: &str| {
        let (url, place) = (url.to_owned(), place.display().to_string());
        cx.update(|w, cx| {
            let view = ws.read(cx).new_project.clone().unwrap();
            let v = view.read(cx);
            let (n, l, u) = (v.name.clone(), v.location.clone(), v.base_url.clone());
            n.update(cx, |i, cx| i.set_value("Shop E2E", w, cx));
            l.update(cx, |i, cx| i.set_value(place, w, cx));
            u.update(cx, |i, cx| i.set_value(url, w, cx));
        });
        cx.run_until_parked();
    };
    set(cx, "staging.shop.test");
    let problem = cx.update(|_, cx| ws.read(cx).new_project.as_ref().unwrap().read(cx).problem(cx));
    assert!(problem.is_some_and(|p| p.contains("http")), "an address without its scheme");
    set(cx, "https://staging.shop.test");
    let problem = cx.update(|_, cx| ws.read(cx).new_project.as_ref().unwrap().read(cx).problem(cx));
    assert_eq!(problem, None);
    click(cx, "new-project-create");
    let dir = place.join("Shop E2E");
    let opened =
        pump_until(cx, 30, |cx| cx.update(|_, cx| model.read(cx).project_root().as_deref() == Some(dir.as_path())));
    assert!(opened, "the new project opens");
    let package: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).unwrap()).unwrap();
    assert_eq!(package["name"], "shop-e2e");
    assert!(package["devDependencies"]["@playwright/test"].is_string());
    let config = std::fs::read_to_string(dir.join("playwright.config.ts")).unwrap();
    assert!(config.contains("process.env.BASE_URL ?? 'https://staging.shop.test'"), "{config}");
    assert!(dir.join("tests/home.spec.ts").is_file() && dir.join("tsconfig.json").is_file());
    assert!(std::fs::read_to_string(dir.join(".gitignore")).unwrap().contains("playwright-report/"));
    assert!(dir.join(".git").exists());
    assert!(playwright::is_root(&*snout_host::local(), &dir));
    // It opens offering to install its packages.
    let setup = cx.update(|_, cx| ws.read(cx).pw_setup.clone());
    let offered = pump_until(cx, 10, |cx| {
        cx.update(|_, cx| {
            matches!(setup.read(cx).need, Some(Need::Packages { manager: PackageManager::Npm, dir: None }))
        })
    });
    assert!(offered);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(place);
}

/// The new project's files work with the real Playwright (network): run by hand with `--ignored`.
#[test]
#[ignore]
fn a_new_playwright_project_lists_its_tests_in_every_browser() {
    let host = snout_host::local();
    let dir = scratch_dir("pw-new-real");
    crate::new_project::generate::write_playwright(&*host, &dir, "shop-e2e", "https://example.com").unwrap();
    let node = node::discover(&*host).into_iter().next().expect("Node");
    let npm = crate::lang::typescript::npm_cli(&*host, &node).unwrap();
    let path = crate::lang::typescript::path_with(&*host, &node);
    let run = |args: &[&str]| {
        let out =
            std::process::Command::new(&node.bin).args(args).current_dir(&dir).env("PATH", &path).output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
        )
    };
    let (ok, text) = run(&[npm.to_str().unwrap(), "install", "--no-audit", "--no-fund", "typescript@5"]);
    assert!(ok, "{text}");
    let (ok, text) = run(&["node_modules/@playwright/test/cli.js", "test", "--list"]);
    assert!(ok, "{text}");
    println!("{text}");
    for project in ["[chromium]", "[firefox]", "[webkit]"] {
        assert!(text.contains(project) && text.contains("the home page loads"), "{text}");
    }
    let (ok, text) = run(&["node_modules/typescript/bin/tsc", "-p", "."]);
    assert!(ok, "the files type-check: {text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn typescript_server_uris_are_kept_as_the_documents_are() {
    use crate::lang::typescript::canonical_uri;
    assert_eq!(canonical_uri("file:///Users/a/My%20Tests/a.spec.ts"), "file:///Users/a/My%20Tests/a.spec.ts");
    assert_eq!(canonical_uri("untitled:Untitled-1"), "untitled:Untitled-1");
    if cfg!(windows) {
        assert_eq!(canonical_uri("file:///c%3A/Users/a/e2e/a.spec.ts"), "file:///C:/Users/a/e2e/a.spec.ts");
    }
}

// ----- Running ----------------------------------------------------------------------------

pub(crate) mod running {
    use std::collections::BTreeMap;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::write;
    use crate::lang::node;
    use crate::playwright::history::{Cell, History, Record, cell_key};
    use crate::playwright::report::{self, Run, State};
    use crate::playwright::run::{Options, Scope};
    use crate::run::config::{Environment, parse_dotenv};

    fn begin() -> serde_json::Value {
        let t = |id: &str, project: &str, title: &str, line: u32| {
            json!({ "id": id, "project": project, "file": "/p/tests/shop.spec.ts", "line": line, "column": 3,
                    "path": ["shop", title], "tags": [], "expected": "passed" })
        };
        json!({ "e": "begin", "version": "1.64.1", "rootDir": "/p", "workers": 2,
                "projects": [{ "name": "chromium", "browser": "chromium" }, { "name": "firefox", "browser": "firefox" }],
                "tests": [t("a1", "chromium", "signs in", 3), t("a2", "firefox", "signs in", 3),
                          t("b1", "chromium", "pays", 7), t("b2", "firefox", "pays", 7)] })
    }

    #[test]
    fn the_reporters_lines_are_told_from_the_console() {
        assert_eq!(report::split_line("  ✓ signs in"), (Some("  ✓ signs in"), None));
        assert_eq!(report::split_line("\u{1e}snout:{\"e\":\"end\"}"), (None, Some("{\"e\":\"end\"}")));
        assert_eq!(report::split_line("hi\u{1e}snout:{}"), (Some("hi"), Some("{}")));
    }

    #[test]
    fn a_run_follows_its_tests_workers_retries_and_attachments() {
        let mut run = Run::default();
        run.apply(&begin());
        assert_eq!(run.counts().total, 4);
        assert_eq!(run.rows().len(), 2, "a row per test, a cell per project");
        assert_eq!(run.rows()[0].key, "tests/shop.spec.ts › shop › signs in");
        assert_eq!(run.shown_projects(), ["chromium", "firefox"]);
        run.apply(&json!({ "e": "testBegin", "id": "a1", "retry": 0, "worker": 0, "start": 1 }));
        run.apply(&json!({ "e": "step", "id": "a1", "retry": 0, "worker": 0, "title": "page.goto('/')" }));
        assert_eq!(run.busy[&0].step.as_deref(), Some("page.goto('/')"));
        assert_eq!(run.state_of("a1"), State::Running);
        run.apply(&json!({ "e": "out", "id": "a1", "retry": 0, "text": "hi\n" }));
        run.apply(&json!({ "e": "testEnd", "id": "a1", "retry": 0, "worker": 0, "status": "passed",
                           "outcome": "expected", "duration": 1500.4 }));
        assert!(run.busy.is_empty());
        assert_eq!(run.tests["a1"].tries[0].output, "hi\n");
        // A retry that passes: flaky.
        run.apply(&json!({ "e": "testBegin", "id": "a2", "retry": 0, "worker": 1 }));
        run.apply(&json!({ "e": "testEnd", "id": "a2", "retry": 0, "worker": 1, "status": "failed", "outcome": "unexpected" }));
        assert_eq!(run.state_of("a2"), State::Failed);
        run.apply(&json!({ "e": "testBegin", "id": "a2", "retry": 1, "worker": 1 }));
        run.apply(
            &json!({ "e": "testEnd", "id": "a2", "retry": 1, "worker": 1, "status": "passed", "outcome": "flaky" }),
        );
        assert_eq!(run.state_of("a2"), State::Flaky);
        run.apply(&json!({ "e": "testBegin", "id": "b1", "retry": 0, "worker": 0 }));
        run.apply(&json!({ "e": "testEnd", "id": "b1", "retry": 0, "worker": 0, "status": "failed", "outcome": "unexpected",
            "errors": [{ "message": "Error: \u{1b}[31mboom\u{1b}[39m\nmore", "line": 9, "file": "/p/tests/shop.spec.ts" }],
            "attachments": [
                { "name": "home-expected.png", "contentType": "image/png", "path": "/r/home-expected.png" },
                { "name": "home-actual.png", "contentType": "image/png", "path": "/r/home-actual.png" },
                { "name": "home-diff.png", "contentType": "image/png", "path": "/r/home-diff.png" },
                { "name": "trace", "contentType": "application/zip", "path": "/r/trace.zip" },
                { "name": "video", "contentType": "video/webm", "path": "/r/video.webm" }],
            "steps": [{ "title": "click", "category": "pw:api", "duration": 12.5, "error": null, "line": 8, "depth": 0 }] }));
        let b1 = &run.tests["b1"];
        let last = b1.last_try().unwrap();
        assert_eq!(last.errors[0].headline(), "Error: boom");
        assert_eq!(last.trace(), Some(Path::new("/r/trace.zip")));
        assert_eq!(last.video(), Some(Path::new("/r/video.webm")));
        let cmp = &last.comparisons()[0];
        assert_eq!((cmp.name.as_str(), cmp.diff.as_deref()), ("home.png", Some(Path::new("/r/home-diff.png"))));
        assert_eq!(run.location("b1"), Some((PathBuf::from("/p/tests/shop.spec.ts"), 7)));
        // b2 never ran: the run was stopped.
        run.apply(&json!({ "e": "testBegin", "id": "b2", "retry": 0, "worker": 1 }));
        run.finish();
        assert_eq!(run.state_of("b2"), State::Interrupted);
        let c = run.counts();
        assert_eq!((c.passed, c.failed, c.flaky, c.skipped, c.done), (1, 2, 1, 0, 4));
        assert_eq!(run.failures().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["b1", "b2"]);
    }

    #[test]
    fn the_history_keeps_each_tests_results_and_says_what_is_new() {
        let mut run = Run::default();
        run.apply(&begin());
        for (id, status, outcome) in [
            ("a1", "passed", "expected"),
            ("a2", "passed", "flaky"),
            ("b1", "failed", "unexpected"),
            ("b2", "passed", "expected"),
        ] {
            run.apply(&json!({ "e": "testBegin", "id": id, "retry": 0, "worker": 0 }));
            run.apply(&json!({ "e": "testEnd", "id": id, "retry": 0, "worker": 0, "status": status, "outcome": outcome, "duration": 2000 }));
        }
        run.apply(&json!({ "e": "end", "status": "failed", "duration": 9000 }));
        let rec = Record::of(
            &run,
            100,
            "All tests".into(),
            Some("Staging".into()),
            (Some("main".into()), Some("abc1234".into())),
        );
        assert_eq!((rec.passed, rec.failed, rec.flaky, rec.millis), (2, 1, 1, 9000));
        let key = cell_key("chromium", "tests/shop.spec.ts › shop › pays");
        assert_eq!(rec.results[&key], Cell { state: State::Failed, millis: 2000 });
        let text = serde_json::to_string(&rec).unwrap();
        assert!(text.contains("\"f2000\""), "kept short: {text}");
        assert_eq!(serde_json::from_str::<Record>(&text).unwrap(), rec);

        let mut before = rec.clone();
        before.results.insert(key.clone(), Cell { state: State::Passed, millis: 1900 });
        assert_eq!(rec.new_failures(Some(&before)), std::slice::from_ref(&key));
        assert!(rec.new_failures(Some(&rec)).is_empty(), "it failed before too");

        let mut h = History::default();
        for s in [State::Passed, State::Failed, State::Passed, State::Flaky, State::Passed] {
            let mut r = rec.clone();
            r.results.insert(key.clone(), Cell { state: s, millis: 3000 });
            h.add(r);
        }
        assert_eq!(h.last(&key, 3), [State::Passed, State::Flaky, State::Passed]);
        assert!(h.flaky_rate(&key).is_some_and(|r| r >= 20), "{:?}", h.flaky_rate(&key));
        assert_eq!(h.flaky_rate(&cell_key("chromium", "tests/shop.spec.ts › shop › signs in")), None);
        // What is left takes about as long as it took before, spread over the workers.
        let mut live = Run::default();
        live.apply(&begin());
        // chromium's `pays` took 3000 ms last, the others 2000.
        assert_eq!(h.eta(&live), Some((3000 + 3 * 2000) / 2));
        assert_eq!(History::default().eta(&live), None, "no times yet");
        for r in &mut h.runs {
            for t in live.tests.values() {
                r.results.insert(cell_key(&t.project, &t.key()), Cell { state: State::Passed, millis: 3000 });
            }
        }
        assert_eq!(h.eta(&live), Some(4 * 3000 / 2));
    }

    #[test]
    fn environments_give_base_url_over_their_variables_and_file() {
        assert_eq!(
            parse_dotenv("# comment\nexport A=1\nB = \"two words\"\nC='x' \nD=plain # note\n\nbad line"),
            [
                ("A".into(), "1".into()),
                ("B".into(), "two words".into()),
                ("C".into(), "x".into()),
                ("D".into(), "plain".into())
            ]
        );
        let e = Environment {
            name: "Staging".into(),
            base_url: " https://staging.shop.test/ ".into(),
            vars: BTreeMap::from([("A".into(), "over".into())]),
            env_file: Some(".env.staging".into()),
        };
        let vars = e.variables(|f| (f == ".env.staging").then(|| "A=file\nPASSWORD=s3cret\nBASE_URL=nope".into()));
        assert_eq!(
            vars,
            [
                ("A".into(), "over".into()),
                ("BASE_URL".into(), "https://staging.shop.test/".into()),
                ("PASSWORD".into(), "s3cret".into())
            ]
        );
        assert_eq!(e.detail(), "staging.shop.test");
    }

    #[test]
    fn scopes_and_options_become_playwrights_arguments() {
        let root = Path::new("/p");
        assert!(Scope::All.args(root).is_empty());
        assert_eq!(Scope::Project("firefox".into()).args(root), ["--project=firefox"]);
        let at = Scope::At { file: "/p/tests/a.spec.ts".into(), line: Some(12), project: Some("webkit".into()) };
        assert_eq!(at.args(root), ["tests/a.spec.ts:12", "--project=webkit"]);
        assert_eq!(at.title(), "a.spec.ts:12 [webkit]");
        assert_eq!(Scope::LastFailed.args(root), ["--last-failed"]);
        let o = Options { headed: true, update_snapshots: true, repeat: Some(5), ..Default::default() };
        assert_eq!(o.args(), ["--headed", "--workers=1", "--update-snapshots", "--repeat-each=5"]);
        let debug = Options { breakpoints: true, ..Default::default() };
        assert_eq!(debug.args(), ["--headed", "--workers=1", "--timeout=0"]);
        assert_eq!(Options { inspector: true, ..Default::default() }.args(), ["--debug"]);
    }

    /// A `trace.zip` as Playwright 1.64 writes one (format 10), small.
    pub(super) fn write_trace(path: &Path) {
        let frame = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/playwright/frame.jpg")).unwrap();
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("1-trace.trace", opts).unwrap();
        for line in [
            json!({ "type": "context-options", "version": 10, "monotonicTime": 400.0 }),
            json!({ "type": "before", "callId": "c1", "startTime": 410.0, "class": "Frame", "method": "goto", "params": { "url": "https://shop.test/" } }),
            json!({ "type": "after", "callId": "c1", "endTime": 450.0 }),
            json!({ "type": "before", "callId": "c2", "startTime": 460.0, "class": "Frame", "method": "click", "params": { "selector": "internal:role=button" } }),
            json!({ "type": "after", "callId": "c2", "endTime": 470.0, "error": { "error": { "message": "Timeout 5000ms exceeded.\nCall log" } } }),
            json!({ "type": "screencast-frame", "pageId": "p", "file": "screencast/p-1.jpeg", "width": 8, "height": 6, "timestamp": 420.0 }),
            json!({ "type": "screencast-frame", "pageId": "p", "file": "screencast/p-2.jpeg", "width": 8, "height": 6, "timestamp": 465.0 }),
            json!({ "type": "console", "messageType": "error", "text": "Payment widget: order id missing", "time": 466.0 }),
            json!({ "type": "console", "messageType": "log", "text": "fine", "time": 467.0 }),
        ] {
            writeln!(zip, "{line}").unwrap();
        }
        zip.start_file("1-trace.network", opts).unwrap();
        for line in [
            json!({ "type": "resource-snapshot", "snapshot": { "request": { "method": "POST", "url": "https://shop.test/api/orders" }, "response": { "status": 502 }, "time": 1200.0 } }),
            json!({ "type": "resource-snapshot", "snapshot": { "request": { "method": "GET", "url": "https://shop.test/" }, "response": { "status": 200 }, "time": 30.0 } }),
        ] {
            writeln!(zip, "{line}").unwrap();
        }
        for name in ["screencast/p-1.jpeg", "screencast/p-2.jpeg"] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(&frame).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn a_trace_gives_its_frames_actions_failed_requests_and_console_errors() {
        let dir = crate::tests::scratch_dir("pw-trace");
        let path = dir.join("trace.zip");
        write_trace(&path);
        let t = crate::playwright::trace::read(&path).unwrap();
        assert_eq!(
            t.frames.iter().map(|f| (f.entry.as_str(), f.at)).collect::<Vec<_>>(),
            [("screencast/p-1.jpeg", 20.0), ("screencast/p-2.jpeg", 65.0)]
        );
        assert_eq!(t.actions.len(), 2);
        assert_eq!(t.actions[0].title, "frame.goto https://shop.test/");
        assert_eq!(t.actions[1].error.as_deref(), Some("Timeout 5000ms exceeded."));
        assert_eq!(t.failed_requests.len(), 1);
        assert_eq!(
            (t.failed_requests[0].status, t.failed_requests[0].url.as_str()),
            (502, "https://shop.test/api/orders")
        );
        assert_eq!(t.console_errors, ["Payment widget: order id missing"]);
        let bytes = crate::playwright::trace::entry(&path, "screencast/p-2.jpeg").unwrap();
        assert_eq!(&bytes[..2], &[0xff, 0xd8], "a JPEG");
        assert_eq!(t.filmstrip(8).len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A Playwright project whose test runner is `fake-cli.js`: it drives SnoutCode's own
    /// reporter through a small run (see the file).
    pub(crate) fn fake_project(name: &str) -> PathBuf {
        let repo = crate::tests::demo_repo(name);
        let fixtures = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/playwright"));
        write(&repo, "package.json", r#"{ "devDependencies": { "@playwright/test": "1.64.1" } }"#);
        write(&repo, "playwright.config.ts", "export default {};\n");
        write(&repo, "tests/shop.spec.ts", "import { test } from '@playwright/test';\n");
        write(&repo, "node_modules/@playwright/test/package.json", r#"{ "version": "1.64.1" }"#);
        write(&repo, "node_modules/playwright-core/package.json", r#"{ "version": "1.64.1" }"#);
        write(&repo, "node_modules/playwright-core/browsers.json", r#"{ "browsers": [] }"#);
        let pkg = repo.join("node_modules/@playwright/test");
        std::fs::copy(fixtures.join("fake-cli.js"), pkg.join("cli.js")).unwrap();
        for shot in ["home-expected.png", "home-actual.png", "home-diff.png"] {
            std::fs::copy(fixtures.join("shot.png"), pkg.join(shot)).unwrap();
        }
        write(&pkg, "video.webm", "");
        write_trace(&pkg.join("trace.zip"));
        repo
    }

    #[gpui::test]
    fn a_run_shows_live_in_the_tests_window_and_its_failure_opens_its_trace(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::pump_until;
        use crate::tests::{open, with_repo};
        if node::discover(&*snout_host::local()).is_empty() {
            println!("playwright runs: skipped; needs Node");
            return;
        }
        cx.executor().allow_parking();
        let repo = fake_project("pw-run");
        let (_, ws, cx) = open(cx, with_repo(&repo));
        let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
        let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
        let listed = pump_until(cx, 30, |cx| cx.update(|_, cx| tests.read(cx).listing.is_some()));
        assert!(listed, "the tests are listed: {:?}", cx.update(|_, cx| tests.read(cx).listing_error.clone()));
        cx.update(|_, cx| {
            let t = tests.read(cx);
            assert_eq!(t.rows(cx).len(), 2);
            assert_eq!(t.projects(cx), ["chromium", "firefox"]);
            let names: Vec<String> = runs.read(cx).configurations().iter().map(|c| c.name.clone()).collect();
            assert_eq!(names, ["All tests", "Tests in chromium", "Tests in firefox"]);
        });
        assert!(cx.debug_bounds("stripe-tests").is_some(), "the Tests window has its stripe button");
        // The run's environment reaches the tests.
        cx.update(|_, cx| {
            runs.update(cx, |r, cx| {
                let e = Environment {
                    name: "Staging".into(),
                    base_url: "https://staging.shop.test".into(),
                    ..Default::default()
                };
                r.set_environments(vec![e], Some("Staging".into()), cx)
            })
        });

        cx.update(|_, cx| tests.update(cx, |t, cx| t.start(Scope::All, cx)));
        let ended = pump_until(cx, 30, |cx| {
            cx.update(|_, cx| {
                tests.read(cx).live.is_some() && !tests.read(cx).running(cx) && tests.read(cx).history.runs.len() == 1
            })
        });
        assert!(ended, "the run ends and is kept");
        assert!(cx.debug_bounds("tests-grid").is_some(), "the Tests window shows");
        cx.update(|_, cx| {
            let t = tests.read(cx);
            let run = t.run(cx).unwrap();
            let c = run.counts();
            assert_eq!((c.passed, c.failed, c.flaky), (2, 1, 1), "{c:?}");
            assert_eq!(run.environment.as_deref(), Some("Staging"));
            let rec = &t.history.runs[0];
            assert_eq!((rec.environment.as_deref(), rec.scope.as_str()), (Some("Staging"), "All tests"));
            // The console reads as Playwright's list reporter, without the reporter's lines.
            let s = runs.read(cx).session(t.live.unwrap()).unwrap();
            let text = s.console_text();
            assert!(text.contains("✘ [chromium] › pays by card"), "{text}");
            assert!(!text.contains("snout:"), "{text}");
            assert!(s.launch.title() == "All tests");
            if let crate::run::Launch::Process(p) = &s.launch {
                assert!(p.env.iter().any(|(k, v)| k == "BASE_URL" && v == "https://staging.shop.test"));
            }
        });
        // The failure: its error at its line, its screenshots, its steps and its trace.
        cx.update(|_, cx| {
            tests.update(cx, |t, cx| t.select("tests/shop.spec.ts › shop › pays by card".into(), "chromium".into(), cx))
        });
        let traced = pump_until(cx, 10, |cx| cx.debug_bounds("tests-filmstrip").is_some());
        assert!(traced, "the trace is read: its filmstrip shows");
        for id in [
            "tests-detail",
            "tests-accept-screenshot",
            "tests-open-trace",
            "tests-video",
            "tests-copy-error",
            "tests-run-this",
        ] {
            assert!(cx.debug_bounds(id).is_some(), "{id}");
        }
        // A second run: the grid's last results and the History tab have two.
        cx.update(|_, cx| tests.update(cx, |t, cx| t.start(Scope::Project("chromium".into()), cx)));
        let again = pump_until(cx, 30, |cx| {
            cx.update(|_, cx| tests.read(cx).history.runs.len() == 2 && !tests.read(cx).running(cx))
        });
        assert!(again);
        cx.update(|_, cx| {
            let t = tests.read(cx);
            let key = cell_key("chromium", "tests/shop.spec.ts › shop › pays by card");
            assert_eq!(t.history.last(&key, 10), [State::Failed, State::Failed]);
            assert_eq!(t.rows(cx).len(), 2, "a run of one project keeps the others' rows");
            assert!(t.history.runs[1].new_failures(t.history.previous()).is_empty(), "nothing new broke");
        });
        cx.update(|_, cx| tests.update(cx, |t, cx| t.set_tab(crate::playwright::tests::Tab::History, cx)));
        cx.run_until_parked();
        assert!(cx.debug_bounds("tests-history").is_some());
        let _ = std::fs::remove_dir_all(repo);
    }

    /// A fake Playwright project debugged under the fake js-debug, stopped at the breakpoint
    /// on line 4 of its spec: the project, the server's folder, the runs, the session.
    fn debug_paused<'a>(
        name: &str,
        cx: &'a mut gpui::TestAppContext,
    ) -> Option<(PathBuf, PathBuf, gpui::Entity<crate::run::Runs>, u64, &'a mut gpui::VisualTestContext)> {
        use crate::lang_tests::pump_until;
        use crate::run::Target;
        use crate::tests::{open, with_repo};
        if node::discover(&*snout_host::local()).is_empty() {
            println!("playwright debugging: skipped; needs Node");
            return None;
        }
        cx.executor().allow_parking();
        let repo = fake_project(name);
        let spec = "import { test } from '@playwright/test';\n\ntest('buys', async ({ page }) => {\n  const total = 2 * 3;\n  await page.goto('/');\n});\n";
        write(&repo, "tests/shop.spec.ts", spec);
        let server = crate::tests::scratch_dir(&format!("{name}-js-debug"));
        let fixtures = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/playwright"));
        std::fs::copy(fixtures.join("fake-js-debug.js"), server.join("dapDebugServer.js")).unwrap();
        let (_, ws, cx) = open(cx, with_repo(&repo));
        let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
        let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
        let listed = pump_until(cx, 30, |cx| cx.update(|_, cx| tests.read(cx).listing.is_some()));
        assert!(listed, "the tests are listed");
        let file = repo.join("tests/shop.spec.ts");
        cx.update(|_, cx| {
            runs.update(cx, |r, cx| {
                r.js_debug_server = Some(server.join("dapDebugServer.js"));
                r.toggle_breakpoint(&file, 3, cx);
                let target = Target::Playwright { file: file.clone(), line: 3, title: "buys".into(), group: false };
                r.run_target(&target, None, true, cx);
            })
        });
        let paused = pump_until(cx, 30, |cx| {
            cx.update(|_, cx| {
                let r = runs.read(cx);
                r.paused_debug().is_some_and(|id| !r.session(id).unwrap().debug.as_ref().unwrap().locals().is_empty())
            })
        });
        let log = std::fs::read_to_string(server.join("js-debug.log")).unwrap_or_default();
        let console = cx.update(|_, cx| runs.read(cx).sessions.last().map(|s| s.console_text()).unwrap_or_default());
        assert!(paused, "the worker stops at the breakpoint: {log}\n{console}");
        let id = cx.update(|_, cx| runs.read(cx).paused_debug().unwrap());
        Some((repo, server, runs, id, cx))
    }

    #[gpui::test]
    fn debugging_a_test_stops_at_its_breakpoint_in_the_worker_js_debug_starts(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::pump_until;
        use crate::run::Place;
        let Some((repo, server, runs, id, cx)) = debug_paused("pw-debug", cx) else { return };
        let file = repo.join("tests/shop.spec.ts");
        let log = || std::fs::read_to_string(server.join("js-debug.log")).unwrap_or_default();
        cx.update(|_, cx| {
            let r = runs.read(cx);
            let s = r.session(id).unwrap();
            assert_eq!(s.place(), Place::Debug);
            assert!(s.playwright.is_some(), "the Tests window follows it too");
            let d = s.debug.as_ref().unwrap();
            assert_eq!(d.targets.len(), 2, "the launch's connection and the worker's");
            assert!(d.is_current(&d.targets[1]), "the worker's stopped");
            assert_eq!(d.exec_point(), Some((file.clone(), 3)));
            assert_eq!(d.locals(), [("total".to_owned(), "6".to_owned())]);
        });
        // The program got js-debug's variables, and Playwright's debugging arguments.
        let started: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo.join("node_modules/@playwright/test/debugged.json")).unwrap(),
        )
        .unwrap();
        let bootloader = server.join("bootloader.js");
        assert_eq!(started["NODE_OPTIONS"], format!("--require {:?}", bootloader.display().to_string()));
        let args: Vec<&str> = started["args"].as_array().unwrap().iter().filter_map(|a| a.as_str()).collect();
        assert!(args.contains(&"--timeout=0") && args.contains(&"--workers=1"), "{args:?}");
        assert!(args.contains(&"tests/shop.spec.ts:3"), "{args:?}");
        let said = log();
        assert!(said.contains(r#""role":"worker","type":"request","command":"setBreakpoints""#), "{said}");
        assert!(said.contains(r#""__pendingTargetId":"w1""#), "the worker's session launched as asked: {said}");

        // Going on: the worker's session ends, the debugger lets go of it, the run finishes.
        cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
        let ended =
            pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
        assert!(ended, "the run ends once the debugger lets go: {}", log());
        cx.update(|_, cx| {
            let r = runs.read(cx);
            let s = r.session(id).unwrap();
            let d = s.debug.as_ref().unwrap();
            assert!(d.ended && !d.live() && d.server.is_none());
            assert!(s.playwright.as_ref().unwrap().counts().passed > 0);
        });
        assert!(log().contains(r#""role":"worker","type":"request","command":"disconnect""#));
        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(server);
    }

    #[gpui::test]
    fn stopping_a_paused_test_ends_the_program_and_its_debugger(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::pump_until;
        let Some((repo, server, runs, id, cx)) = debug_paused("pw-debug-stop", cx) else { return };
        cx.update(|_, cx| runs.update(cx, |r, cx| r.stop(id, cx)));
        let ended =
            pump_until(cx, 30, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
        assert!(ended, "Stop ends it");
        cx.update(|_, cx| {
            let r = runs.read(cx);
            let s = r.session(id).unwrap();
            assert_eq!(s.ended.as_ref().unwrap().outcome, crate::run::Outcome::Cancelled);
            let d = s.debug.as_ref().unwrap();
            assert!(d.ended && !d.paused && d.targets.is_empty());
            assert!(r.paused_debug().is_none());
        });
        let log = std::fs::read_to_string(server.join("js-debug.log")).unwrap_or_default();
        assert!(log.contains(r#""command":"disconnect","arguments":{"terminateDebuggee":true}"#), "{log}");
        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(server);
    }

    #[gpui::test]
    fn spec_files_get_run_icons_that_run_their_test(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::{editor_of, pump_until};
        use crate::model::Action;
        use crate::run::Target;
        use crate::tests::{act, open, with_repo};
        cx.executor().allow_parking();
        let repo = crate::tests::demo_repo("pw-gutter");
        write(&repo, "playwright.config.ts", "export default {};\n");
        write(&repo, "package.json", r#"{ "devDependencies": { "@playwright/test": "1.64.1" } }"#);
        let spec = repo.join("tests/login.spec.ts");
        write(
            &repo,
            "tests/login.spec.ts",
            "import { test } from '@playwright/test';\n\ntest('signs in', async ({ page }) => {});\n",
        );
        let (model, ws, cx) = open(cx, with_repo(&repo));
        act(&model, cx, Action::OpenFile(spec.clone(), None));
        let editor = editor_of(&ws, &spec, cx).expect("the spec opens");
        let found = pump_until(cx, 10, |cx| cx.update(|_, cx| editor.read(cx).runnables.is_some()));
        assert!(found);
        let r = cx.update(|_, cx| editor.read(cx).runnables.clone()).unwrap();
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].line, 2);
        assert!(
            matches!(&r.items[0].target, Target::Playwright { line: 3, title, group: false, .. } if title == "signs in")
        );
        assert_eq!(r.at_line(2), Some(&r.items[0].target), "⌃⇧R runs the test the caret is in");
        let _ = std::fs::remove_dir_all(repo);
    }
}

/// A real debug of a spec under js-debug (downloaded as the app does, unless
/// `SNOUT_JS_DEBUG` names a server script): `SNOUT_PW_DEBUG_AT` is a breakpoint of an
/// installed Playwright project, as `<project>/<spec>:<line from 1>`. Run by hand with
/// `--ignored`.
#[gpui::test]
#[ignore]
fn a_real_debug_stops_at_a_breakpoint_in_a_spec(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::tests::{open, with_repo};
    let Some(at) = std::env::var("SNOUT_PW_DEBUG_AT").ok() else {
        println!("set SNOUT_PW_DEBUG_AT to <project>/<spec>:<line>");
        return;
    };
    let (file, line) = at.rsplit_once(':').unwrap();
    let (file, line) = (PathBuf::from(file), line.parse::<u32>().unwrap());
    let dir = file.ancestors().find(|d| d.join("playwright.config.ts").is_file()).unwrap().to_owned();
    cx.executor().allow_parking();
    let (_, ws, cx) = open(cx, with_repo(&dir));
    let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
    let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
    let listed = pump_until(cx, 60, |cx| cx.update(|_, cx| tests.read(cx).listing.is_some()));
    assert!(listed, "{:?}", cx.update(|_, cx| tests.read(cx).listing_error.clone()));
    cx.update(|_, cx| {
        runs.update(cx, |r, cx| {
            r.js_debug_server = std::env::var_os("SNOUT_JS_DEBUG").map(PathBuf::from);
            r.toggle_breakpoint(&file, line - 1, cx);
            let scope = playwright::run::Scope::At { file: file.clone(), line: None, project: None };
            let options = playwright::run::Options { breakpoints: true, ..Default::default() };
            r.start_playwright(scope, options, cx);
        })
    });
    let console = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, cx| runs.read(cx).sessions.last().map(|s| s.console_text()).unwrap_or_default())
    };
    let started = Instant::now();
    let paused = pump_until(cx, 180, |cx| {
        cx.update(|_, cx| {
            runs.read(cx).paused_debug().is_some_and(|id| {
                let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap();
                !d.frames.is_empty() && !d.locals().is_empty()
            })
        })
    });
    assert!(paused, "it stops at the breakpoint:\n{}", console(cx));
    let id = cx.update(|_, cx| runs.read(cx).paused_debug().unwrap());
    cx.update(|_, cx| {
        let d = runs.read(cx).session(id).unwrap().debug.as_ref().unwrap();
        println!(
            "paused after {:?} in {:?}, {} connections",
            started.elapsed(),
            d.frames[d.frame].name,
            d.targets.len()
        );
        println!("  locals: {:?}", d.locals());
        assert_eq!(d.exec_point(), Some((file.clone(), line - 1)));
    });
    cx.update(|_, cx| runs.update(cx, |r, cx| r.remove_breakpoint(&file, line - 1, cx)));
    cx.update(|_, cx| runs.update(cx, |r, cx| r.step(id, "continue", cx)));
    let ended =
        pump_until(cx, 60, |cx| cx.update(|_, cx| runs.read(cx).session(id).is_some_and(|s| s.ended.is_some())));
    assert!(ended, "the run ends:\n{}", console(cx));
    cx.update(|_, cx| {
        let s = runs.read(cx).session(id).unwrap();
        let c = s.playwright.as_ref().unwrap().counts();
        println!("ended: {c:?}, {:?}", s.ended.as_ref().map(|e| e.outcome));
        assert!(c.passed > 0, "{}", s.console_text());
    });
}

/// A real Playwright run through the Tests window, against the installed project at
/// `SNOUT_PW_PROJECT` (its tests and browsers as they are): run by hand with `--ignored`.
#[gpui::test]
#[ignore]
fn a_real_playwright_run_feeds_the_tests_window(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::tests::{open, with_repo};
    let Some(dir) = std::env::var_os("SNOUT_PW_PROJECT").map(PathBuf::from) else {
        println!("set SNOUT_PW_PROJECT to an installed Playwright project");
        return;
    };
    cx.executor().allow_parking();
    let (_, ws, cx) = open(cx, with_repo(&dir));
    let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
    let listed = pump_until(cx, 60, |cx| cx.update(|_, cx| tests.read(cx).listing.is_some()));
    assert!(listed, "{:?}", cx.update(|_, cx| tests.read(cx).listing_error.clone()));
    let total = cx.update(|_, cx| tests.read(cx).listing.as_ref().unwrap().tests.len());
    cx.update(|_, cx| tests.update(cx, |t, cx| t.start(playwright::run::Scope::All, cx)));
    let ended = pump_until(cx, 300, |cx| {
        cx.update(|_, cx| {
            tests.read(cx).live.is_some() && !tests.read(cx).running(cx) && !tests.read(cx).history.runs.is_empty()
        })
    });
    assert!(ended, "the run ends");
    cx.update(|_, cx| {
        let t = tests.read(cx);
        let run = t.run(cx).unwrap();
        let c = run.counts();
        println!("real run: {c:?} of {total} listed, Playwright {:?}", run.version);
        assert_eq!(c.total, total);
        assert_eq!(c.done, total, "every test has an outcome");
        for f in run.failures() {
            let last = f.last_try().unwrap();
            println!(
                "  failed: {} [{}]: {}",
                f.key(),
                f.project,
                last.errors.first().map(|e| e.headline()).unwrap_or_default()
            );
            println!("    attachments: {:?}", last.attachments.iter().map(|a| a.name.as_str()).collect::<Vec<_>>());
            if let Some(trace) = last.trace() {
                let tr = crate::playwright::trace::read(trace).unwrap();
                println!(
                    "    trace: {} frames, {} actions, console errors {:?}",
                    tr.frames.len(),
                    tr.actions.len(),
                    tr.console_errors
                );
            }
        }
    });
}

/// Phase 3: writing tests. Record at Cursor, Pick Locator and UI Mode.
mod writing {
    use std::path::{Path, PathBuf};

    use super::write;
    use crate::lang::node;
    use crate::playwright::gutter::Spot;
    use crate::playwright::record::{self, Start};
    use crate::playwright::report::Project;
    use crate::run::config::Environment;

    const GENERATED: &str = "import { test, expect } from '@playwright/test';\n\ntest.use({\n  storageState: '.auth/user.json'\n});\n\ntest('test', async ({ page }) => {\n  await page.goto('https://shop.test/cart');\n  await page.getByRole('button', { name: 'Pay' }).click();\n\n  await expect(page.getByText('Paid')).toBeVisible();\n});";

    #[test]
    fn the_recorders_file_gives_its_steps_and_a_half_written_one_none() {
        let steps = record::steps(GENERATED).unwrap();
        assert_eq!(
            steps,
            [
                "await page.goto('https://shop.test/cart');",
                "await page.getByRole('button', { name: 'Pay' }).click();",
                "",
                "await expect(page.getByText('Paid')).toBeVisible();",
            ]
        );
        assert_eq!(record::count(&steps), 3);
        assert_eq!(record::steps(&GENERATED[..GENERATED.len() - 3]), None, "not whole yet");
        assert_eq!(record::steps(""), None);
    }

    #[test]
    fn a_goto_to_the_base_url_is_made_relative_only_for_a_bare_origin() {
        let goto = "await page.goto('https://shop.test/cart');";
        assert_eq!(record::relative(goto, Some("https://shop.test")), "await page.goto('/cart');");
        assert_eq!(record::relative(goto, Some("https://shop.test/")), "await page.goto('/cart');");
        assert_eq!(
            record::relative("await page.goto('https://shop.test');", Some("https://shop.test")),
            "await page.goto('/');"
        );
        // `/cart` against `…/app` would drop `/app`.
        assert_eq!(record::relative(goto, Some("https://shop.test/app")), goto);
        assert_eq!(record::relative(goto, Some("https://other.test")), goto);
        assert_eq!(record::relative(goto, None), goto);
    }

    #[test]
    fn steps_go_into_a_test_into_a_new_one_or_make_the_whole_spec() {
        let steps = record::steps(GENERATED).unwrap();
        let base = Some("https://shop.test");
        let body = record::render(&Spot::Body { indent: "    ".into() }, &steps, GENERATED, "  ", base);
        assert_eq!(
            body,
            "    await page.goto('/cart');\n    await page.getByRole('button', { name: 'Pay' }).click();\n\n    await expect(page.getByText('Paid')).toBeVisible();"
        );
        let new = record::render(&Spot::Outside { indent: "  ".into() }, &steps[..1], GENERATED, "  ", base);
        assert_eq!(new, "  test('new test', async ({ page }) => {\n    await page.goto('/cart');\n  });");
        let whole = record::render(&Spot::Empty, &steps, GENERATED, "  ", base);
        assert!(whole.starts_with("import { test, expect } from '@playwright/test';\n"), "{whole}");
        assert!(whole.contains("  await page.goto('/cart');\n") && whole.ends_with("});\n"), "{whole}");
    }

    #[test]
    fn a_recording_takes_a_blank_line_or_goes_after_the_carets_line() {
        let text = "test('a', async ({ page }) => {\n  await page.goto('/');\n  \n});\n";
        let blank = text.find("  \n}").unwrap() + 1;
        let body = Spot::Body { indent: "  ".into() };
        assert_eq!(record::place(text, blank, &body), (blank - 1..blank + 1, ""));
        let goto = text.find("goto").unwrap();
        let end = text.find(";\n").unwrap() + 1;
        assert_eq!(record::place(text, goto, &body), (end..end, "\n"));
        let outside = Spot::Outside { indent: String::new() };
        assert_eq!(record::place(text, 1, &outside), (31..31, "\n\n"));
        assert_eq!(record::place("\n\n", 1, &Spot::Empty), (0..2, ""));
    }

    #[test]
    fn a_pick_goes_on_page_unless_the_caret_follows_a_member_access() {
        let l = "getByRole('button', { name: 'Log in' })";
        assert_eq!(record::picked_text("  await ", l), format!("page.{l}"));
        assert_eq!(record::picked_text("  await expect(", l), format!("page.{l}"));
        assert_eq!(record::picked_text("  await dialog.", l), l);
    }

    #[test]
    fn a_recording_starts_where_the_tests_do_as_whom_they_log_in() {
        let project = |name: &str, browser: &str, storage: &str| Project {
            name: name.into(),
            browser: browser.into(),
            base_url: "https://shop.test".into(),
            storage_state: storage.into(),
            ..Default::default()
        };
        let projects = [
            project("setup", "", ""),
            project("firefox", "firefox", ".auth/user.json"),
            Project { test_id_attribute: "data-qa".into(), ..project("chromium", "chromium", "") },
        ];
        let root = Path::new("/qa");
        let start = Start::of(&projects, None, root);
        assert_eq!(start.url.as_deref(), Some("https://shop.test"));
        assert_eq!(start.browser.as_deref(), Some("firefox"), "the project that logs in");
        assert_eq!(start.storage, Some(PathBuf::from("/qa/.auth/user.json")));
        // The environment picked comes before the configuration.
        let staging =
            Environment { name: "Staging".into(), base_url: "https://staging.shop.test".into(), ..Default::default() };
        assert_eq!(Start::of(&projects, Some(&staging), root).url.as_deref(), Some("https://staging.shop.test"));
        let chromium = Start::of(&projects[2..], None, root);
        let args = chromium.codegen_args(Path::new("/tmp/r.ts"), false);
        assert_eq!(
            args,
            [
                "codegen",
                "--target=playwright-test",
                "--output=/tmp/r.ts",
                "--browser=chromium",
                "--test-id-attribute=data-qa",
                "https://shop.test"
            ]
        );
        let logged_in = start.codegen_args(Path::new("/tmp/r.ts"), true);
        let load = format!("--load-storage={}", root.join(".auth/user.json").display());
        assert!(logged_in.contains(&load), "{logged_in:?}");
        assert!(!start.codegen_args(Path::new("/tmp/r.ts"), false).iter().any(|a| a.starts_with("--load-storage")));
        // Nothing listed and no environment: the recorder opens empty.
        assert_eq!(Start::of(&[], None, root).codegen_args(Path::new("/r.ts"), false).len(), 3);
    }

    const SPEC: &str = "import { test, expect } from '@playwright/test';\n\ntest.describe('shop', () => {\n    test('buys', async ({ page }) => {\n        await page.goto('/');\n        \n    });\n});\n";

    /// A Playwright project of the fakes, its spec open, the caret on `find` (plus
    /// `offset`), the tests listed.
    fn spec_open<'a>(
        name: &str,
        find: &str,
        offset: usize,
        cx: &'a mut gpui::TestAppContext,
    ) -> Option<(
        PathBuf,
        gpui::Entity<crate::workspace::Workspace>,
        gpui::Entity<crate::editor::EditorPanel>,
        &'a mut gpui::VisualTestContext,
    )> {
        use crate::lang_tests::{caret_at, pump_until};
        use crate::tests::{open, with_repo};
        if node::discover(&*snout_host::local()).is_empty() {
            println!("writing tests: skipped; needs Node");
            return None;
        }
        cx.executor().allow_parking();
        let repo = super::running::fake_project(name);
        write(&repo, "tests/shop.spec.ts", SPEC);
        let fixtures = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/playwright"));
        std::fs::copy(fixtures.join("fake-api.js"), repo.join("node_modules/@playwright/test/index.js")).unwrap();
        let (_, ws, cx) = open(cx, with_repo(&repo));
        cx.update(|_, cx| cx.set_global(crate::lang_tests::TestWorkspace(ws.clone())));
        let tests = cx.update(|_, cx| ws.read(cx).pw_tests.clone());
        let listed = pump_until(cx, 30, |cx| cx.update(|_, cx| tests.read(cx).listing.is_some()));
        assert!(listed, "the tests are listed: {:?}", cx.update(|_, cx| tests.read(cx).listing_error.clone()));
        let path = repo.join("tests/shop.spec.ts");
        cx.update(|w, cx| ws.update(cx, |ws, cx| ws.open_file(path.clone(), None, false, w, cx)));
        let opened = pump_until(cx, 10, |cx| {
            cx.update(|_, cx| ws.read(cx).editors(cx).iter().any(|e| e.read(cx).path == path && !e.read(cx).loading))
        });
        assert!(opened, "the spec opens");
        let editor = crate::lang_tests::editor_of(&ws, &path, cx).unwrap();
        caret_at(&editor, find, 0, offset, cx);
        Some((repo, ws, editor, cx))
    }

    fn message(ws: &gpui::Entity<crate::workspace::Workspace>, cx: &mut gpui::VisualTestContext) -> String {
        cx.update(|_, cx| ws.read(cx).model.read(cx).message.clone())
    }

    #[gpui::test]
    fn record_at_cursor_writes_the_recorders_steps_into_the_test_live(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::{pump_until, text};
        // The caret on the blank line of the test.
        let Some((repo, ws, editor, cx)) = spec_open("pw-record", "        \n    });", 8, cx) else { return };
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::RecordAtCursor), cx));
        let first =
            pump_until(cx, 20, |cx| text(&editor, cx).contains("await page.goto('/');\n        await page.goto('/');"));
        assert!(first, "the first step is written as it comes:\n{}", text(&editor, cx));
        assert!(cx.debug_bounds("record-bar").is_some(), "the tab says it is recording");
        let all = pump_until(cx, 20, |cx| text(&editor, cx).contains("fill('qa@shop.test')"));
        assert!(all, "the rest follow:\n{}", text(&editor, cx));
        assert_eq!(
            text(&editor, cx),
            "import { test, expect } from '@playwright/test';\n\ntest.describe('shop', () => {\n    test('buys', async ({ page }) => {\n        await page.goto('/');\n        await page.goto('/');\n        await page.getByRole('link', { name: 'Shop' }).click();\n        await page.getByLabel('Email').fill('qa@shop.test');\n    });\n});\n",
            "in place of the blank line, at the test's indentation, the base URL made relative"
        );
        // The recorder got the configuration's browser, URL and test id attribute; no saved
        // session yet (the setup has not run).
        let args_file =
            std::fs::read_dir(crate::playwright::record::recording_file(snout_host::local().info()).parent().unwrap())
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.to_string_lossy().ends_with(".args.json"))
                .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                .unwrap();
        let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&args_file).unwrap()).unwrap();
        let _ = std::fs::remove_file(&args_file);
        for want in ["--browser=chromium", "--test-id-attribute=data-qa", "https://shop.test"] {
            assert!(args.iter().any(|a| a == want), "{want} in {args:?}");
        }
        assert!(!args.iter().any(|a| a.starts_with("--load-storage")), "{args:?}");
        // Stop ends it and says how it went.
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::StopRecording), cx));
        let ended = pump_until(cx, 20, |cx| cx.debug_bounds("record-bar").is_none());
        assert!(ended, "the bar goes when it ends");
        assert_eq!(message(&ws, cx), "Recorded 3 steps into shop.spec.ts");
        let _ = repo;
    }

    #[gpui::test]
    fn recording_between_tests_writes_a_new_test(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::{pump_until, text};
        // The caret at the end of the describe's last line: after the test.
        let Some((_, _, editor, cx)) = spec_open("pw-record-new", "    });\n});", 7, cx) else { return };
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::RecordAtCursor), cx));
        let all = pump_until(cx, 20, |cx| text(&editor, cx).contains("fill('qa@shop.test')"));
        assert!(all, "{}", text(&editor, cx));
        assert!(
            text(&editor, cx).contains("    });\n\n    test('new test', async ({ page }) => {\n        await page.goto('/');\n        await page.getByRole('link', { name: 'Shop' }).click();\n        await page.getByLabel('Email').fill('qa@shop.test');\n    });\n});\n"),
            "{}",
            text(&editor, cx)
        );
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::StopRecording), cx));
        assert!(pump_until(cx, 20, |cx| cx.debug_bounds("record-bar").is_none()));
    }

    #[gpui::test]
    fn pick_locator_puts_the_locator_at_the_caret_and_a_new_pick_replaces_it(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::{pump_until, text};
        let Some((repo, ws, editor, cx)) = spec_open("pw-pick", "await page.goto('/');", 6, cx) else { return };
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::PickLocator), cx));
        let first = pump_until(cx, 20, |cx| {
            text(&editor, cx).contains("await page.getByRole('button', { name: 'Log in' })page.goto('/')")
        });
        assert!(first, "{}", text(&editor, cx));
        assert!(cx.debug_bounds("record-bar").is_some(), "the tab says it is picking");
        let second =
            pump_until(cx, 20, |cx| text(&editor, cx).contains("await page.getByTestId('cart')page.goto('/')"));
        assert!(second, "the next pick takes the last one's place: {}", text(&editor, cx));
        assert_eq!(message(&ws, cx), "Picked getByTestId('cart')");
        // The browser opened where the tests start, with their test id attribute.
        let opened: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(repo.join("picker-opened.json")).unwrap()).unwrap();
        assert_eq!(opened["url"], "https://shop.test");
        assert_eq!(opened["browser"], "chromium");
        assert_eq!(opened["testIdAttribute"], "data-qa");
        assert_eq!(opened["headless"], false);
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::StopRecording), cx));
        assert!(pump_until(cx, 20, |cx| cx.debug_bounds("record-bar").is_none()), "Stop ends the pick");
    }

    #[gpui::test]
    fn ui_mode_serves_playwrights_runner_to_the_browser(cx: &mut gpui::TestAppContext) {
        use crate::lang_tests::pump_until;
        let Some((_, ws, _, cx)) = spec_open("pw-ui", "test(", 0, cx) else { return };
        cx.update(|w, cx| w.dispatch_action(Box::new(crate::menus::OpenUiMode), cx));
        let runs = cx.update(|_, cx| ws.read(cx).runs.clone());
        let started = pump_until(cx, 20, |cx| {
            cx.update(|_, cx| runs.read(cx).sessions.iter().any(|s| s.launch.title() == "UI Mode" && s.running()))
        });
        assert!(started, "UI Mode runs");
        cx.update(|_, cx| {
            let r = runs.read(cx);
            let s = r.sessions.iter().find(|s| s.launch.title() == "UI Mode").unwrap();
            let crate::run::Launch::Process(p) = &s.launch else { panic!("a process") };
            assert!(p.args.ends_with(&["test".into(), "--ui".into(), "--ui-port=0".into()]), "{:?}", p.args);
        });
        assert_eq!(message(&ws, cx), "UI Mode opens in your browser");
        cx.update(|_, cx| {
            runs.update(cx, |r, cx| {
                let ids: Vec<u64> = r.sessions.iter().filter(|s| s.running()).map(|s| s.id).collect();
                for id in ids {
                    r.stop(id, cx);
                }
            })
        });
        assert!(pump_until(cx, 20, |cx| cx.update(|_, cx| !runs.read(cx).sessions.iter().any(|s| s.running()))));
    }
}

#[gpui::test]
fn the_title_bar_keeps_run_and_search_with_an_environment_in_a_small_window(cx: &mut gpui::TestAppContext) {
    use crate::lang_tests::pump_until;
    use crate::tests::{open, with_repo};
    cx.executor().allow_parking();
    let repo = crate::tests::demo_repo("pw-title");
    write(&repo, "package.json", r#"{ "devDependencies": { "@playwright/test": "1.64.1" } }"#);
    write(&repo, "playwright.config.ts", "export default {};\n");
    let (_, _ws, cx) = open(cx, with_repo(&repo));
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(500.)));
    let shown = pump_until(cx, 10, |cx| cx.debug_bounds("title-environment").is_some());
    assert!(shown, "the project has its environment switch");
    for id in ["title-run", "title-search", "title-settings"] {
        let b = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
        assert!(b.right() <= gpui::px(800.), "{id} is on the window: {b:?}");
    }
}
