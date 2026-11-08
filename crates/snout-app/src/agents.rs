//! The coding agents SnoutCode knows, apart from how each one tells the app what it does.
//! An [`Agent`] has a name, a colour of its own (its caret, its marks in the gutter, its
//! dots on tabs and files) and an icon, and a [`Channel`]: how the app learns of its turns
//! and its edits.
//!
//! - Claude Code: the IDE connection (an MCP server it finds through a lock file) and its
//!   hooks, which report every edit before and after, and the prompt of each turn.
//! - Codex: its hooks, set for the session, which report every edit before and after and
//!   the prompt of each turn; its `notify` program, which hears the end of each turn with
//!   the prompt that started it; and the file watcher while its turn runs in the console.
//! - opencode and Amp: a plugin of SnoutCode's, which reports every edit before and after
//!   and the prompts, and the file watcher while the turn runs.
//! - Any other command line agent run in the console: the file watcher, while its turn runs
//!   (the console busy after Return).

use std::path::{Path, PathBuf};

use gpui::{App, Global, Hsla, SharedString};
use serde::{Deserialize, Serialize};

use crate::components::IconName;
use crate::theme::{Palette, hex};

pub const CLAUDE: &str = "claude";
pub const CODEX: &str = "codex";
pub const OPENCODE: &str = "opencode";
pub const AMP: &str = "amp";
pub const CUSTOM: &str = "custom";

/// How the app learns what an agent does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Claude Code's IDE connection and hooks: every edit before and after, and its turns.
    ClaudeIde,
    /// The agent's own hooks (Codex's), set for the session: every edit before and after,
    /// and its turns.
    Hooks,
    /// A plugin of SnoutCode's in the agent (opencode, Amp): every edit before and after,
    /// and its turns.
    Plugin,
    /// The file watcher while the console's turn runs.
    Watcher,
}

/// Which agent the console runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleAgent {
    /// Claude Code (`claude`).
    #[default]
    Claude,
    /// OpenAI's Codex CLI (`codex`).
    Codex,
    /// opencode (`opencode`).
    Opencode,
    /// Amp (`amp`).
    Amp,
    /// A command of your own (`custom_agent_command`).
    Custom,
}

impl ConsoleAgent {
    pub const ALL: [ConsoleAgent; 5] =
        [ConsoleAgent::Claude, ConsoleAgent::Codex, ConsoleAgent::Opencode, ConsoleAgent::Amp, ConsoleAgent::Custom];

    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => CLAUDE,
            Self::Codex => CODEX,
            Self::Opencode => OPENCODE,
            Self::Amp => AMP,
            Self::Custom => CUSTOM,
        }
    }
}

/// A coding agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub id: &'static str,
    /// Its name in full ("Claude Code"), and as a caret's flag says it ("Claude").
    pub name: SharedString,
    pub short: SharedString,
    pub icon: IconName,
    /// The one-character mark the gutter draws beside its lines.
    pub glyph: char,
    pub channel: Channel,
    /// Its colour in the light and the dark themes.
    light: u32,
    dark: u32,
    /// The command a custom agent runs.
    pub command: Option<String>,
}

/// Claude's colour: the terracotta of its mark.
pub const CLAUDE_LIGHT: u32 = crate::theme::CLAUDE_LIGHT;
pub const CLAUDE_DARK: u32 = crate::theme::CLAUDE_DARK;

impl Agent {
    pub fn claude() -> Self {
        Self {
            id: CLAUDE,
            name: "Claude Code".into(),
            short: "Claude".into(),
            icon: IconName::Sparkles,
            glyph: '\u{2733}',
            channel: Channel::ClaudeIde,
            light: CLAUDE_LIGHT,
            dark: CLAUDE_DARK,
            command: None,
        }
    }

    pub fn codex() -> Self {
        Self {
            id: CODEX,
            name: "Codex".into(),
            short: "Codex".into(),
            icon: IconName::Hexagon,
            glyph: '\u{2B22}',
            channel: Channel::Hooks,
            light: 0x6E56CF,
            dark: 0xA594F0,
            command: None,
        }
    }

    pub fn opencode() -> Self {
        Self {
            id: OPENCODE,
            name: "opencode".into(),
            short: "opencode".into(),
            icon: IconName::SquareCode,
            glyph: '\u{25A0}',
            channel: Channel::Plugin,
            light: 0x2F7D32,
            dark: 0x6CC070,
            command: None,
        }
    }

