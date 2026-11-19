//! Git's credentials, in one place: for each repository, the helpers that answer for its
//! remote, where each is set, the programs each calls and whether Git finds them, and the
//! account they sign in as. A helper that calls a program by a name Git's `PATH` lacks, when
//! the program is installed elsewhere (`gh` in `/opt/homebrew/bin` for a window opened from
//! the Finder), is repaired deterministically: it calls the program by its full path, in the
//! file that set it. Tokens are kept by the system's own helper (the Keychain on the Mac),
//! through `git credential`; SnoutCode never answers for Git itself.

use std::path::{Path, PathBuf};

use serde::Serialize;
use snout_core::credentials::{self as core, Helper};
use snout_host::Host;

/// A program a helper calls, and whether Git finds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Program {
    pub name: String,
    /// Where Git finds it (its `PATH`, or its own folder for `git-credential-*`).
    pub on_git_path: Option<PathBuf>,
    /// Where it is installed when Git does not find it: where installers put programs, or
    /// where the user's shell finds it.
    pub installed_at: Option<PathBuf>,
}

/// A helper that answers for the repository's remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelperState {
    pub helper: Helper,
    pub programs: Vec<Program>,
}

impl HelperState {
    /// Whether every program it calls is there for Git.
    pub fn works(&self) -> bool {
        self.programs.iter().all(|p| p.on_git_path.is_some())
    }
}

/// What a repository's sign in rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub repo: PathBuf,
    /// `origin`'s URL (or the first remote's).
    pub remote: Option<String>,
    /// The helpers that answer for the remote, in Git's order.
    pub helpers: Vec<HelperState>,
    /// The remote is reached over SSH: keys, not helpers, sign in.
    pub ssh: bool,
}

/// The repair a helper takes: call `program` by `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repair {
    pub repo: PathBuf,
    pub helper: Helper,
    pub program: String,
    pub path: PathBuf,
    /// The helper's value after the repair.
    pub value: String,
}

impl Report {
    /// The first helper that a full path repairs, if any.
    pub fn repair(&self) -> Option<Repair> {
        self.helpers.iter().find_map(|h| {
            h.helper.file.as_ref()?;
            h.programs.iter().find_map(|p| {
                let path = p.installed_at.as_ref().filter(|_| p.on_git_path.is_none())?;
                let value = h.helper.calling(&p.name, path)?;
                Some(Repair {
                    repo: self.repo.clone(),
                    helper: h.helper.clone(),
                    program: p.name.clone(),
                    path: path.clone(),
                    value,
                })
            })
        })
    }

    /// The server of the remote, as `github.com`.
    pub fn host_name(&self) -> Option<String> {
        let url = self.remote.as_deref()?;
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        let authority = rest.split(['/', ':']).next()?;
        Some(authority.rsplit('@').next().unwrap_or(authority).to_owned())
    }
}

/// The remote whose sign in matters in `repo`: `origin`, else the first one.
fn main_remote(host: &dyn Host, repo: &Path) -> Option<String> {
    if let Some(url) = core::remote_url(host, repo, "origin") {
        return Some(url);
    }
    let out = host.run(&snout_host::Cmd::new("git").arg("-C").arg(repo).arg("remote"), None).ok()?;
    let first = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_owned();
    core::remote_url(host, repo, &first)
}

/// Git's own folder, where it finds `git-credential-*` helpers.
fn exec_path(host: &dyn Host) -> Option<PathBuf> {
    let out = host.run(&snout_host::Cmd::new("git").arg("--exec-path"), None).ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!p.is_empty()).then(|| PathBuf::from(p))
}

/// Folders to look in first for a program Git does not find, besides where installers put
/// programs and where the user's shell finds them (the tests' own).
#[derive(Clone, Default)]
pub struct ProgramDirs(pub Vec<PathBuf>);

impl gpui::Global for ProgramDirs {}

impl ProgramDirs {
    pub fn of(cx: &gpui::App) -> Vec<PathBuf> {
        cx.try_global::<Self>().map(|d| d.0.clone()).unwrap_or_default()
    }
}

