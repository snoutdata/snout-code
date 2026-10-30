//! What the host says about its processes: one snapshot of every process of the user
//! (parent, command line, memory, CPU, age), the engine's Gradle daemons as their logs show
//! them (busy or idle, since when), and stopping a process the snapshot saw.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use snout_host::{Host, HostRef};

/// The class every Gradle daemon runs, on its command line.
pub const DAEMON_MAIN: &str = "org.gradle.launcher.daemon.bootstrap.GradleDaemon";

/// The engine's main class, on its command line.
pub const ENGINE_MAIN: &str = "com.snoutcode.engine.cli.SnoutEngine";

/// What jdtls's command line has that nothing else has: the model the engine wrote for it.
pub const JDTLS_MARK: &str = "-Dsnout.model=";

/// One process, as the snapshot saw it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cmd: Vec<String>,
    /// Resident memory, in bytes.
    pub memory: u64,
    /// Percent of one core, since the snapshot before.
    pub cpu: f32,
    /// Seconds since 1970 when it started.
    pub start: u64,
}

impl Proc {
    pub fn has_arg(&self, arg: &str) -> bool {
        self.cmd.iter().any(|a| a == arg)
    }

    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.cmd.iter().any(|a| a.starts_with(prefix))
    }

    /// The argument after `flag`.
    pub fn arg_after(&self, flag: &str) -> Option<&str> {
        self.cmd.iter().position(|a| a == flag).and_then(|i| self.cmd.get(i + 1)).map(String::as_str)
    }

    /// The command line's words after the program, joined.
    pub fn line(&self) -> String {
        self.cmd.join(" ")
    }

    pub fn is_engine(&self) -> bool {
        self.has_arg(ENGINE_MAIN)
    }

    pub fn is_jdtls(&self) -> bool {
        self.has_prefix(JDTLS_MARK)
    }

    pub fn is_daemon(&self) -> bool {
        self.has_arg(DAEMON_MAIN)
    }

    /// The folder an engine serves (`snout-engine lsp <folder>` or `sync <folder>`).
    pub fn engine_folder(&self) -> Option<&str> {
        let at = self.cmd.iter().position(|a| a == ENGINE_MAIN)?;
        match self.cmd.get(at + 1).map(String::as_str) {
            Some("lsp" | "sync") => self.cmd.get(at + 2).map(String::as_str),
            _ => None,
        }
    }
}

/// One of the engine's Gradle daemons, from its log in the registry.
#[derive(Clone, Debug, PartialEq)]
pub struct DaemonSeen {
    pub pid: u32,
    /// The registry's folder name: the hash of the project folder it serves.
    pub registry: String,
    pub version: String,
    pub busy: bool,
    /// Seconds since 1970 when it last turned busy or idle, as its log says.
    pub since: Option<u64>,
}

/// Every process of the moment, and the engine's daemons among them.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub procs: HashMap<u32, Proc>,
    pub daemons: Vec<DaemonSeen>,
    /// Seconds since 1970.
    pub now: u64,
}

impl Snapshot {
    pub fn get(&self, pid: u32) -> Option<&Proc> {
        self.procs.get(&pid)
    }

    /// The processes whose parent is `pid`, oldest first.
    pub fn children(&self, pid: u32) -> Vec<&Proc> {
        let mut out: Vec<&Proc> = self.procs.values().filter(|p| p.parent == Some(pid)).collect();
        out.sort_by_key(|p| (p.start, p.pid));
        out
    }

    /// Whether `p`'s parent is gone: none, the system's init, or a process no longer there.
    pub fn orphan(&self, p: &Proc) -> bool {
        match p.parent {
            None | Some(0 | 1) => true,
            Some(parent) => !self.procs.contains_key(&parent),
        }
    }
}

/// A Gradle daemon log, as far as it was read.
#[derive(Clone, Debug, Default)]
struct LogState {
    offset: u64,
    busy: bool,
    since: Option<u64>,
}

