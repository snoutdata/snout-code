//! Windows's terminals: Windows Terminal (`wt.exe`), PowerShell (PowerShell 7's `pwsh.exe`
//! where it is installed, else Windows PowerShell), the Command Prompt, WezTerm and
//! Alacritty, found on `PATH`. Windows Terminal is the default; where it is missing (Windows
//! 10 without it), PowerShell opens instead. A console program opens in a console window of
//! its own through `cmd /c start`.

use std::path::{Path, PathBuf};

use super::{ExternalTerminal, Where};

/// The apps, in the order the picker lists them.
pub const APPS: &[ExternalTerminal] = &[
    ExternalTerminal::Terminal,
    ExternalTerminal::PowerShell,
    ExternalTerminal::CommandPrompt,
    ExternalTerminal::WezTerm,
    ExternalTerminal::Alacritty,
];

/// What the system's own terminal is called.
pub const DEFAULT_LABEL: &str = "Windows Terminal";

/// `name` on `PATH`, with Windows's program extensions.
fn on_path(name: &str) -> Option<PathBuf> {
    let host = snout_host::local();
    snout_host::which(&*host, &[name], &[])
}

/// PowerShell 7 where it is installed, else Windows PowerShell (always there).
fn powershell() -> PathBuf {
    on_path("pwsh").or_else(|| on_path("powershell")).unwrap_or_else(|| PathBuf::from("powershell.exe"))
}

/// Where the app is installed, if it is.
pub fn installed(term: ExternalTerminal) -> Option<PathBuf> {
    match term {
        // Windows Terminal, else PowerShell in its place.
        ExternalTerminal::Terminal => on_path("wt").or_else(|| Some(powershell())),
        ExternalTerminal::PowerShell => Some(powershell()),
        ExternalTerminal::CommandPrompt => on_path("cmd"),
        ExternalTerminal::WezTerm => on_path("wezterm"),
        ExternalTerminal::Alacritty => on_path("alacritty"),
        _ => None,
    }
}

/// The command line that opens `term` at `at`, program first.
pub fn command(term: ExternalTerminal, at: &Where, _scripts: &Path) -> std::io::Result<Vec<String>> {
    let missing = || std::io::Error::other(format!("{} is not installed", term.label()));
    let s = |v: &str| v.to_owned();
    let exe = |p: PathBuf| p.display().to_string();
    let console = |dir: Option<&Path>, program: Vec<String>| {
        let mut v = vec![s("cmd.exe"), s("/d"), s("/c"), s("start"), s("")];
        if let Some(dir) = dir {
            v.extend([s("/D"), dir.display().to_string()]);
        }
        v.extend(program);
        v
    };
    let wt = match term {
        ExternalTerminal::Terminal => on_path("wt"),
        _ => None,
    };
    Ok(match (term, wt, at) {
        (ExternalTerminal::Terminal, Some(wt), Where::Local(dir)) => vec![exe(wt), s("-d"), dir.display().to_string()],
        // Windows Terminal reads `;` as the start of another command: `\;` is a semicolon.
        (ExternalTerminal::Terminal, Some(wt), Where::Remote { ssh, .. }) => {
            [vec![exe(wt), s("new-tab"), s("--")], ssh.iter().map(|a| a.replace(';', "\\;")).collect()].concat()
        }
        (ExternalTerminal::Terminal | ExternalTerminal::PowerShell, _, Where::Local(dir)) => {
            console(Some(dir), vec![exe(powershell()), s("-NoLogo")])
        }
        (ExternalTerminal::CommandPrompt, _, Where::Local(dir)) => console(Some(dir), vec![s("cmd.exe")]),
        (
            ExternalTerminal::Terminal | ExternalTerminal::PowerShell | ExternalTerminal::CommandPrompt,
            _,
            Where::Remote { ssh, .. },
        ) => console(None, ssh.clone()),
        (ExternalTerminal::WezTerm, _, Where::Local(dir)) => {
            vec![exe(on_path("wezterm").ok_or_else(missing)?), s("start"), s("--cwd"), dir.display().to_string()]
        }
        (ExternalTerminal::WezTerm, _, Where::Remote { ssh, .. }) => {
            [vec![exe(on_path("wezterm").ok_or_else(missing)?), s("start"), s("--")], ssh.clone()].concat()
        }
        (ExternalTerminal::Alacritty, _, Where::Local(dir)) => {
            vec![exe(on_path("alacritty").ok_or_else(missing)?), s("--working-directory"), dir.display().to_string()]
        }
        (ExternalTerminal::Alacritty, _, Where::Remote { ssh, .. }) => {
            [vec![exe(on_path("alacritty").ok_or_else(missing)?), s("-e")], ssh.clone()].concat()
        }
        _ => return Err(std::io::Error::other(format!("{} is not a Windows app", term.label()))),
    })
}

