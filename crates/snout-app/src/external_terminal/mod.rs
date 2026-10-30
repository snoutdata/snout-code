//! Open in External Terminal: a terminal app of the system's own, at the folder of what was
//! picked (a file opens at its parent). Which app is a setting: on a Mac Terminal, iTerm,
//! Ghostty, WezTerm, Alacritty, kitty or Warp, found by their bundle ids through
//! LaunchServices (`mac.rs`); on Linux the system's default terminal, GNOME Terminal, Konsole,
//! Xfce Terminal, Ghostty, WezTerm, Alacritty, kitty, foot or xterm, found on `PATH`
//! (`linux.rs`); on Windows Windows Terminal, PowerShell, the Command Prompt, WezTerm or
//! Alacritty (`windows.rs`); or, on each, a command of the user's own with `{dir}` in it.
//!
//! On a project of a remote machine the terminal opens an ssh session there instead,
//! `ssh -t <host> 'cd <dir>; exec $SHELL -l'`, over the same ssh config and shared
//! connection (ControlMaster) the window uses, so it opens without asking again.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod mac;
#[cfg(target_os = "macos")]
use mac as imp;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

/// The terminal app Open in External Terminal opens. The settings name every app of every
/// system; each system offers its own (see [`ExternalTerminal::APPS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ExternalTerminal {
    /// macOS's Terminal; on Linux, the system's default terminal.
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
    #[serde(rename = "GNOME Terminal")]
    GnomeTerminal,
    Konsole,
    #[serde(rename = "Xfce Terminal")]
    XfceTerminal,
    #[serde(rename = "foot")]
    Foot,
    #[serde(rename = "xterm")]
    XTerm,
    PowerShell,
    #[serde(rename = "Command Prompt")]
    CommandPrompt,
    /// The command in `external_terminal_command`.
    Custom,
}

impl ExternalTerminal {
    /// The apps of this system, in the order the picker lists them.
    pub const APPS: &'static [Self] = imp::APPS;

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => imp::DEFAULT_LABEL,
            Self::ITerm => "iTerm",
            Self::Ghostty => "Ghostty",
            Self::WezTerm => "WezTerm",
            Self::Alacritty => "Alacritty",
            Self::Kitty => "kitty",
            Self::Warp => "Warp",
            Self::GnomeTerminal => "GNOME Terminal",
            Self::Konsole => "Konsole",
            Self::XfceTerminal => "Xfce Terminal",
            Self::Foot => "foot",
            Self::XTerm => "xterm",
            Self::PowerShell => "PowerShell",
            Self::CommandPrompt => "Command Prompt",
            Self::Custom => "Custom command",
        }
    }

    /// Where the app is installed, if it is.
    pub fn installed(self) -> Option<PathBuf> {
        imp::installed(self)
    }
}

/// The apps installed here, in the picker's order.
pub fn installed() -> Vec<ExternalTerminal> {
    ExternalTerminal::APPS.iter().copied().filter(|t| t.installed().is_some()).collect()
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

/// The command line that opens `term` at `at`, program first (see the platform's own
/// `command`). A custom command runs through `sh` (on Windows the Command Prompt), with
/// `{dir}` and `{ssh}` filled in.
pub fn command(term: ExternalTerminal, custom: &str, at: &Where, scripts: &Path) -> std::io::Result<Vec<String>> {
    if term == ExternalTerminal::Custom {
        let template = custom.trim();
        if template.is_empty() {
            return Err(std::io::Error::other(
                "The custom external terminal has no command: set one in Settings ▸ Tools ▸ Terminal.",
            ));
        }
        let (dir, ssh) = match at {
            Where::Local(dir) => (dir, String::new()),
            Where::Remote { dir, ssh } => (dir, ssh.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")),
        };
        let line = template.replace("{dir}", &shell_quote(&dir.display().to_string())).replace("{ssh}", &ssh);
        #[cfg(windows)]
        return Ok(imp::shell(line));
        #[cfg(not(windows))]
        return Ok(vec!["/bin/sh".to_owned(), "-c".to_owned(), line]);
    }
    imp::command(term, at, scripts)
}

/// `s` quoted for the shell a custom command runs in: `sh`, or on Windows the Command Prompt.
fn shell_quote(s: &str) -> String {
    #[cfg(windows)]
    return imp::quote(s);
    #[cfg(not(windows))]
    sh_quote(s)
}

/// Run `argv` (from [`command`]). On a Mac it waits for `open`, which returns once the app has
/// the request; a terminal app started directly (Linux, Windows, or a custom command) keeps
/// running, so it is not waited for. Why it failed, if it did.
#[cfg(windows)]
pub fn run(argv: &[String]) -> Result<(), String> {
    imp::run(argv)
}

/// Run `argv` (from [`command`]). On a Mac it waits for `open`, which returns once the app has
/// the request; a terminal app started directly (Linux, or a custom command) keeps running,
/// so it is not waited for. Why it failed, if it did.
#[cfg(unix)]
pub fn run(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or_else(|| "nothing to run".to_owned())?;
    let mut c = std::process::Command::new(program);
    c.args(args).stdin(std::process::Stdio::null());
    if program == "/bin/sh" || !imp::WAITS {
        use std::os::unix::process::CommandExt as _;
        // The terminal outlives the app: a process group of its own keeps it from the app's
        // signals.
        c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).process_group(0);
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
        assert_eq!(serde_json::to_string(&ExternalTerminal::GnomeTerminal).unwrap(), r#""GNOME Terminal""#);
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

    #[cfg(unix)]
    #[test]
    fn a_custom_command_runs_at_the_folder_or_over_ssh() {
        let dir = Where::Local(PathBuf::from("/w/shop app"));
        let scripts = std::env::temp_dir();
        let custom = command(ExternalTerminal::Custom, "myterm --cd {dir}", &dir, &scripts).unwrap();
        assert_eq!(custom, ["/bin/sh", "-c", "myterm --cd '/w/shop app'"]);
        assert!(command(ExternalTerminal::Custom, " ", &dir, &scripts).is_err());
        let remote_cmd = remote_shell(Path::new("/srv/it's"));
        assert_eq!(remote_cmd, r"cd '/srv/it'\''s'; exec $SHELL -l");
        let ssh: Vec<String> = ["ssh", "-o", "ControlPath=/tmp/s/x", "-t", "box", &remote_cmd].map(String::from).into();
        let at = Where::Remote { dir: PathBuf::from("/srv/it's"), ssh };
        let custom = command(ExternalTerminal::Custom, "open -na MyTerm --args -e {ssh}", &at, &scripts).unwrap();
        assert!(
            custom[2].starts_with("open -na MyTerm --args -e ssh -o ControlPath=/tmp/s/x -t box 'cd"),
            "{custom:?}"
        );
    }

    #[test]
    fn a_custom_command_runs_with_the_folder() {
        let dir = crate::tests::scratch_dir("external-terminal-custom");
        let spaced = dir.join("a folder");
        std::fs::create_dir_all(&spaced).unwrap();
        let template = if cfg!(windows) { "cd /d {dir} && type nul > opened" } else { "touch {dir}/opened" };
        let argv = command(ExternalTerminal::Custom, template, &Where::Local(spaced.clone()), &dir).unwrap();
        run(&argv).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !spaced.join("opened").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(spaced.join("opened").exists(), "the command ran with the folder");
        let _ = std::fs::remove_dir_all(dir);
    }
}
