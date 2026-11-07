//! One update check, from the feed to ready, reporting as it goes. Blocking: it runs on a thread of
//! its own.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::failure::{Failure, Kind, describe};
use crate::manifest::{Asset, MANIFEST, Manifest, SIGNATURE, verified};
use crate::route::Route;
use crate::stage::{self, CodeCheck, Staged};
use crate::status::{ManualReason, UpdateStatus};
use crate::version::is_newer;

/// The public release feed: GitHub serves the newest published release under this static path, so
/// no API, no authentication and no rate limit are needed to find the latest.
pub const FEED: &str = "https://github.com/snoutdata/snout-code/releases/latest/download";

/// What a check needs to know.
#[derive(Clone, Debug)]
pub struct Config {
    pub feed: String,
    pub public_key: String,
    pub current: String,
    pub platform: String,
    pub route: Route,
    pub updates: PathBuf,
    pub log: PathBuf,
    pub code_check: CodeCheck,
}

impl Config {
    fn log(&self, message: &str) {
        crate::log_to(&self.log, message);
    }
}

/// An update that is ready: what the swap needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    pub version: String,
    pub notes: Option<String>,
    pub path: PathBuf,
}

/// Run a check, telling `emit` each step. The update that is now ready, if one is.
pub fn run(cfg: &Config, cancel: &AtomicBool, emit: &mut dyn FnMut(UpdateStatus)) -> Option<Ready> {
    emit(UpdateStatus::Checking);
    if cfg.route == Route::FromSource {
        emit(UpdateStatus::NotAvailable { version: cfg.current.clone(), from_source: true });
        return None;
    }
    let manifest = match read_feed(cfg) {
        Ok(m) => m,
        Err(failure) => {
            report(cfg, &failure);
            emit(UpdateStatus::Error(failure));
            return None;
        }
    };
    cfg.log(&format!("the feed says v{}; this is v{}", manifest.version, cfg.current));
    if !is_newer(&manifest.version, &cfg.current) {
        emit(UpdateStatus::NotAvailable { version: cfg.current.clone(), from_source: false });
        return None;
    }
    let version = manifest.version.clone();
    let manual = |reason| UpdateStatus::Manual { version: version.clone(), reason };
    if let Route::Manual(reason) = cfg.route {
        emit(manual(reason));
        return None;
    }
    let record = crate::record::read(&cfg.updates);
    if !crate::record::allowed(record.as_ref(), &version) {
        cfg.log(&format!("v{version} was swapped in {} times and never arrived", crate::record::MAX_ATTEMPTS));
        emit(manual(ManualReason::DidNotLand));
        return None;
    }
    let Some(asset) = manifest.platforms.get(&cfg.platform).cloned() else {
        cfg.log(&format!("v{version} has no file for {}", cfg.platform));
        emit(UpdateStatus::NotAvailable { version: cfg.current.clone(), from_source: false });
        return None;
    };
    emit(UpdateStatus::Available { version: version.clone(), notes: manifest.notes.clone() });
    match fetch_and_stage(cfg, &manifest, &asset, cancel, emit) {
        Ok(path) => {
            emit(UpdateStatus::Downloaded { version: version.clone(), notes: manifest.notes.clone() });
            Some(Ready { version, notes: manifest.notes, path })
        }
        Err(failure) => {
            report(cfg, &failure);
            // A new version that will not stage, and is not a bad signature, is still out there:
            // the user can get it by hand. An alarm stays an alarm.
            if matches!(failure.kind, Kind::Unknown) {
                emit(manual(ManualReason::CannotReplace));
            } else {
                emit(UpdateStatus::Error(failure));
            }
            None
        }
    }
}

fn report(cfg: &Config, failure: &Failure) {
    cfg.log(&format!("update error ({:?}): {}", failure.kind, failure.detail.as_deref().unwrap_or("")));
}

