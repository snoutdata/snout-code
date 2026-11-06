//! Making a downloaded version ready. On a Mac the new app is unpacked beside the live one (a
//! hidden sibling, so the swap is a same-volume rename), its code signature is checked against the
//! running app's own designated requirement, and it is started once to prove it runs and is the
//! version the feed named. On Windows the verified installer is what is ready.
//!
//! A staged version is remembered in `staged.json`, in the updates folder: nothing can be written
//! into a signed bundle.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::failure::{Failure, describe};

/// Set on a staged build: it writes its version to the file this names, and exits.
pub const SELF_CHECK_ENV: &str = "SNOUT_UPDATE_SELF_CHECK";
const SELF_CHECK_TIMEOUT: Duration = Duration::from_secs(90);
/// The stock app's name inside the release's zip.
const APP_IN_ZIP: &str = "SnoutCode.app";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundlePaths {
    pub install: PathBuf,
    pub next: PathBuf,
    pub old: PathBuf,
}

/// `<folder>/SnoutCode.app`, `<folder>/.SnoutCode.next.app` and `<folder>/.SnoutCode.old.app`:
/// siblings, so both moves are renames. Bundles still, as `codesign` needs them to be.
pub fn bundle_paths(app: &Path) -> BundlePaths {
    let stem = app.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "SnoutCode".into());
    let sibling = |what: &str| app.with_file_name(format!(".{stem}.{what}.app"));
    BundlePaths { install: app.to_owned(), next: sibling("next"), old: sibling("old") }
}

/// Where a release's archive unpacks: beside the app, on its volume.
fn unpack_dir(app: &Path) -> PathBuf {
    app.with_file_name(format!(".{}.unpack", app.file_stem().unwrap_or_default().to_string_lossy()))
}

/// The program inside a bundle.
pub fn bundle_exe(app: &Path) -> PathBuf {
    app.join("Contents/MacOS/snout-code")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staged {
    pub version: String,
    /// What the swap uses: the staged bundle, or the installer.
    pub path: PathBuf,
}

fn staged_file(updates: &Path) -> PathBuf {
    updates.join("staged.json")
}

pub fn read_staged(updates: &Path) -> Option<Staged> {
    serde_json::from_slice(&std::fs::read(staged_file(updates)).ok()?).ok()
}

pub fn write_staged(updates: &Path, staged: &Staged) -> std::io::Result<()> {
    std::fs::create_dir_all(updates)?;
    std::fs::write(staged_file(updates), serde_json::to_vec(staged)?)
}

pub fn forget_staged(updates: &Path) {
    let _ = std::fs::remove_file(staged_file(updates));
}

/// How strict the code signature check is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeCheck {
    /// The new app must satisfy the running app's designated requirement, as a release's must.
    Requirement,
    /// Development: a build signed ad hoc has no requirement another build could meet.
    IntactOnly,
}

/// Unpack `zip` beside the live app at `app` and prove it: whole, ours, and `version`.
pub fn stage_bundle(
    zip: &Path,
    app: &Path,
    version: &str,
    check: CodeCheck,
    log: &dyn Fn(&str),
) -> Result<PathBuf, Failure> {
    let paths = bundle_paths(app);
    let unpacked = unpack_dir(app);
    let _ = std::fs::remove_dir_all(&paths.next);
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked).map_err(io_failure)?;
    let result = (|| {
        log(&format!("staging v{version} beside {}", app.display()));
        run(crate::command("ditto").args(["-x", "-k"]).arg(zip).arg(&unpacked))
            .map_err(|e| describe(&format!("the release's archive did not unpack: {e}"), None))?;
        std::fs::rename(unpacked.join(APP_IN_ZIP), &paths.next).map_err(io_failure)?;
        verify_bundle(&paths.next, app, version, check, log)
    })();
    let _ = std::fs::remove_dir_all(&unpacked);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&paths.next);
    }
    result.map(|()| paths.next)
}

/// A stage made earlier, or just now: it may have been damaged since.
pub fn verify_bundle(
    next: &Path,
    live: &Path,
    version: &str,
    check: CodeCheck,
    log: &dyn Fn(&str),
) -> Result<(), Failure> {
    verify_code(next, live, check)?;
    log("the staged app's code signature is good");
    let found = self_check(&bundle_exe(next))?;
    if found != version {
        return Err(describe(&format!("the signature is good but the staged app is v{found}, not v{version}"), None));
    }
    log("the staged app passed its self-check");
    Ok(())
}

fn verify_code(next: &Path, live: &Path, check: CodeCheck) -> Result<(), Failure> {
    let mut verify = crate::command("codesign");
    verify.args(["--verify", "--deep", "--strict"]);
    if check == CodeCheck::Requirement {
        let out = crate::command("codesign")
            .args(["-d", "-r-"])
            .arg(live)
            .output()
            .map_err(|e| describe(&format!("codesign did not start: {e}"), None))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let requirement = designated_requirement(&text)
            .ok_or_else(|| describe(&format!("the running app is not signed: {}", text.trim()), None))?;
        verify.arg(format!("-R={requirement}"));
    }
    run(verify.arg(next)).map_err(|e| describe(&format!("the new app's signature is not ours: {e}"), None))
}