    pub fn amp() -> Self {
        Self {
            id: AMP,
            name: "Amp".into(),
            short: "Amp".into(),
            icon: IconName::Zap,
            glyph: '\u{25B2}',
            channel: Channel::Plugin,
            light: 0xC2255C,
            dark: 0xF06595,
            command: None,
        }
    }

    /// The agent reports each of its edits, before and after, and starts its turns itself
    /// (its prompt reported as it is sent): the watcher does not start them.
    pub fn reports_prompts(&self) -> bool {
        reports_prompts(self.id)
    }

    /// The program's name, as SnoutCode looks for it.
    pub fn program(&self) -> Option<&'static str> {
        match self.id {
            CLAUDE => Some("claude"),
            CODEX => Some("codex"),
            OPENCODE => Some("opencode"),
            AMP => Some("amp"),
            _ => None,
        }
    }

    /// A command of the user's own, named `name` (its program's name when empty).
    pub fn custom(name: &str, command: &str) -> Self {
        let name = Some(name.trim()).filter(|n| !n.is_empty()).map(str::to_owned).unwrap_or_else(|| {
            command.split_whitespace().next().and_then(|p| Path::new(p).file_name()).map_or_else(
                || "Agent".to_owned(),
                |n| {
                    let n = n.to_string_lossy();
                    let mut c = n.chars();
                    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
                },
            )
        });
        Self {
            id: CUSTOM,
            name: name.clone().into(),
            short: name.into(),
            icon: IconName::Bot,
            glyph: '\u{25C6}',
            channel: Channel::Watcher,
            light: 0x0F8A80,
            dark: 0x4CC5BA,
            command: Some(command.to_owned()),
        }
    }

    /// Its colour in the theme `p`.
    pub fn color(&self, p: &Palette) -> Hsla {
        hex(if p.dark { self.dark } else { self.light })
    }

    /// The agent behind `id`, as the settings name it.
    pub fn of(id: &str, s: &crate::settings::Settings) -> Self {
        match id {
            CODEX => Self::codex(),
            OPENCODE => Self::opencode(),
            AMP => Self::amp(),
            CUSTOM => Self::custom(&s.custom_agent_name, &s.custom_agent_command),
            _ => Self::claude(),
        }
    }
}

/// The agent `id` reports its prompts as it sends them, and so starts its turns itself
/// (Claude Code's and Codex's hooks, SnoutCode's plugin in opencode). Amp's plugin hears
/// a turn's prompt only as it ends.
pub fn reports_prompts(id: &str) -> bool {
    matches!(id, CLAUDE | CODEX | OPENCODE)
}

/// The agent behind `id` as the window knows it (a custom agent with the name the
/// settings give it).
pub fn by_id(id: &str, cx: &App) -> Agent {
    let console = console_agent(cx);
    if console.id == id {
        return console;
    }
    Agent::of(id, &crate::settings::Settings::default())
}

/// Every agent the app knows, for the settings.
pub fn known(s: &crate::settings::Settings) -> Vec<Agent> {
    vec![
        Agent::claude(),
        Agent::codex(),
        Agent::opencode(),
        Agent::amp(),
        Agent::custom(&s.custom_agent_name, &s.custom_agent_command),
    ]
}

/// The agent the console runs, as the settings say.
pub fn of_settings(s: &crate::settings::Settings) -> Agent {
    Agent::of(s.console_agent.id(), s)
}

/// The agent the console of the window runs (the app's settings; Claude Code before they
/// are known).
pub fn console_agent(cx: &App) -> Agent {
    cx.try_global::<ConsoleAgentNow>().map(|a| a.0.clone()).unwrap_or_else(Agent::claude)
}

/// The agent the console runs now: set from the settings as they change.
#[derive(Clone, Debug)]
pub struct ConsoleAgentNow(pub Agent);

impl Global for ConsoleAgentNow {}

/// What runs in the console's agent session now: whose turn, since when, and whether the
/// agent is at work (it printed after the last Return and has not gone quiet).
#[derive(Clone, Debug, Default)]
pub struct ConsoleTurn {
    pub agent: Option<&'static str>,
    pub active: bool,
    /// The folder the session runs in.
    pub root: Option<PathBuf>,
    /// When the turn began (the Return) and when the agent last printed.
    pub since: Option<std::time::Instant>,
    pub last_output: Option<std::time::Instant>,
    /// When the last turn ended.
    pub ended: Option<std::time::Instant>,
}

impl Global for ConsoleTurn {}

/// How long after a turn ends the files that change are still put down to its agent (its
/// last writes land as it finishes).
pub const TURN_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

