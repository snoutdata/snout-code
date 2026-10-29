//! Open in External Terminal: a terminal app of the Mac's own, at the folder of what was
//! picked (a file opens at its parent). Which app is a setting: Terminal, iTerm, Ghostty,
//! WezTerm, Alacritty, kitty or Warp, found by their bundle ids through LaunchServices, or a
//! command of the user's own with `{dir}` in it.
//!
//! On a project of a remote machine the terminal opens an ssh session there instead,
//! `ssh -t <host> 'cd <dir>; exec $SHELL -l'`, over the same ssh config and shared
//! connection (ControlMaster) the window uses, so it opens without asking again.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The terminal app Open in External Terminal opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ExternalTerminal {
    /// macOS's Terminal.
    #[default]
    Terminal,
    #[serde(rename = "iTerm")]
    ITerm,
    Ghostty,
    WezTerm,
    Alacritty,
    #[serde(rename = "kitty")]
    Kitty,
    Warp,
    /// The command in `external_terminal_command`.
    Custom,
}

impl ExternalTerminal {
    /// The apps, in the order the picker lists them.
    pub const APPS: [Self; 7] =
        [Self::Terminal, Self::ITerm, Self::Ghostty, Self::WezTerm, Self::Alacritty, Self::Kitty, Self::Warp];

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::ITerm => "iTerm",
            Self::Ghostty => "Ghostty",
            Self::WezTerm => "WezTerm",
            Self::Alacritty => "Alacritty",
            Self::Kitty => "kitty",
            Self::Warp => "Warp",
            Self::Custom => "Custom command",
        }
    }

    /// The app's bundle identifier, by which LaunchServices finds it wherever it is installed.
    pub fn bundle_id(self) -> Option<&'static str> {
        Some(match self {
            Self::Terminal => "com.apple.Terminal",
            Self::ITerm => "com.googlecode.iterm2",
            Self::Ghostty => "com.mitchellh.ghostty",
            Self::WezTerm => "com.github.wez.wezterm",
            Self::Alacritty => "org.alacritty",
            Self::Kitty => "net.kovidgoyal.kitty",
            Self::Warp => "dev.warp.Warp-Stable",
            Self::Custom => return None,
        })
    }

    /// Where the app is installed, if it is.
    pub fn installed(self) -> Option<PathBuf> {
        app_for_bundle(self.bundle_id()?)
    }
}

/// The apps installed on this Mac, in the picker's order.
pub fn installed() -> Vec<ExternalTerminal> {
    ExternalTerminal::APPS.into_iter().filter(|t| t.installed().is_some()).collect()
}

#[cfg(target_os = "macos")]
fn app_for_bundle(id: &str) -> Option<PathBuf> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let ws = NSWorkspace::sharedWorkspace();
    let url = ws.URLForApplicationWithBundleIdentifier(&NSString::from_str(id))?;
    url.path().map(|p| PathBuf::from(p.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn app_for_bundle(_: &str) -> Option<PathBuf> {
    None
}

/// `s` quoted for `sh`: as it is when it is plain, else in single quotes.
pub fn sh_quote(s: &str) -> String {
    let plain = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c));
    if plain { s.to_owned() } else { format!("'{}'", s.replace('\'', r"'\''")) }
}

/// The shell command an ssh session runs on the remote machine: a login shell in `dir`.
pub fn remote_shell(dir: &Path) -> String {
    format!("cd {}; exec $SHELL -l", sh_quote(&dir.display().to_string()))
}

/// What to open: a folder of this Mac, or one of a remote machine with the ssh command line
/// (program first) that opens a terminal there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    Local(PathBuf),
    Remote { dir: PathBuf, ssh: Vec<String> },
}