/// A custom command line, run by the Command Prompt.
pub fn shell(line: String) -> Vec<String> {
    vec!["cmd.exe".to_owned(), "/d".to_owned(), "/c".to_owned(), line]
}

/// `s` quoted for the Command Prompt: in double quotes when it has anything but plain
/// characters.
pub fn quote(s: &str) -> String {
    let plain = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || r"-_./:\@+=,".contains(c));
    if plain { s.to_owned() } else { format!("\"{}\"", s.replace('"', "\"\"")) }
}

/// An argument of `start` as `cmd` reads it: its switches (`/D`) bare, anything else in
/// double quotes (the first quoted one, even empty, is the window's title).
fn start_quote(a: &str) -> String {
    if a.starts_with('/') && !a.contains([' ', '"']) { a.to_owned() } else { format!("\"{a}\"") }
}

/// Start `argv` on its own: it outlives the app. The line `cmd /c` runs is passed as it is,
/// each argument in double quotes, so `cmd` reads it as written.
pub fn run(argv: &[String]) -> Result<(), String> {
    use std::os::windows::process::CommandExt as _;
    let (program, args) = argv.split_first().ok_or_else(|| "nothing to run".to_owned())?;
    let mut c = std::process::Command::new(program);
    let is_cmd = program.eq_ignore_ascii_case("cmd.exe");
    if is_cmd && args.len() >= 2 && args[1] == "/c" && args.get(2).is_some_and(|a| a == "start") {
        // `cmd /d /c start "" /D "dir" "program" args…`
        let line = args[3..].iter().map(|a| start_quote(a)).collect::<Vec<_>>().join(" ");
        c.args(&args[..2]).raw_arg(format!("start {line}"));
    } else if is_cmd && args.len() == 3 && args[1] == "/c" {
        // A custom command: the line as the user wrote it.
        c.args(&args[..2]).raw_arg(&args[2]);
    } else {
        c.args(args);
    }
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    snout_host::win::spawn_detached(&mut c).map(drop).map_err(|e| format!("Could not open the external terminal: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_programs_open_at_the_folder_in_a_window_of_their_own() {
        let dir = Where::Local(PathBuf::from(r"C:\w\shop app"));
        let line = command(ExternalTerminal::CommandPrompt, &dir, Path::new(".")).unwrap();
        assert_eq!(line, ["cmd.exe", "/d", "/c", "start", "", "/D", r"C:\w\shop app", "cmd.exe"]);
        let ps = command(ExternalTerminal::PowerShell, &dir, Path::new(".")).unwrap();
        assert!(ps[7].to_ascii_lowercase().contains("powershell") || ps[7].to_ascii_lowercase().contains("pwsh"));
        assert!(installed(ExternalTerminal::Terminal).is_some(), "Windows Terminal, or PowerShell in its place");
    }

    #[test]
    fn a_remote_folder_opens_an_ssh_session_there() {
        let ssh: Vec<String> = ["ssh", "-t", "box", "cd /srv; exec $SHELL -l"].map(String::from).into();
        let at = Where::Remote { dir: PathBuf::from("/srv"), ssh: ssh.clone() };
        let cmd = command(ExternalTerminal::CommandPrompt, &at, Path::new(".")).unwrap();
        assert_eq!(cmd[5..], ssh[..]);
        assert_eq!(quote(r"C:\a b"), "\"C:\\a b\"");
        assert_eq!(quote(r"C:\ab"), r"C:\ab");
        assert_eq!(start_quote("/D"), "/D");
        assert_eq!(start_quote(""), "\"\"");
    }

    #[test]
    fn a_mac_app_is_not_offered() {
        assert!(!APPS.contains(&ExternalTerminal::ITerm));
        assert!(command(ExternalTerminal::Warp, &Where::Local(PathBuf::from(r"C:\")), Path::new(".")).is_err());
    }
}
