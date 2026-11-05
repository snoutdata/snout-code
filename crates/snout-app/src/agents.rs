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
            // Windows: npm's global folder.
            .chain(host.info().var("APPDATA").map(|d| PathBuf::from(d).join("npm")))
            .collect();
    snout_host::which(host, &[name], &extra)
}

/// Where the agent `id` is: `configured` (its path in the settings; none when it names a
/// file that is not there), else on the host's PATH or where its installers put it, else
/// where the user's shell finds it (a login, interactive shell: npm's folder under nvm is
/// on its PATH only there). The custom agent runs through the shell and is not looked for.
pub fn locate(host: &dyn snout_host::Host, id: &str, configured: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = configured {
        return (!path.is_absolute() || host.is_file(&path)).then_some(path);
    }
    let name = match id {
        CLAUDE => "claude",
        CODEX => "codex",
        OPENCODE => "opencode",
        AMP => "amp",
        _ => return None,
    };
    let found = if id == CLAUDE { snout_core::claude::find_claude(host) } else { find_agent(host, name) };
    found.or_else(|| from_shell(host, name))
}

/// The program `name` as the user's shell finds it (not on Windows).
fn from_shell(host: &dyn snout_host::Host, name: &str) -> Option<PathBuf> {
    if host.info().is_windows() {
        return None;
    }
    let shell = crate::console::default_shell(host);
    let cmd = snout_host::Cmd::new(&shell).arg("-ilc").arg(format!("command -v {name}"));
    let out = host.run(&cmd, None).ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().rev().map(str::trim).filter(|l| l.starts_with('/')).map(PathBuf::from).find(|p| host.is_file(p))
}

/// What installs the agent `id` on a host like `info`: a line for its shell (PowerShell on
/// Windows). None for the custom agent.
pub fn install_command(id: &str, info: &snout_host::HostInfo) -> Option<&'static str> {
    Some(match (id, info.is_windows()) {
        (CLAUDE, false) => "curl -fsSL https://claude.ai/install.sh | bash",
        (CLAUDE, true) => "irm https://claude.ai/install.ps1 | iex",
        (CODEX, _) => "npm install -g @openai/codex",
        (OPENCODE, false) => "curl -fsSL https://opencode.ai/install | bash",
        (OPENCODE, true) => "npm install -g opencode-ai",
        (AMP, false) => "curl -fsSL https://ampcode.com/install.sh | bash",
        (AMP, true) => "npm install -g @sourcegraph/amp",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