/// The manifest, read with its signature. Once more on a bad signature: a release published
/// between the two reads gives a manifest and a signature of different releases.
fn read_feed(cfg: &Config) -> Result<Manifest, Failure> {
    let once = || -> Result<Manifest, Failure> {
        // A unique query: neither a CDN nor a proxy between us may serve a stale feed.
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let url = |name: &str| {
            if cfg.feed.starts_with("file:") {
                format!("{}/{name}", cfg.feed)
            } else {
                format!("{}/{name}?noCache={stamp}", cfg.feed)
            }
        };
        let body = crate::fetch::text(&url(MANIFEST))?;
        let signature = crate::fetch::text(&url(SIGNATURE))?;
        verified(&body, &signature, &cfg.public_key)
    };
    match once() {
        Err(e) if e.kind == Kind::Signature => once(),
        other => other,
    }
}

fn fetch_and_stage(
    cfg: &Config,
    manifest: &Manifest,
    asset: &Asset,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(UpdateStatus),
) -> Result<PathBuf, Failure> {
    let log = |m: &str| cfg.log(m);
    let version = manifest.version.as_str();
    // Still good from the last launch: it is reused once it passes the check again, since
    // something may have damaged it since.
    if let (Some(staged), Route::MacBundle { app }) = (stage::read_staged(&cfg.updates), &cfg.route)
        && staged.version == version
        && staged.path.exists()
    {
        emit(UpdateStatus::Downloading { percent: None, preparing: true });
        match stage::verify_bundle(&staged.path, app, version, cfg.code_check, &log) {
            Ok(()) => {
                cfg.log(&format!("v{version} is staged already"));
                return Ok(staged.path);
            }
            Err(failure) => report(cfg, &failure),
        }
    }
    let file = cfg.updates.join(&asset.file);
    download(cfg, asset, &file, cancel, emit)?;
    emit(UpdateStatus::Downloading { percent: None, preparing: true });
    let path = match &cfg.route {
        Route::MacBundle { app } => stage::stage_bundle(&file, app, version, cfg.code_check, &log)?,
        // The installer is verified, and is what runs.
        _ => file,
    };
    stage::write_staged(&cfg.updates, &Staged { version: version.to_owned(), path: path.clone() })
        .map_err(|e| describe(&format!("the new version could not be written: {e}"), None))?;
    // Now, not as the app quits, which has seconds to do it in. If this fails the quit tries again.
    if let Route::WindowsInstaller { exe, .. } = &cfg.route
        && let Err(e) = crate::swap::make_waiter_copy(&cfg.updates, exe, &cfg.current)
    {
        cfg.log(&format!("the waiter's program could not be copied yet: {e}"));
    }
    Ok(path)
}

/// `file` whole and matching the signed feed: a download of it from an earlier run is reused.
fn download(
    cfg: &Config,
    asset: &Asset,
    file: &Path,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(UpdateStatus),
) -> Result<(), Failure> {
    let matches = |path: &Path| crate::fetch::sha256(path).is_ok_and(|sha| sha.eq_ignore_ascii_case(&asset.sha256));
    std::fs::create_dir_all(&cfg.updates)
        .map_err(|e| describe(&format!("the new version could not be written: {e}"), None))?;
    if file.exists() {
        if matches(file) {
            cfg.log(&format!("{} is here already", asset.file));
            return Ok(());
        }
        let _ = std::fs::remove_file(file);
    }
    let part = file.with_extension(format!("{}.part", std::process::id()));
    let url = format!("{}/{}", cfg.feed, asset.file);
    cfg.log(&format!("downloading {url}"));
    let mut last = None;
    let result = crate::fetch::download(&url, &part, cancel, |got| {
        let percent = percent(got, asset.size);
        if last != Some(percent) {
            last = Some(percent);
            emit(UpdateStatus::Downloading { percent: Some(percent), preparing: false });
        }
    });
    if let Err(failure) = result {
        let _ = std::fs::remove_file(&part);
        return Err(failure);
    }
    if !matches(&part) {
        let _ = std::fs::remove_file(&part);
        return Err(describe("the download's SHA-256 is not the feed's", None));
    }
    std::fs::rename(&part, file).map_err(|e| describe(&format!("the new version could not be written: {e}"), None))
}