impl ConsoleTurn {
    /// The agent is at work, or stopped a moment ago.
    pub fn at_work(&self) -> bool {
        self.active || self.ended.is_some_and(|t| t.elapsed() < TURN_GRACE)
    }
}

/// The agent a change on disk is put down to now (its turn runs in the console, or Claude
/// Code is connected): an open tab plays such a change out in its colour.
#[derive(Clone, Debug, Default)]
pub struct Watching(pub Option<Agent>);

impl Global for Watching {}

/// Whether the editor follows the agents' edits (the console's Follow).
#[derive(Clone, Copy, Debug)]
pub struct Following(pub bool);

impl Global for Following {}

/// The agent's program `name` (`codex`, `opencode`, `amp`) on the host's PATH or where
/// its installers put it (npm, bun, Homebrew, opencode's and Amp's install scripts).
pub fn find_agent(host: &dyn snout_host::Host, name: &str) -> Option<PathBuf> {
    let home = &host.info().home;
    let extra: Vec<PathBuf> =
        [".local/bin", ".npm-global/bin", ".bun/bin", ".cargo/bin", ".volta/bin", ".opencode/bin", ".amp/bin"]
            .iter()
            .filter(|_| !home.as_os_str().is_empty())
            .map(|d| home.join(d))
            .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from))
            .chain(windows_program_dirs(host.info()))
            .collect();
    snout_host::which(host, &[name], &extra)
}

/// Where Windows's package managers put what they install: npm's global folder, pnpm's, Volta's,
/// Scoop's and winget's. Each is often on the PATH only of programs started after it was installed.
fn windows_program_dirs(info: &snout_host::HostInfo) -> Vec<PathBuf> {
    if !info.is_windows() {
        return Vec::new();
    }
    let under = |var: &str, rest: &str| info.var(var).map(|d| PathBuf::from(d).join(rest));
    [
        under("APPDATA", "npm"),
        under("LOCALAPPDATA", "pnpm"),
        under("LOCALAPPDATA", "Volta\\bin"),
        under("LOCALAPPDATA", "Microsoft\\WinGet\\Links"),
        under("USERPROFILE", "scoop\\shims"),
        under("NVM_SYMLINK", ""),
        under("ProgramFiles", "nodejs"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Where the agent `id` is: `configured` (its path in the settings; none when it names a
/// file that is not there), else on the host's PATH or where its installers put it, else
/// where the user's shell finds it (a login, interactive shell: npm's folder under nvm is
/// on its PATH only there). The custom agent runs through the shell and is not looked for.
pub fn locate(host: &dyn snout_host::Host, id: &str, configured: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = configured {
        return (!path.is_absolute() || host.is_file(&path)).then_some(path);
    }
    let name = program_name(id)?;
    let found = if id == CLAUDE { snout_core::claude::find_claude(host) } else { find_agent(host, name) };
    found.or_else(|| from_shell(host, name))
}

/// How long the user's shell gets to name a program: a profile that hangs, or waits for
/// something, must not leave the console starting for ever.
const SHELL_LOOKUP: std::time::Duration = std::time::Duration::from_secs(6);

/// The PATH for a program started now on Windows: this app's, then the user's and the machine's as
/// they are now, for what was installed since this app started. None elsewhere, or if it cannot say.
pub fn windows_fresh_path(host: &dyn snout_host::Host) -> Option<String> {
    let info = host.info();
    if !info.is_windows() {
        return None;
    }
    let script =
        "[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::GetEnvironmentVariable('Path','Machine')";
    let cmd = snout_host::Cmd::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
        .stdout(snout_host::Stdio::Piped)
        .group();
    let out = output_within(host, cmd, SHELL_LOOKUP)?;
    let mut dirs: Vec<String> = info.path_dirs().iter().map(|d| d.display().to_string()).collect();
    merge_path(&mut dirs, &String::from_utf8_lossy(&out));
    Some(dirs.join(";"))
}

/// `found` (lines of `;`-separated folders) after `dirs`, each folder once, whatever its case.
fn merge_path(dirs: &mut Vec<String>, found: &str) {
    for dir in found.lines().flat_map(|l| l.split(';')).map(str::trim).filter(|d| !d.is_empty()) {
        if !dirs.iter().any(|d| d.eq_ignore_ascii_case(dir)) {
            dirs.push(dir.to_owned());
        }
    }
}

/// What PowerShell runs to name the program `name`: on the PATH the user has now, not the one this
/// app started with, which misses whatever was installed since, and after their profile has run
/// (fnm and the like add to the PATH there). Programs only: npm's `.ps1` shim cannot be started.
fn windows_lookup_script(name: &str) -> String {
    format!(
        "$env:Path = $env:Path + ';' + [Environment]::GetEnvironmentVariable('Path','User') + ';' + \
         [Environment]::GetEnvironmentVariable('Path','Machine'); \
         (Get-Command {name} -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source"
    )
}

/// The program a shell named: the last line that is a path to a file (a profile can print anything).
fn program_in_output(text: &str, windows: bool, is_file: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let absolute = |l: &str| {
        if windows { l.starts_with("\\\\") || l.as_bytes().get(1) == Some(&b':') } else { l.starts_with('/') }
    };
    text.lines().rev().map(str::trim).filter(|l| absolute(l)).map(PathBuf::from).find(|p| is_file(p))
}

/// What `cmd` prints within `limit`, or nothing. On a thread of its own: what a profile left running
/// can hold the pipe open.
fn output_within(host: &dyn snout_host::Host, cmd: snout_host::Cmd, limit: std::time::Duration) -> Option<Vec<u8>> {
    let mut child = host.spawn(&cmd).ok()?;
    let mut stdout = child.take_stdout()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut out);
        let _ = tx.send(out);
    });
    let out = rx.recv_timeout(limit);
    let _ = child.kill();
    let _ = child.wait();
    out.ok()
}

/// The program `name` as the user's shell finds it, if the shell says within [`SHELL_LOOKUP`].
fn from_shell(host: &dyn snout_host::Host, name: &str) -> Option<PathBuf> {
    let windows = host.info().is_windows();
    let cmd = if windows {
        snout_host::Cmd::new("powershell.exe")
            .args(["-NoLogo", "-NonInteractive", "-Command"])
            .arg(windows_lookup_script(name))
    } else {
        snout_host::Cmd::new(crate::console::default_shell(host)).arg("-ilc").arg(format!("command -v {name}"))
    };
    let cmd = cmd.stdout(snout_host::Stdio::Piped).group();
    let out = output_within(host, cmd, SHELL_LOOKUP)?;
    program_in_output(&String::from_utf8_lossy(&out), windows, |p| host.is_file(p))
}

/// What installs the agent `id` on a host like `info`: a line for its shell (PowerShell on
/// Windows). None for the custom agent.
pub fn install_command(id: &str, info: &snout_host::HostInfo) -> Option<&'static str> {
    install_line(id, info.is_windows())
}