/// The command line that opens `term` at `at`, program first. A terminal that opens only
/// folders or scripts (Terminal, iTerm, Warp) is given an executable `.command` script for an
/// ssh session, written into `scripts`.
pub fn command(term: ExternalTerminal, custom: &str, at: &Where, scripts: &Path) -> std::io::Result<Vec<String>> {
    let s = |v: &str| v.to_owned();
    let open_new =
        |id: &str, args: Vec<String>| [vec![s("open"), s("-n"), s("-b"), id.to_owned(), s("--args")], args].concat();
    if term == ExternalTerminal::Custom {
        let template = custom.trim();
        if template.is_empty() {
            return Err(std::io::Error::other(
                "The custom external terminal has no command: set one in Settings ▸ Tools ▸ Terminal.",
            ));
        }
        let (dir, ssh) = match at {
            Where::Local(dir) => (dir, String::new()),
            Where::Remote { dir, ssh } => (dir, ssh.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" ")),
        };
        let line = template.replace("{dir}", &sh_quote(&dir.display().to_string())).replace("{ssh}", &ssh);
        return Ok(vec![s("/bin/sh"), s("-c"), line]);
    }
    let id = term.bundle_id().unwrap_or_default();
    Ok(match (term, at) {
        (ExternalTerminal::Terminal | ExternalTerminal::ITerm | ExternalTerminal::Warp, Where::Local(dir)) => {
            vec![s("open"), s("-b"), s(id), dir.display().to_string()]
        }
        (ExternalTerminal::Terminal | ExternalTerminal::ITerm | ExternalTerminal::Warp, Where::Remote { ssh, .. }) => {
            let script = write_script(ssh, scripts)?;
            vec![s("open"), s("-b"), s(id), script.display().to_string()]
        }
        (ExternalTerminal::Ghostty, Where::Local(dir)) => {
            open_new(id, vec![format!("--working-directory={}", dir.display())])
        }
        (ExternalTerminal::Ghostty, Where::Remote { ssh, .. }) => open_new(id, [vec![s("-e")], ssh.clone()].concat()),
        (ExternalTerminal::WezTerm, Where::Local(dir)) => {
            open_new(id, vec![s("start"), s("--cwd"), dir.display().to_string()])
        }
        (ExternalTerminal::WezTerm, Where::Remote { ssh, .. }) => {
            open_new(id, [vec![s("start"), s("--")], ssh.clone()].concat())
        }
        (ExternalTerminal::Alacritty, Where::Local(dir)) => {
            open_new(id, vec![s("--working-directory"), dir.display().to_string()])
        }
        (ExternalTerminal::Alacritty, Where::Remote { ssh, .. }) => open_new(id, [vec![s("-e")], ssh.clone()].concat()),
        (ExternalTerminal::Kitty, Where::Local(dir)) => open_new(id, vec![s("--directory"), dir.display().to_string()]),
        (ExternalTerminal::Kitty, Where::Remote { ssh, .. }) => open_new(id, ssh.clone()),
        (ExternalTerminal::Custom, _) => unreachable!("handled above"),
    })
}