/// Takes snapshots of a host; it keeps what the one before saw of the daemon logs (the host
/// keeps what it needs to measure the CPU use).
pub struct Scanner {
    host: HostRef,
    logs: HashMap<PathBuf, LogState>,
}

impl Scanner {
    pub fn new(host: HostRef) -> Self {
        Self { host, logs: HashMap::new() }
    }

    /// Every process now, with the daemons of the registries under `cache`.
    pub fn snapshot(&mut self, cache: &Path) -> Snapshot {
        self.take(cache, true)
    }

    /// The same, without waiting to measure the CPU use (it reads 0).
    pub fn quick_snapshot(&mut self, cache: &Path) -> Snapshot {
        self.take(cache, false)
    }

    fn take(&mut self, cache: &Path, cpu: bool) -> Snapshot {
        let procs: HashMap<u32, Proc> = self
            .host
            .processes(cpu)
            .unwrap_or_default()
            .into_iter()
            .map(|p| {
                let proc = Proc {
                    pid: p.pid,
                    parent: p.parent,
                    name: p.name,
                    cmd: p.cmd,
                    memory: p.memory,
                    cpu: p.cpu,
                    start: p.start,
                };
                (p.pid, proc)
            })
            .collect();
        let daemons = self.daemons(cache, &procs);
        Snapshot { procs, daemons, now: now_secs() }
    }

    /// The daemons of every registry under `cache`: each log `<registry>/<Gradle version>/
    /// daemon-<pid>.out.log` whose process runs Gradle's daemon and started before the log
    /// was last written (a later process that got the pid of a daemon gone since does not
    /// count: a daemon writes its log as it stops).
    fn daemons(&mut self, cache: &Path, procs: &HashMap<u32, Proc>) -> Vec<DaemonSeen> {
        let host = self.host.clone();
        let host = &*host;
        let mut out = Vec::new();
        let mut seen = Vec::new();
        for registry in dirs(host, &cache.join("daemons")) {
            let hash = file_name(&registry);
            for version in dirs(host, &registry) {
                let Ok(files) = host.read_dir(&version) else { continue };
                for file in files.into_iter().map(|e| e.path) {
                    let name = file_name(&file);
                    let Some(pid) = name
                        .strip_prefix("daemon-")
                        .and_then(|r| r.strip_suffix(".out.log"))
                        .and_then(|p| p.parse().ok())
                    else {
                        continue;
                    };
                    let Some(proc) = procs.get(&pid).filter(|p| p.is_daemon()) else { continue };
                    let stat = host.metadata(&file).ok();
                    let written = stat
                        .as_ref()
                        .and_then(|m| m.modified)
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs());
                    if proc.start > written + 1 {
                        continue;
                    }
                    let state = self.logs.entry(file.clone()).or_default();
                    read_log(host, &file, stat.map_or(0, |m| m.len), state);
                    seen.push(file.clone());
                    out.push(DaemonSeen {
                        pid,
                        registry: hash.clone(),
                        version: file_name(&version),
                        busy: state.busy,
                        since: state.since.or(Some(proc.start)),
                    });
                }
            }
        }
        self.logs.retain(|f, _| seen.contains(f));
        out.sort_by_key(|d| (d.registry.clone(), d.version.clone(), d.pid));
        out
    }
}

/// Read what the log (now `len` bytes long) gained since `state.offset`: the last time the
/// daemon was marked busy or idle wins.
fn read_log(host: &dyn Host, file: &Path, len: u64, state: &mut LogState) {
    if len < state.offset {
        *state = LogState::default();
    }
    if len == state.offset {
        return;
    }
    let Ok(bytes) = host.read_range(file, state.offset, len - state.offset) else { return };
    // Only whole lines: the rest waits for the next read.
    let Some(end) = bytes.iter().rposition(|b| *b == b'\n') else { return };
    state.offset += end as u64 + 1;
    for line in String::from_utf8_lossy(&bytes[..=end]).lines() {
        let busy = if line.contains("Marking the daemon as busy") {
            true
        } else if line.contains("Marking the daemon as idle") {
            false
        } else {
            continue;
        };
        state.busy = busy;
        state.since = log_time(line).or(state.since);
    }
}

