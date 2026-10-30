//! Linux's terminal apps, found on `PATH` and started directly. The system's default is
//! `xdg-terminal-exec` where the desktop has it, else Debian's `x-terminal-emulator`, else
//! the first of the others installed.

use std::path::{Path, PathBuf};

use super::{ExternalTerminal, Where};

/// The apps, in the order the picker lists them.
pub const APPS: &[ExternalTerminal] = &[
    ExternalTerminal::Terminal,
    ExternalTerminal::GnomeTerminal,
    ExternalTerminal::Konsole,
    ExternalTerminal::XfceTerminal,
    ExternalTerminal::Ghostty,
    ExternalTerminal::WezTerm,
    ExternalTerminal::Alacritty,
    ExternalTerminal::Kitty,
    ExternalTerminal::Foot,
    ExternalTerminal::XTerm,
];

/// What the system's own terminal is called.
pub const DEFAULT_LABEL: &str = "Default Terminal";

/// The terminal is started and left running.
pub const WAITS: bool = false;

/// The program of an app.
fn program(term: ExternalTerminal) -> Option<&'static str> {
    Some(match term {
        ExternalTerminal::GnomeTerminal => "gnome-terminal",
        ExternalTerminal::Konsole => "konsole",
        ExternalTerminal::XfceTerminal => "xfce4-terminal",
        ExternalTerminal::Ghostty => "ghostty",
        ExternalTerminal::WezTerm => "wezterm",
        ExternalTerminal::Alacritty => "alacritty",
        ExternalTerminal::Kitty => "kitty",
        ExternalTerminal::Foot => "foot",
        ExternalTerminal::XTerm => "xterm",
        _ => return None,
    })
}

/// `name` on `PATH`.
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|f| f.is_file())
}

/// The default terminal: a launcher (`xdg-terminal-exec`, `x-terminal-emulator`), or the
/// first app installed.
fn default_terminal() -> Option<(PathBuf, Option<ExternalTerminal>)> {
    for launcher in ["xdg-terminal-exec", "x-terminal-emulator"] {
        if let Some(p) = on_path(launcher) {
            return Some((p, None));
        }
    }
    APPS.iter().filter_map(|t| Some((on_path(program(*t)?)?, Some(*t)))).next()
}

/// Where the app is installed, if it is.
pub fn installed(term: ExternalTerminal) -> Option<PathBuf> {
    match term {
        ExternalTerminal::Terminal => default_terminal().map(|(p, _)| p),
        _ => on_path(program(term)?),
    }
}

/// The command line that opens `term` at `at`, program first.
pub fn command(term: ExternalTerminal, at: &Where, _scripts: &Path) -> std::io::Result<Vec<String>> {
    let missing = || std::io::Error::other(format!("{} is not installed", term.label()));
    let (exe, as_app) = match term {
        ExternalTerminal::Terminal => {
            let (exe, app) = default_terminal().ok_or_else(missing)?;
            (exe.display().to_string(), app)
        }
        _ => (
            program(term).ok_or_else(|| std::io::Error::other(format!("{} is a Mac app", term.label())))?.to_owned(),
            Some(term),
        ),
    };
    Ok(argv(&exe, as_app, at))
}

/// How `exe` opens at `at`: as the app `app`, or as a launcher (`None`) that runs a command
/// given after it and starts in the folder it is run from.
pub fn argv(exe: &str, app: Option<ExternalTerminal>, at: &Where) -> Vec<String> {
    use ExternalTerminal as T;
    let s = |v: &str| v.to_owned();
    // Started in the folder by a shell, for a terminal that takes no folder of its own.
    let in_dir =
        |dir: &Path| vec![s("/bin/sh"), s("-c"), s("cd \"$1\" && exec \"$0\""), s(exe), dir.display().to_string()];
    match (app, at) {
        (Some(T::GnomeTerminal | T::XfceTerminal | T::Ghostty | T::Foot), Where::Local(dir)) => {
            vec![s(exe), format!("--working-directory={}", dir.display())]
        }
        (Some(T::Konsole), Where::Local(dir)) => vec![s(exe), s("--workdir"), dir.display().to_string()],
        (Some(T::WezTerm), Where::Local(dir)) => vec![s(exe), s("start"), s("--cwd"), dir.display().to_string()],
        (Some(T::Alacritty), Where::Local(dir)) => vec![s(exe), s("--working-directory"), dir.display().to_string()],
        (Some(T::Kitty), Where::Local(dir)) => vec![s(exe), s("--directory"), dir.display().to_string()],
        (_, Where::Local(dir)) => in_dir(dir),
        (Some(T::GnomeTerminal), Where::Remote { ssh, .. }) => [vec![s(exe), s("--")], ssh.clone()].concat(),
        (Some(T::XfceTerminal), Where::Remote { ssh, .. }) => [vec![s(exe), s("-x")], ssh.clone()].concat(),
        (Some(T::WezTerm), Where::Remote { ssh, .. }) => [vec![s(exe), s("start"), s("--")], ssh.clone()].concat(),
        (Some(T::Kitty | T::Foot), Where::Remote { ssh, .. }) => [vec![s(exe)], ssh.clone()].concat(),
        // xdg-terminal-exec takes the command as it is.
        (None, Where::Remote { ssh, .. }) if exe.ends_with("xdg-terminal-exec") => [vec![s(exe)], ssh.clone()].concat(),
        // Konsole, Ghostty, Alacritty, xterm and x-terminal-emulator take `-e`.
        (_, Where::Remote { ssh, .. }) => [vec![s(exe), s("-e")], ssh.clone()].concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_app_opens_at_the_folder() {
        let dir = Where::Local(PathBuf::from("/w/shop app"));
        let line = |t| argv(program(t).unwrap(), Some(t), &dir).join(" ");
        assert_eq!(line(ExternalTerminal::GnomeTerminal), "gnome-terminal --working-directory=/w/shop app");
        assert_eq!(line(ExternalTerminal::Konsole), "konsole --workdir /w/shop app");
        assert_eq!(line(ExternalTerminal::WezTerm), "wezterm start --cwd /w/shop app");
        assert_eq!(line(ExternalTerminal::Kitty), "kitty --directory /w/shop app");
        assert_eq!(line(ExternalTerminal::XTerm), "/bin/sh -c cd \"$1\" && exec \"$0\" xterm /w/shop app");
        assert_eq!(argv("/usr/bin/x-terminal-emulator", None, &dir)[3], "/usr/bin/x-terminal-emulator");
    }

    #[test]
    fn a_remote_folder_opens_an_ssh_session_there() {
        let ssh: Vec<String> = ["ssh", "-t", "box", "cd /srv; exec $SHELL -l"].map(String::from).into();
        let at = Where::Remote { dir: PathBuf::from("/srv"), ssh: ssh.clone() };
        assert_eq!(argv("gnome-terminal", Some(ExternalTerminal::GnomeTerminal), &at)[..2], ["gnome-terminal", "--"]);
        assert_eq!(argv("konsole", Some(ExternalTerminal::Konsole), &at)[2..], ssh[..]);
        assert_eq!(argv("/usr/bin/xdg-terminal-exec", None, &at)[1..], ssh[..]);
        assert_eq!(argv("/usr/bin/x-terminal-emulator", None, &at)[1], "-e");
    }

    #[test]
    fn a_mac_app_is_not_offered() {
        assert!(!APPS.contains(&ExternalTerminal::ITerm));
        assert!(command(ExternalTerminal::Warp, &Where::Local(PathBuf::from("/")), Path::new("/tmp")).is_err());
    }
}