/// What `codesign -d -r-` prints as the designated requirement, when it names a signer: one made
/// of a bare hash (signed ad hoc) only ever matches the very same build.
fn designated_requirement(text: &str) -> Option<String> {
    let line = text.lines().find_map(|l| l.trim().strip_prefix("designated => "))?;
    (!line.trim_start().starts_with("cdhash")).then(|| line.trim().to_owned())
}

/// Start `exe` as a self-check: the version it says it is. A build that cannot start says nothing.
pub fn self_check(exe: &Path) -> Result<String, Failure> {
    let answer = tempfile_in_temp();
    let mut child = crate::command(exe)
        .env(SELF_CHECK_ENV, &answer)
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| describe(&format!("the staged app did not start: {e}"), None))?;
    let started = Instant::now();
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if started.elapsed() > SELF_CHECK_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(describe("the staged app did not answer its self-check", None));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let said = std::fs::read_to_string(&answer).unwrap_or_default().trim().to_owned();
    let _ = std::fs::remove_file(&answer);
    if status.success() && !said.is_empty() {
        Ok(said)
    } else {
        Err(describe(&format!("the staged app failed its self-check ({status})"), None))
    }
}

fn tempfile_in_temp() -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    std::env::temp_dir().join(format!("snout-self-check-{}-{nanos}", std::process::id()))
}

/// What a launch tidies, off the boot path: what the last swap left, and a stage or download that
/// is not newer than this build (its update arrived some other way). A stage of a newer version is
/// kept for reuse.
pub fn cleanup(updates: &Path, app: Option<&Path>, current: &str) {
    if let Some(app) = app {
        let paths = bundle_paths(app);
        let _ = std::fs::remove_dir_all(&paths.old);
        let _ = std::fs::remove_dir_all(unpack_dir(app));
        let keep = read_staged(updates).is_some_and(|s| crate::version::is_newer(&s.version, current));
        if !keep {
            let _ = std::fs::remove_dir_all(&paths.next);
        }
    }
    let staged = read_staged(updates).filter(|s| crate::version::is_newer(&s.version, current));
    if staged.is_none() {
        forget_staged(updates);
    }
    let Ok(dir) = std::fs::read_dir(updates) else { return };
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // What is ours to keep: the records, and the installer a stage is.
        let keep = name.ends_with(".json")
            || staged.as_ref().is_some_and(|s| s.path == entry.path())
            || read_staged(updates).is_some_and(|s| download_of(&name, &s.version));
        if !keep {
            let path = entry.path();
            let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        }
    }
}

/// Whether a release file named `name` is of `version`.
fn download_of(name: &str, version: &str) -> bool {
    name.contains(&format!("-{version}-"))
}

fn run(c: &mut Command) -> Result<(), String> {
    let out = c.stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
}

