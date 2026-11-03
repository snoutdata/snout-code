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
