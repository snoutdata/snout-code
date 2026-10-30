//! The Mac's terminal apps, found by their bundle ids through LaunchServices and opened with
//! `open`.

use std::path::{Path, PathBuf};

use super::{ExternalTerminal, Where, sh_quote};

/// The apps, in the order the picker lists them.
pub const APPS: &[ExternalTerminal] = &[
    ExternalTerminal::Terminal,
    ExternalTerminal::ITerm,
    ExternalTerminal::Ghostty,
    ExternalTerminal::WezTerm,
    ExternalTerminal::Alacritty,
    ExternalTerminal::Kitty,
    ExternalTerminal::Warp,
];

/// What the system's own terminal is called.
pub const DEFAULT_LABEL: &str = "Terminal";

/// `open` returns once the app has the request: it is waited for.
pub const WAITS: bool = true;

/// The app's bundle identifier, by which LaunchServices finds it wherever it is installed.
fn bundle_id(term: ExternalTerminal) -> Option<&'static str> {
    Some(match term {
        ExternalTerminal::Terminal => "com.apple.Terminal",
        ExternalTerminal::ITerm => "com.googlecode.iterm2",
        ExternalTerminal::Ghostty => "com.mitchellh.ghostty",
        ExternalTerminal::WezTerm => "com.github.wez.wezterm",
        ExternalTerminal::Alacritty => "org.alacritty",
        ExternalTerminal::Kitty => "net.kovidgoyal.kitty",
        ExternalTerminal::Warp => "dev.warp.Warp-Stable",
        _ => return None,
    })
}

/// Where the app is installed, if it is.
pub fn installed(term: ExternalTerminal) -> Option<PathBuf> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let ws = NSWorkspace::sharedWorkspace();
    let url = ws.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id(term)?))?;
    url.path().map(|p| PathBuf::from(p.to_string()))
}

/// The command line that opens `term` at `at`, program first. A terminal that opens only
/// folders or scripts (Terminal, iTerm, Warp) is given an executable `.command` script for an
/// ssh session, written into `scripts`.
pub fn command(term: ExternalTerminal, at: &Where, scripts: &Path) -> std::io::Result<Vec<String>> {
    let s = |v: &str| v.to_owned();
    let open_new =
        |id: &str, args: Vec<String>| [vec![s("open"), s("-n"), s("-b"), id.to_owned(), s("--args")], args].concat();
    let Some(id) = bundle_id(term) else {
        return Err(std::io::Error::other(format!("{} is not a Mac app", term.label())));
    };
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
        _ => unreachable!("every Mac app has a bundle id"),
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

#[cfg(test)]
mod tests {
    use super::super::{command, installed, remote_shell};
    use super::*;

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
    }

    #[test]
    fn launch_services_finds_terminal() {
        assert!(ExternalTerminal::Terminal.installed().is_some(), "Terminal is on every Mac");
        assert!(installed().contains(&ExternalTerminal::Terminal));
    }

    #[test]
    fn a_remote_folder_opens_an_ssh_session_there() {
        let remote_cmd = remote_shell(Path::new("/srv/it's"));
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
        let _ = std::fs::remove_dir_all(scripts);
    }
}