/// Where Git finds `name`, and where it is installed when Git does not (in `extra` first).
/// Asks the user's shell, which can take seconds: call it off the UI thread.
fn locate(host: &dyn Host, name: &str, exec: Option<&Path>, extra: &[PathBuf]) -> Program {
    let path = Path::new(name);
    let on_git_path = if path.is_absolute() {
        host.is_file(path).then(|| path.to_owned())
    } else {
        let extra: Vec<PathBuf> = exec.map(Path::to_path_buf).into_iter().collect();
        snout_host::which(host, &[name], &extra)
    };
    let installed_at = if on_git_path.is_some() || path.is_absolute() {
        None
    } else {
        extra
            .iter()
            .map(|d| d.join(name))
            .find(|p| host.is_file(p))
            .or_else(|| crate::agents::find_agent(host, name))
            .or_else(|| crate::agents::from_shell(host, name))
    };
    Program { name: name.to_owned(), on_git_path, installed_at }
}

/// What `repo`'s sign in rests on, looking in `extra` first for a program Git does not find.
/// Runs Git and may ask the user's shell: off the UI thread.
pub fn diagnose(host: &dyn Host, repo: &Path, extra: &[PathBuf]) -> Report {
    let remote = main_remote(host, repo);
    let ssh = remote.as_deref().is_some_and(|u| !u.contains("://") || u.starts_with("ssh://"));
    let all = core::helpers(host, repo).unwrap_or_default();
    let url = remote.clone().unwrap_or_default();
    let exec = exec_path(host);
    let helpers = if ssh {
        Vec::new()
    } else {
        core::in_effect(&all, &url)
            .into_iter()
            .map(|h| HelperState {
                helper: h.clone(),
                programs: h
                    .programs()
                    .into_iter()
                    .map(|(name, _)| locate(host, &name, exec.as_deref(), extra))
                    .collect(),
            })
            .collect()
    };
    Report { repo: repo.to_owned(), remote, helpers, ssh }
}

/// Make the repair: the helper calls the program by its full path.
pub fn apply(host: &dyn Host, repair: &Repair) -> Result<(), String> {
    core::set_helper(host, &repair.repo, &repair.helper, &repair.value).map_err(|e| e.to_string())
}

/// The account the helpers sign in as for `repo`'s remote, asked without prompting anyone.
pub fn account(host: &dyn Host, report: &Report) -> Option<String> {
    let url = report.remote.as_deref().filter(|_| !report.ssh)?;
    core::account_for(host, &report.repo, url)
}

/// Keep `username` and `token` for `repo`'s remote in the system's keychain helper, adding that
/// helper to the user's Git settings first when no helper that stores answers for the remote.
pub fn save_token(host: &dyn Host, report: &Report, username: &str, token: &str) -> Result<(), String> {
    let url = report.remote.as_deref().ok_or("The repository has no remote.")?;
    let store = core::system_store(&host.info().os);
    let stores = report
        .helpers
        .iter()
        .any(|h| h.works() && h.programs.iter().any(|p| p.name.ends_with(&format!("credential-{store}"))));
    if !stores {
        let out = host
            .run(&snout_host::Cmd::new("git").args(["config", "--global", "--add", "credential.helper", store]), None)
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
    }
    core::save(host, &report.repo, url, username, token).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use snout_core::credentials::ConfigScope;

    fn report(installed: Option<&str>, on_path: bool) -> Report {
        let helper = Helper {
            scope: ConfigScope::Local,
            file: Some("/r/.git/config".into()),
            key: "credential.https://github.com.helper".into(),
            url: Some("https://github.com".into()),
            value: "!gh auth git-credential".into(),
        };
        let program = Program {
            name: "gh".into(),
            on_git_path: on_path.then(|| PathBuf::from("/usr/bin/gh")),
            installed_at: installed.map(PathBuf::from),
        };
        Report {
            repo: "/r".into(),
            remote: Some("https://github.com/joel/sarvida.git".into()),
            helpers: vec![HelperState { helper, programs: vec![program] }],
            ssh: false,
        }
    }

    #[test]
    fn a_helper_whose_program_is_installed_elsewhere_has_a_repair() {
        let r = report(Some("/opt/homebrew/bin/gh"), false);
        let repair = r.repair().expect("a repair");
        assert_eq!(repair.value, "!/opt/homebrew/bin/gh auth git-credential");
        assert_eq!(repair.program, "gh");
        assert_eq!(r.host_name().as_deref(), Some("github.com"));
        assert!(!r.helpers[0].works());
        // Found by Git: nothing to repair. Installed nowhere: nothing a path repairs.
        assert!(report(None, true).repair().is_none());
        assert!(report(None, false).repair().is_none());
    }
}