/// What installs the agent `id`: in PowerShell on Windows, else in a Unix shell.
pub fn install_line(id: &str, windows: bool) -> Option<&'static str> {
    Some(match (id, windows) {
        (CLAUDE, false) => "curl -fsSL https://claude.ai/install.sh | bash",
        (CLAUDE, true) => "irm https://claude.ai/install.ps1 | iex",
        (CODEX, _) => "npm install -g @openai/codex",
        (OPENCODE, false) => "curl -fsSL https://opencode.ai/install | bash",
        (OPENCODE, true) => "npm install -g opencode-ai",
        (AMP, false) => "curl -fsSL https://ampcode.com/install.sh | bash",
        (AMP, true) => "irm https://ampcode.com/install.ps1 | iex",
        _ => return None,
    })
}

/// Whether the agent `id`'s makers would have it run inside WSL on Windows: Amp's say "The CLI
/// supports macOS, Linux, and Windows through WSL". The console looks for it in the distros
/// too, and offers WSL before Windows itself.
pub fn wsl_first(id: &str) -> bool {
    id == AMP
}

/// The program the agent `id` runs as, for all but the custom agent.
pub fn program_name(id: &str) -> Option<&'static str> {
    Some(match id {
        CLAUDE => "claude",
        CODEX => "codex",
        OPENCODE => "opencode",
        AMP => "amp",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_s_answer_is_the_last_line_that_is_a_file() {
        let is_file = |p: &Path| !p.to_string_lossy().contains("gone");
        let unix = "Welcome back\n/home/ann/gone/amp\n/home/ann/.nvm/bin/amp\nnot a path\n";
        assert_eq!(program_in_output(unix, false, is_file), Some(PathBuf::from("/home/ann/.nvm/bin/amp")));
        let win = "profile says hello\r\nC:\\Users\\ann\\AppData\\Roaming\\npm\\amp.cmd\r\n";
        let found = program_in_output(win, true, is_file);
        assert_eq!(found, Some(PathBuf::from(r"C:\Users\ann\AppData\Roaming\npm\amp.cmd")));
        assert_eq!(program_in_output("just talking\n", true, is_file), None);
        assert_eq!(program_in_output("/home/ann/gone/amp\n", false, is_file), None);
    }

    #[test]
    fn a_fresh_path_keeps_this_apps_folders_first_and_adds_each_new_one_once() {
        let mut dirs = vec![r"C:\Windows".to_owned()];
        merge_path(&mut dirs, "C:\\Users\\a\\AppData\\Roaming\\npm;c:\\windows;\r\nC:\\Program Files\\nodejs\r\n");
        assert_eq!(dirs, [r"C:\Windows", r"C:\Users\a\AppData\Roaming\npm", r"C:\Program Files\nodejs"]);
    }

    #[test]
    fn windows_is_asked_for_a_program_on_the_path_the_user_has_now() {
        let script = windows_lookup_script("amp");
        assert!(script.contains("GetEnvironmentVariable('Path','User')"), "{script}");
        assert!(script.contains("Get-Command amp -CommandType Application"), "{script}");
        assert!(!script.contains('"'), "it goes as one argument: {script}");
    }

    #[test]
    fn each_agent_has_its_own_colour_and_name() {
        let s = crate::settings::Settings::default();
        let all = known(&s);
        assert_eq!(all.iter().map(|a| a.id).collect::<Vec<_>>(), [CLAUDE, CODEX, OPENCODE, AMP, CUSTOM]);
        assert_eq!(ConsoleAgent::ALL.map(ConsoleAgent::id), [CLAUDE, CODEX, OPENCODE, AMP, CUSTOM]);
        for dark in [false, true] {
            let p = crate::theme::Palette::new(crate::settings::ColorTheme::Classic, dark);
            let colours: Vec<Hsla> = all.iter().map(|a| a.color(&p)).collect();
            for (i, a) in colours.iter().enumerate() {
                assert!(colours[i + 1..].iter().all(|b| b != a), "each agent has a colour of its own");
            }
        }
        let glyphs: std::collections::HashSet<char> = all.iter().map(|a| a.glyph).collect();
        assert_eq!(glyphs.len(), all.len(), "and a mark of its own");
        assert_eq!(Agent::of(AMP, &s).channel, Channel::Plugin);
        assert_eq!(Agent::of(OPENCODE, &s).program(), Some("opencode"));
        assert_eq!(Agent::custom("", "/opt/bin/aider --model x").name, "Aider");
        assert_eq!(Agent::custom("Gemini", "gemini").short, "Gemini");
        assert_eq!(of_settings(&s).id, CLAUDE, "Claude Code by default");
    }

    #[test]
    fn each_agent_but_the_custom_one_has_an_installer() {
        let mut info = snout_host::HostInfo::default();
        for os in ["macos", "windows"] {
            info.os = os.into();
            for id in [CLAUDE, CODEX, OPENCODE, AMP] {
                assert!(install_command(id, &info).is_some(), "{id} on {os}");
            }
            assert_eq!(install_command(CUSTOM, &info), None);
        }
    }

    #[test]
    fn amp_on_windows_is_installed_by_its_own_installer_and_wsl_comes_first() {
        assert_eq!(install_line(AMP, true), Some("irm https://ampcode.com/install.ps1 | iex"));
        assert_eq!(install_line(AMP, false), Some("curl -fsSL https://ampcode.com/install.sh | bash"));
        assert!(wsl_first(AMP));
        assert!(!wsl_first(CLAUDE) && !wsl_first(CODEX) && !wsl_first(OPENCODE));
    }

    #[test]
    fn the_status_bar_names_every_agent_with_changes_waiting() {
        use crate::claude_ide::AgentSummary;
        let s = crate::settings::Settings::default();
        let short = |id: &'static str| Agent::of(id, &s).short.to_string();
        let one = AgentSummary::of(&[CLAUDE]);
        assert_eq!(one.text(short), "Claude changed 1 file");
        // The one that changed the most first; Claude first of equals.
        let two = AgentSummary::of(&[CLAUDE, OPENCODE, OPENCODE, OPENCODE]);
        assert_eq!(two.agents, [OPENCODE, CLAUDE]);
        assert_eq!(two.text(short), "opencode and Claude changed 4 files");
        assert_eq!(AgentSummary::of(&[OPENCODE, CLAUDE]).agents, [CLAUDE, OPENCODE]);
        let three = AgentSummary::of(&[CODEX, AMP, CLAUDE, AMP, CODEX]);
        assert_eq!(three.text(short), "3 agents changed 5 files");
        assert_eq!(AgentSummary::of(&[]), AgentSummary::default());
    }
}