fn percent(got: u64, size: u64) -> u32 {
    got.saturating_mul(100).checked_div(size).map_or(0, |p| p.min(100) as u32)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::manifest::tests::{public, sign};

    /// A feed in a folder (curl reads it through file://), and a scratch place for the updates.
    struct World {
        dir: tempfile::TempDir,
    }

    impl World {
        fn new() -> Self {
            let w = Self { dir: tempfile::tempdir().unwrap() };
            std::fs::create_dir_all(w.feed()).unwrap();
            w
        }
        fn feed(&self) -> PathBuf {
            self.dir.path().join("feed")
        }
        fn updates(&self) -> PathBuf {
            self.dir.path().join("updates")
        }
        /// Publish `version` with one file for the test's platform.
        fn publish(&self, version: &str, file: &str, content: &[u8]) {
            std::fs::write(self.feed().join(file), content).unwrap();
            let sha = crate::manifest::to_hex(&{
                use sha2::Digest as _;
                sha2::Sha256::digest(content)
            });
            let body = format!(
                r#"{{"version":"{version}","notes":"Notes of {version}.","platforms":{{"test":{{"file":"{file}","sha256":"{sha}","size":{}}}}}}}"#,
                content.len()
            );
            std::fs::write(self.feed().join(MANIFEST), &body).unwrap();
            std::fs::write(self.feed().join(SIGNATURE), sign(body.as_bytes())).unwrap();
        }
        fn config(&self, route: Route) -> Config {
            Config {
                feed: format!("file://{}", self.feed().display()),
                public_key: public(),
                current: "1.0.0".into(),
                platform: "test".into(),
                route,
                updates: self.updates(),
                log: self.dir.path().join("update.log"),
                code_check: CodeCheck::IntactOnly,
            }
        }
        fn installer(&self) -> Route {
            Route::WindowsInstaller { folder: "/p".into(), exe: "/p/s.exe".into() }
        }
    }

    fn check(cfg: &Config) -> (Option<Ready>, Vec<UpdateStatus>) {
        let seen = RefCell::new(Vec::new());
        let ready = run(cfg, &AtomicBool::new(false), &mut |s| seen.borrow_mut().push(s));
        (ready, seen.into_inner())
    }

    #[test]
    fn a_build_from_the_source_says_so_and_asks_nothing() {
        let w = World::new();
        let mut cfg = w.config(Route::FromSource);
        cfg.feed = "file:///does/not/exist".into();
        let (ready, seen) = check(&cfg);
        assert_eq!(ready, None);
        assert_eq!(seen.last().unwrap(), &UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: true });
    }

    #[test]
    fn the_same_version_is_up_to_date() {
        let w = World::new();
        w.publish("1.0.0", "setup.exe", b"x");
        let (ready, seen) = check(&w.config(w.installer()));
        assert_eq!(ready, None);
        assert_eq!(seen.last().unwrap(), &UpdateStatus::NotAvailable { version: "1.0.0".into(), from_source: false });
        assert!(!w.updates().join("setup.exe").exists(), "nothing downloaded");
    }

    #[test]
    fn a_new_version_downloads_and_is_ready_in_the_order_the_toast_expects() {
        let w = World::new();
        w.publish("1.0.1", "setup-1.0.1-.exe", b"the installer");
        let (ready, seen) = check(&w.config(w.installer()));
        let ready = ready.expect("ready");
        assert_eq!(ready.version, "1.0.1");
        assert_eq!(ready.notes.as_deref(), Some("Notes of 1.0.1."));
        assert_eq!(std::fs::read(&ready.path).unwrap(), b"the installer");
        assert_eq!(seen[0], UpdateStatus::Checking);
        assert!(matches!(seen[1], UpdateStatus::Available { .. }));
        assert!(matches!(seen.last().unwrap(), UpdateStatus::Downloaded { .. }));
        let staged = stage::read_staged(&w.updates()).unwrap();
        assert_eq!((staged.version.as_str(), staged.path), ("1.0.1", ready.path));
    }

    #[test]
    fn linux_announces_and_downloads_nothing() {
        let w = World::new();
        w.publish("1.0.1", "pkg.deb", b"x");
        let (ready, seen) = check(&w.config(Route::Manual(ManualReason::PackageManager)));
        assert_eq!(ready, None);
        assert_eq!(
            seen.last().unwrap(),
            &UpdateStatus::Manual { version: "1.0.1".into(), reason: ManualReason::PackageManager }
        );
        assert!(!w.updates().exists());
    }

    #[test]
    fn a_feed_someone_replaced_is_refused_loudly_and_nothing_is_fetched() {
        let w = World::new();
        w.publish("1.0.1", "setup.exe", b"x");
        let body = std::fs::read_to_string(w.feed().join(MANIFEST)).unwrap().replace("1.0.1", "1.0.2");
        std::fs::write(w.feed().join(MANIFEST), body).unwrap();
        let (ready, seen) = check(&w.config(w.installer()));
        assert_eq!(ready, None);
        assert!(matches!(seen.last().unwrap(), UpdateStatus::Error(f) if f.kind == Kind::Signature));
        assert!(!w.updates().join("setup.exe").exists());
    }

    #[test]
    fn a_file_someone_replaced_is_refused_and_removed() {
        let w = World::new();
        w.publish("1.0.1", "setup.exe", b"the real one");
        std::fs::write(w.feed().join("setup.exe"), b"another one").unwrap();
        let (ready, seen) = check(&w.config(w.installer()));
        assert_eq!(ready, None);
        assert!(matches!(seen.last().unwrap(), UpdateStatus::Error(f) if f.kind == Kind::Signature));
        assert_eq!(std::fs::read_dir(w.updates()).unwrap().count(), 0, "no file is kept");
    }

    #[test]
    fn a_feed_that_is_not_there_is_an_error_and_a_later_check_succeeds() {
        let w = World::new();
        let (ready, seen) = check(&w.config(w.installer()));
        assert_eq!(ready, None);
        assert!(matches!(seen.last().unwrap(), UpdateStatus::Error(_)));
        w.publish("1.0.1", "setup.exe", b"x");
        assert!(check(&w.config(w.installer())).0.is_some());
    }

    #[test]
    fn a_download_from_an_earlier_run_is_reused_if_it_matches() {
        let w = World::new();
        w.publish("1.0.1", "setup.exe", b"the real one");
        assert!(check(&w.config(w.installer())).0.is_some());
        // The feed's file goes, so only the file already here can make this work.
        std::fs::remove_file(w.feed().join("setup.exe")).unwrap();
        assert!(check(&w.config(w.installer())).0.is_some());
        // Damaged since: fetched again, which now fails, and nothing bad stays.
        std::fs::write(w.updates().join("setup.exe"), b"damaged").unwrap();
        assert!(check(&w.config(w.installer())).0.is_none());
        assert!(!w.updates().join("setup.exe").exists());
    }

    #[test]
    fn a_platform_the_release_has_no_file_for_stays_quiet() {
        let w = World::new();
        w.publish("1.0.1", "setup.exe", b"x");
        let mut cfg = w.config(w.installer());
        cfg.platform = "other".into();
        let (ready, seen) = check(&cfg);
        assert_eq!(ready, None);
        assert!(matches!(seen.last().unwrap(), UpdateStatus::NotAvailable { .. }));
    }

    #[test]
    fn a_version_that_never_arrived_goes_to_the_user() {
        let w = World::new();
        w.publish("1.0.1", "setup.exe", b"x");
        crate::record::note_attempt(&w.updates(), "1.0.1");
        crate::record::note_attempt(&w.updates(), "1.0.1");
        let (ready, seen) = check(&w.config(w.installer()));
        assert_eq!(ready, None);
        assert_eq!(
            seen.last().unwrap(),
            &UpdateStatus::Manual { version: "1.0.1".into(), reason: ManualReason::DidNotLand }
        );
    }

    #[test]
    fn progress_is_a_percentage_that_stops_at_100() {
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(300, 200), 100);
        assert_eq!(percent(1, 0), 0);
    }
}