/// The time a Gradle log line starts with (`2026-10-28T16:16:57.446-0400`), in seconds since
/// 1970.
pub fn log_time(line: &str) -> Option<u64> {
    let b = line.as_bytes();
    if b.len() < 28 || b[4] != b'-' || b[10] != b'T' || b[19] != b'.' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| line.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let sign = match b[23] {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let offset = sign * (num(24..26)? * 3600 + num(26..28)? * 60);
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - offset;
    u64::try_from(secs).ok()
}

/// Days since 1970-01-01 of a date of the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn dirs(host: &dyn Host, dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = host.read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries.into_iter().filter(|e| e.is_dir).map(|e| e.path).collect();
    out.sort();
    out
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The name of the engine's daemon registry for a project folder on `host`: the first 16 hex
/// digits of the SHA-256 of its canonical path, as the engine names it.
pub fn registry_hash(host: &dyn Host, folder: &Path) -> String {
    use sha2::Digest as _;
    let path = host.canonicalize(folder).unwrap_or_else(|_| folder.to_owned());
    let digest = sha2::Sha256::digest(path.to_string_lossy().as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Stop the processes `(pid, start)` of `host`: ask each to end (SIGTERM), and kill those
/// still there after `grace`. A process whose start time is not the one given (its pid was
/// taken by another since) is left alone. Blocks for up to `grace`; returns how many it stopped.
pub fn stop(host: &dyn Host, targets: &[(u32, u64)], grace: Duration) -> usize {
    host.stop_processes(targets, grace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradle_log_times_are_read_with_their_offset() {
        // 2026-10-28T20:16:57Z
        assert_eq!(log_time("2026-10-28T16:16:57.446-0400 [INFO] x"), Some(1_793_218_617));
        assert_eq!(log_time("2026-10-28T20:16:57.446+0000 [INFO] x"), Some(1_793_218_617));
        assert_eq!(log_time("1970-01-01T00:00:00.000+0000"), Some(0));
        assert_eq!(log_time("not a time"), None);
    }

    #[test]
    fn a_daemon_log_tells_busy_from_idle() {
        let dir = std::env::temp_dir().join(format!("snout-daemon-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("daemon-1.out.log");
        let busy = "2026-10-28T16:16:55.711-0400 [INFO] [org.gradle.launcher.daemon.server.DaemonRegistryUpdater] Marking the daemon as busy, address: x\n";
        let idle = "2026-10-28T16:16:57.446-0400 [INFO] [org.gradle.launcher.daemon.server.DaemonRegistryUpdater] Marking the daemon as idle, address: x\n";
        std::fs::write(&log, busy).unwrap();
        let mut state = LogState::default();
        let host = snout_host::local();
        let len = |log: &Path| std::fs::metadata(log).unwrap().len();
        read_log(&*host, &log, len(&log), &mut state);
        assert!(state.busy);
        assert_eq!(state.since, Some(1_793_218_615));
        // Only what it gained is read; half a line waits.
        std::fs::write(&log, format!("{busy}{idle}2026-10-28T16:17")).unwrap();
        read_log(&*host, &log, len(&log), &mut state);
        assert!(!state.busy);
        assert_eq!(state.since, Some(1_793_218_617));
        assert_eq!(state.offset as usize, busy.len() + idle.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_registry_is_named_as_the_engine_names_it() {
        // sha256("/") starts 8a5edab282632443.
        assert_eq!(registry_hash(&*snout_host::local(), Path::new("/")), "8a5edab282632443");
    }

    #[test]
    fn an_engine_command_line_names_its_folder() {
        let p = Proc {
            cmd: ["java", "-cp", "x", ENGINE_MAIN, "lsp", "/p", "--cache-dir", "/c"].map(String::from).to_vec(),
            ..Default::default()
        };
        assert!(p.is_engine());
        assert_eq!(p.engine_folder(), Some("/p"));
        assert_eq!(p.arg_after("--cache-dir"), Some("/c"));
    }
}