fn io_failure(e: std::io::Error) -> Failure {
    describe(&format!("the new version could not be written: {e}"), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_and_the_old_app_are_hidden_siblings_and_still_bundles() {
        let p = bundle_paths(Path::new("/Applications/SnoutCode.app"));
        assert_eq!(p.next, Path::new("/Applications/.SnoutCode.next.app"));
        assert_eq!(p.old, Path::new("/Applications/.SnoutCode.old.app"));
        let renamed = bundle_paths(Path::new("/Users/a/Apps/Snout Beta.app"));
        assert_eq!(renamed.next, Path::new("/Users/a/Apps/.Snout Beta.next.app"));
    }

    #[test]
    fn only_a_requirement_that_names_a_signer_is_one() {
        let signed = "Executable=/x\ndesignated => identifier \"com.snoutdata.code\" and anchor apple generic\n";
        assert_eq!(
            designated_requirement(signed).as_deref(),
            Some("identifier \"com.snoutdata.code\" and anchor apple generic")
        );
        assert_eq!(designated_requirement("designated => cdhash H\"abcd\"\n"), None);
        assert_eq!(designated_requirement("code object is not signed at all\n"), None);
    }

    #[test]
    fn staged_is_remembered_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_staged(dir.path()), None);
        let s = Staged { version: "1.0.1".into(), path: "/a/b".into() };
        write_staged(dir.path(), &s).unwrap();
        assert_eq!(read_staged(dir.path()), Some(s));
        forget_staged(dir.path());
        assert_eq!(read_staged(dir.path()), None);
    }

    #[test]
    fn a_launch_tidies_what_is_not_newer_and_keeps_what_is() {
        let dir = tempfile::tempdir().unwrap();
        let (updates, apps) = (dir.path().join("updates"), dir.path().join("apps"));
        std::fs::create_dir_all(&updates).unwrap();
        let app = apps.join("SnoutCode.app");
        let paths = bundle_paths(&app);
        for d in [&app, &paths.next, &paths.old] {
            std::fs::create_dir_all(d).unwrap();
        }
        let zip = updates.join("SnoutCode-1.0.1-arm64-mac.zip");
        let stale = updates.join("SnoutCode-0.9.0-arm64-mac.zip");
        std::fs::write(&zip, "").unwrap();
        std::fs::write(&stale, "").unwrap();
        write_staged(&updates, &Staged { version: "1.0.1".into(), path: paths.next.clone() }).unwrap();

        // Still on 1.0.0: the stage of 1.0.1 is reused.
        cleanup(&updates, Some(&app), "1.0.0");
        assert!(paths.next.exists() && zip.exists() && !paths.old.exists() && !stale.exists());
        assert!(read_staged(&updates).is_some());

        // 1.0.1 arrived.
        cleanup(&updates, Some(&app), "1.0.1");
        assert!(!paths.next.exists() && !zip.exists() && app.exists());
        assert_eq!(read_staged(&updates), None);
    }

    #[cfg(unix)]
    fn fake_app(exe: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(exe, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_self_check_is_the_builds_own_word() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("app");
        fake_app(&exe, &format!("echo 1.0.1 > \"${SELF_CHECK_ENV}\""));
        assert_eq!(self_check(&exe).unwrap(), "1.0.1");
        fake_app(&exe, "exit 3");
        assert!(self_check(&exe).is_err(), "a build that fails says nothing");
        fake_app(&exe, "exit 0");
        assert!(self_check(&exe).is_err(), "a build that does not answer is not one that runs");
        assert!(self_check(&dir.path().join("none")).is_err());
    }

    /// The whole of staging on a real zip, signed ad hoc by the real `codesign`.
    #[cfg(target_os = "macos")]
    #[test]
    fn staging_unpacks_checks_and_refuses_on_a_real_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let (build, apps) = (dir.path().join("build"), dir.path().join("apps"));
        let bundle = build.join(APP_IN_ZIP);
        fake_app(&bundle_exe(&bundle), &format!("echo 1.0.1 > \"${SELF_CHECK_ENV}\""));
        std::fs::write(
            bundle.join("Contents/Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>snout-code</string>\
             <key>CFBundleIdentifier</key><string>com.snoutdata.code.test</string>\
             <key>CFBundlePackageType</key><string>APPL</string></dict></plist>",
        )
        .unwrap();
        let sign = |b: &Path| {
            let ok = Command::new("codesign").args(["--force", "-s", "-"]).arg(b).output().unwrap();
            assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
        };
        sign(&bundle);
        let zip = dir.path().join("new.zip");
        let zipped = Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&bundle).arg(&zip).status().unwrap();
        assert!(zipped.success());
        let live = apps.join(APP_IN_ZIP);
        fake_app(&bundle_exe(&live), "exit 0");
        let log = |_: &str| {};

        let next = stage_bundle(&zip, &live, "1.0.1", CodeCheck::IntactOnly, &log).unwrap();
        assert_eq!(next, bundle_paths(&live).next);
        assert!(bundle_exe(&next).exists());
        assert!(!apps.join(".SnoutCode.unpack").exists());

        // Claims another version: refused, and nothing is left.
        let e = stage_bundle(&zip, &live, "1.0.2", CodeCheck::IntactOnly, &log).unwrap_err();
        assert!(e.detail.unwrap().contains("v1.0.1, not v1.0.2"));
        assert!(!next.exists());

        // Changed after it was signed: refused as a bad signature.
        let next = stage_bundle(&zip, &live, "1.0.1", CodeCheck::IntactOnly, &log).unwrap();
        std::fs::write(bundle_exe(&next), "#!/bin/sh\nexit 0\n").unwrap();
        let e = verify_bundle(&next, &live, "1.0.1", CodeCheck::IntactOnly, &log).unwrap_err();
        assert_eq!(e.kind, crate::failure::Kind::Signature);

        // A live app with no signer to match: a release refuses rather than trusts it.
        let e = stage_bundle(&zip, &live, "1.0.1", CodeCheck::Requirement, &log).unwrap_err();
        assert!(e.detail.unwrap().contains("not signed"));
    }

    /// A garbage file is not a release.
    #[cfg(target_os = "macos")]
    #[test]
    fn an_archive_that_is_not_one_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("x.zip");
        std::fs::write(&zip, "not a zip").unwrap();
        let live = dir.path().join("apps").join(APP_IN_ZIP);
        std::fs::create_dir_all(&live).unwrap();
        assert!(stage_bundle(&zip, &live, "1.0.1", CodeCheck::IntactOnly, &|_| {}).is_err());
        assert_eq!(std::fs::read_dir(live.parent().unwrap()).unwrap().count(), 1, "only the live app");
    }
}