/// An executable `.command` script that runs `argv`: what Terminal, iTerm and Warp open.
fn write_script(argv: &[String], dir: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = dir.join(format!("ssh-{stamp}.command"));
    let line = argv.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" ");
    // The script removes itself: it is only a way to hand the command to the app.
    std::fs::write(&path, format!("#!/bin/sh\nrm -f \"$0\"\nexec {line}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

/// Run `argv` (from [`command`]) and wait for it: `open` returns once the app has the
/// request. Why it failed, if it did.
pub fn run(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or_else(|| "nothing to run".to_owned())?;
    let mut c = std::process::Command::new(program);
    c.args(args).stdin(std::process::Stdio::null());
    if program == "/bin/sh" {
        // A custom terminal app keeps running: do not wait for it.
        return c.spawn().map(drop).map_err(|e| format!("Could not run the external terminal command: {e}"));
    }
    let out = c.output().map_err(|e| format!("Could not open the external terminal: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("Could not open the external terminal: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_is_kept_in_the_settings_by_the_apps_names() {
        assert_eq!(serde_json::to_string(&ExternalTerminal::ITerm).unwrap(), r#""iTerm""#);
        assert_eq!(serde_json::to_string(&ExternalTerminal::Kitty).unwrap(), r#""kitty""#);
        let s = crate::settings::Settings {
            external_terminal: ExternalTerminal::Custom,
            external_terminal_command: "myterm {dir}".into(),
            ..Default::default()
        };
        let back: crate::settings::Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(
            (back.external_terminal, back.external_terminal_command.as_str()),
            (ExternalTerminal::Custom, "myterm {dir}")
        );
    }

    #[test]
    fn each_app_opens_at_the_folder() {
        let dir = Where::Local(PathBuf::from("/w/shop app"));
        let scripts = std::env::temp_dir();
        let line = |t| command(t, "", &dir, &scripts).unwrap().join(" ");
        assert_eq!(line(ExternalTerminal::Terminal), "open -b com.apple.Terminal /w/shop app");
        assert_eq!(
            line(ExternalTerminal::Ghostty),
            "open -n -b com.mitchellh.ghostty --args --working-directory=/w/shop app"
        );
        assert_eq!(line(ExternalTerminal::WezTerm), "open -n -b com.github.wez.wezterm --args start --cwd /w/shop app");
        assert_eq!(line(ExternalTerminal::Kitty), "open -n -b net.kovidgoyal.kitty --args --directory /w/shop app");
        let custom = command(ExternalTerminal::Custom, "myterm --cd {dir}", &dir, &scripts).unwrap();
        assert_eq!(custom, ["/bin/sh", "-c", "myterm --cd '/w/shop app'"]);
        assert!(command(ExternalTerminal::Custom, " ", &dir, &scripts).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launch_services_finds_terminal_and_a_custom_command_runs_at_the_folder() {
        assert!(ExternalTerminal::Terminal.installed().is_some(), "Terminal is on every Mac");
        assert!(installed().contains(&ExternalTerminal::Terminal));
        let dir = crate::tests::scratch_dir("external-terminal-custom");
        let spaced = dir.join("a folder");
        std::fs::create_dir_all(&spaced).unwrap();
        let argv =
            command(ExternalTerminal::Custom, "touch {dir}/opened", &Where::Local(spaced.clone()), &dir).unwrap();
        run(&argv).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !spaced.join("opened").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(spaced.join("opened").exists(), "the command ran with the folder");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_remote_folder_opens_an_ssh_session_there() {
        let remote_cmd = remote_shell(Path::new("/srv/it's"));
        assert_eq!(remote_cmd, r"cd '/srv/it'\''s'; exec $SHELL -l");
        let ssh: Vec<String> = ["ssh", "-o", "ControlPath=/tmp/s/x", "-t", "box", &remote_cmd].map(String::from).into();
        let at = Where::Remote { dir: PathBuf::from("/srv/it's"), ssh: ssh.clone() };
        let scripts = crate::tests::scratch_dir("external-terminal-scripts");
        let ghostty = command(ExternalTerminal::Ghostty, "", &at, &scripts).unwrap();
        assert_eq!(&ghostty[..6], ["open", "-n", "-b", "com.mitchellh.ghostty", "--args", "-e"]);
        assert_eq!(&ghostty[6..], &ssh[..]);
        // Terminal gets a script that runs ssh, and removes itself.
        let terminal = command(ExternalTerminal::Terminal, "", &at, &scripts).unwrap();
        let script = std::fs::read_to_string(&terminal[3]).unwrap();
        assert!(
            script.contains(
                r"exec ssh -o ControlPath=/tmp/s/x -t box 'cd '\''/srv/it'\''\'\'''\''s'\''; exec $SHELL -l'"
            ),
            "{script}"
        );
        let custom = command(ExternalTerminal::Custom, "open -na MyTerm --args -e {ssh}", &at, &scripts).unwrap();
        assert!(
            custom[2].starts_with("open -na MyTerm --args -e ssh -o ControlPath=/tmp/s/x -t box 'cd"),
            "{custom:?}"
        );
        let _ = std::fs::remove_dir_all(scripts);
    }
}
