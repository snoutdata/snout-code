//! Playwright projects and Node: how a project is recognised, what it asks of Node, and
//! which install is chosen.

use std::path::{Path, PathBuf};

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
