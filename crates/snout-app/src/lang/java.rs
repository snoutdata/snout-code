//! What the Java engine needs around it: where its distribution is, which Gradle or Maven
//! build a file belongs to, which JDK that build needs, the JDKs installed, and downloading one
//! from Adoptium when none fits. All of it on the workspace's [`Host`]: blocking, so off the
//! UI thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use snout_host::{Cmd, Host, Stdio};

/// The engine distribution on `host`: `SNOUT_ENGINE_DIST`, else among the app's resources
/// (`Contents/Resources/engine` in the Mac's app bundle, `/usr/lib/snout-code/engine` in a
/// Linux package, see [`crate::platform::resource_dirs`]), else `engine/build/dist` of the
/// workspace the binary was built in (development).
pub fn engine_dist(host: &dyn Host) -> Option<PathBuf> {
    let ok = |d: &Path| host.is_file(&java_bin(host.info(), &d.join("runtime"))) && host.is_dir(&d.join("lib"));
    if let Some(d) = host.info().var("SNOUT_ENGINE_DIST").map(PathBuf::from) {
        return ok(&d).then_some(d);
    }
    // On a remote machine: the copy SnoutCode installed there (see `remote_engine`).
    if is_remote(host) {
        return super::remote_engine::installed(host);
    }
    let exe = host.info().exe.clone()?;
    if let Some(found) = crate::platform::find_resource(&exe, "engine", "engine/build/dist", ok) {
        return Some(found);
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/build/dist");
    ok(&built).then(|| host.canonicalize(&built).unwrap_or(built))
}

/// The command that runs `snout-engine lsp` on `project` with the distribution's own Java,
/// Gradle running on `jdk`, its Gradle daemons stopping after `idle_minutes` without a build,
/// and Maven builds read with `maven_profiles` turned on (`a`) or off (`!a`).
pub fn engine_command(
    host: &dyn Host,
    dist: &Path,
    project: &Path,
    jdk: &Path,
    idle_minutes: u64,
    maven_profiles: &[String],
) -> Cmd {
    let mut c = Cmd::new(java_bin(host.info(), &dist.join("runtime")))
        .arg("-cp")
        .arg(engine_classpath(dist))
        .arg("com.snoutcode.engine.cli.SnoutEngine")
        .arg("lsp")
        .arg(project)
        .arg("--java-home")
        .arg(jdk)
        // The cache the app looks in for the model the engine writes (the same as the
        // engine's own default, unless HOME points elsewhere).
        .arg("--cache-dir")
        .arg(engine_cache(host))
        .arg("--daemon-idle-minutes")
        .arg(idle_minutes.max(1).to_string())
        .cwd(project);
    if !maven_profiles.is_empty() {
        c = c.arg("--maven-profiles").arg(maven_profiles.join(","));
    }
    // The JDKs the app downloaded, for builds that want another Java than `jdk`.
    for extra in downloaded_jdks(host) {
        if extra.home != jdk {
            c = c.arg("--jdk").arg(extra.home);
        }
    }
    c
}

/// The JDKs the app downloaded, under the user's home on `host`.
pub fn downloaded_jdks(host: &dyn Host) -> Vec<Jdk> {
    jdks_within(host, &downloads_dir(host.info()), 2)
}

/// SnoutCode's cache folder on `host`, where the engine keeps its builds, models and jdtls data.
pub fn engine_cache(host: &dyn Host) -> PathBuf {
    crate::project_model::cache_dir(host.info())
}

const SETTINGS: [&str; 2] = ["settings.gradle", "settings.gradle.kts"];
const BUILDS: [&str; 2] = ["build.gradle", "build.gradle.kts"];
/// A Maven build's file.
pub const POM: &str = "pom.xml";

/// Whether `dir` is the root of a build the engine reads: a Gradle build (it has a settings
/// or build script) or a Maven one (it has a `pom.xml`).
pub fn is_gradle_root(host: &dyn Host, dir: &Path) -> bool {
    SETTINGS.iter().chain(BUILDS.iter()).any(|f| host.is_file(&dir.join(f))) || host.is_file(&dir.join(POM))
}

/// Whether `dir` is a Maven build: a `pom.xml` and no Gradle script (a folder with both is
/// a Gradle build, as the engine reads it).
pub fn is_maven_root(host: &dyn Host, dir: &Path) -> bool {
    host.is_file(&dir.join(POM)) && !SETTINGS.iter().chain(BUILDS.iter()).any(|f| host.is_file(&dir.join(f)))
}

/// The build `file` belongs to: the nearest folder above it with a Gradle settings script,
/// else the nearest with a build script, else the Maven reactor of the nearest `pom.xml`
/// (the outermost POM that lists the one below among its modules), looking no higher than
/// `stop`.
pub fn gradle_root(host: &dyn Host, file: &Path, stop: Option<&Path>) -> Option<PathBuf> {
    let mut nearest_build = None;
    let mut nearest_pom: Option<PathBuf> = None;
    for dir in file.ancestors().skip(1) {
        if SETTINGS.iter().any(|f| host.is_file(&dir.join(f))) {
            return Some(dir.to_owned());
        }
        if nearest_build.is_none() && BUILDS.iter().any(|f| host.is_file(&dir.join(f))) {
            nearest_build = Some(dir.to_owned());
        }
        if nearest_pom.is_none() && host.is_file(&dir.join(POM)) {
            nearest_pom = Some(dir.to_owned());
        }
        if stop.is_some_and(|s| dir == s) {
            break;
        }
    }
    nearest_build.or_else(|| nearest_pom.map(|d| maven_reactor(host, &d, stop)))
}

/// The outermost folder above `dir` (a Maven module) whose POM lists the one below among
/// its modules, looking no higher than `stop`; `dir` itself when none does.
pub fn maven_reactor(host: &dyn Host, dir: &Path, stop: Option<&Path>) -> PathBuf {
    let mut at = dir.to_owned();
    while stop.is_none_or(|s| at != s)
        && let Some(up) = at.parent()
        && lists_module(host, up, &at)
    {
        at = up.to_owned();
    }
    at
}

/// Whether the POM in `parent` names `child` in a `<module>` (in any profile).
fn lists_module(host: &dyn Host, parent: &Path, child: &Path) -> bool {
    let Ok(text) = host.read_to_string(&parent.join(POM)) else { return false };
    let mut rest = text.as_str();
    while let Some(at) = rest.find("<module>") {
        rest = &rest[at + 8..];
        let Some(end) = rest.find("</module>") else { break };
        let name = rest[..end].trim().trim_end_matches("/pom.xml").trim_end_matches('/');
        if normalize(&parent.join(name)) == normalize(child) {
            return true;
        }
        rest = &rest[end..];
    }
    false
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// An installed JDK.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jdk {
    pub home: PathBuf,
    /// The feature release: 8, 11, 17, 21…
    pub major: u32,
    pub version: String,
}

/// The `java` of the JDK (or runtime) at `home` on the host `info` describes: `bin/java`, on
/// Windows `bin\java.exe`.
pub fn java_bin(info: &snout_host::HostInfo, home: &Path) -> PathBuf {
    home.join("bin").join(if info.is_windows() { "java.exe" } else { "java" })
}

/// The engine's class path: every jar in the distribution's `lib` (a wildcard Java expands).
pub fn engine_classpath(dist: &Path) -> PathBuf {
    dist.join("lib").join("*")
}

/// The JDK at `home`, read from its `release` file.
pub fn jdk_at(host: &dyn Host, home: &Path) -> Option<Jdk> {
    if !host.is_file(&java_bin(host.info(), home)) {
        return None;
    }
    let release = host.read_to_string(&home.join("release")).ok()?;
    let version =
        release.lines().find_map(|l| l.strip_prefix("JAVA_VERSION=")).map(|v| v.trim().trim_matches('"').to_owned())?;
    let major = major_of(&version)?;
    Some(Jdk { home: home.to_owned(), major, version })
}

/// 17 for "17.0.2", 8 for "1.8.0_392".
pub fn major_of(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// The JDK homes in a folder of JDKs: each entry, or its `Contents/Home` (macOS bundles).
pub fn jdks_in(host: &dyn Host, dir: &Path) -> Vec<Jdk> {
    jdks_within(host, dir, 1)
}

/// [`jdks_in`], looking `depth` folders down: Gradle's toolchains, and the JDKs IntelliJ IDEA
/// and SnoutCode unpack, keep a JDK inside its archive's own folder
/// (`~/.gradle/jdks/eclipse_adoptium-17-aarch64-os_x.2/jdk-17.0.20.1+1/Contents/Home`).
fn jdks_within(host: &dyn Host, dir: &Path, depth: u32) -> Vec<Jdk> {
    let Ok(entries) = host.read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries {
        let p = e.path;
        match [p.join("Contents/Home"), p.clone()].iter().find_map(|h| jdk_at(host, h)) {
            Some(jdk) => out.push(jdk),
            None if depth > 1 && e.is_dir => out.extend(jdks_within(host, &p, depth - 1)),
            None => {}
        }
    }
    out
}

/// The JDKs IntelliJ IDEA and Android Studio know of: the homes of their JDK tables (wherever
/// the user picked them), under the IDEs' settings folders for this host's system.
fn jetbrains_jdks(host: &dyn Host, home: &Path) -> Vec<PathBuf> {
    let info = host.info();
    let bases: Vec<PathBuf> = if info.is_mac() {
        vec![home.join("Library/Application Support/JetBrains"), home.join("Library/Application Support/Google")]
    } else if info.is_windows() {
        info.var("APPDATA")
            .map(|a| vec![Path::new(a).join("JetBrains"), Path::new(a).join("Google")])
            .unwrap_or_default()
    } else {
        vec![home.join(".config/JetBrains"), home.join(".config/Google")]
    };
    let mut out = Vec::new();
    for base in bases {
        let Ok(ides) = host.read_dir(&base) else { continue };
        for ide in ides.into_iter().filter(|e| e.is_dir) {
            let Ok(table) = host.read_to_string(&ide.path.join("options/jdk.table.xml")) else { continue };
            for part in table.split("<homePath value=\"").skip(1) {
                let Some(path) = part.split('"').next() else { continue };
                let path = path.replace("$USER_HOME$", &home.to_string_lossy());
                out.push(PathBuf::from(path));
            }
        }
    }
    out
}

/// The JDKs package managers keep where `jdks_in` cannot list them: Homebrew's (in a formula's
/// `libexec`), and jenv's (links to the others, by version).
fn packaged_jdks(host: &dyn Host, home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for opt in ["/opt/homebrew/opt", "/usr/local/opt", "/home/linuxbrew/.linuxbrew/opt"] {
        let Ok(entries) = host.read_dir(Path::new(opt)) else { continue };
        for e in entries {
            let name = e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.contains("openjdk") || name.contains("temurin") || name.contains("java") {
                out.push(e.path.join("libexec/openjdk.jdk/Contents/Home"));
                out.push(e.path.join("libexec"));
            }
        }
    }
    if let Ok(entries) = host.read_dir(&home.join(".jenv/versions")) {
        out.extend(entries.into_iter().filter_map(|e| host.canonicalize(&e.path).ok()));
    }
    out
}

/// Where SnoutCode keeps the JDKs it downloads on a host: in its data folder on the machine
/// the app runs on (Application Support on a Mac, `~/.local/share/SnoutCode` on Linux), in
/// `~/.snout-code/jdks` on a remote machine (beside the server and the engine SnoutCode
/// installs there).
pub fn downloads_dir(info: &snout_host::HostInfo) -> PathBuf {
    downloads_under(info, &info.home)
}

/// [`downloads_dir`] for the home folder `home` on the host `info` describes.
fn downloads_under(info: &snout_host::HostInfo, home: &Path) -> PathBuf {
    if info.pid == std::process::id() {
        let var = |name: &str| if home == info.home { info.var(name).map(str::to_owned) } else { None };
        snout_host::dirs::AppDirs::for_os(&info.os, home, var).data.join("jdks")
    } else {
        home.join(".snout-code/jdks")
    }
}

/// Every JDK `host` has where JDKs usually are. On a Mac: what `java_home` lists, the
/// user's and the system's JavaVirtualMachines. On Linux: the `java` on `PATH`, and the
/// distributions' folders (`/usr/lib/jvm`, …). Everywhere: `JAVA_HOME` (of the login shell),
/// the JDKs IntelliJ IDEA and Android Studio were given, theirs (`~/.jdks`), SDKMAN, asdf,
/// mise, jenv, Homebrew, Gradle's toolchains, and SnoutCode's own.
pub fn discover(host: &dyn Host, home: &Path) -> Vec<Jdk> {
    let mut found: Vec<Jdk> =
        host.info().var("JAVA_HOME").and_then(|h| jdk_at(host, Path::new(h))).into_iter().collect();
    for h in jetbrains_jdks(host, home).into_iter().chain(packaged_jdks(host, home)) {
        found.extend(jdk_at(host, &h));
    }
    if !host.info().is_mac() {
        let mut homes: Vec<PathBuf> = Vec::new();
        if let Some(java) = snout_host::which(host, &["java"], &[])
            && let Ok(real) = host.canonicalize(&java)
            && let Some(home) = real.parent().and_then(Path::parent)
        {
            homes.push(home.to_owned());
        }
        found.extend(homes.iter().filter_map(|h| jdk_at(host, h)));
        if host.info().is_windows() {
            // Where the JDK installers put them: under Program Files, by vendor.
            let program_files: Vec<PathBuf> = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
                .iter()
                .filter_map(|v| host.info().var(v).map(PathBuf::from))
                .collect();
            for base in program_files {
                for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "Amazon Corretto", "BellSoft", "Semeru"]
                {
                    found.extend(jdks_in(host, &base.join(vendor)));
                }
            }
        } else {
            for dir in ["/usr/lib/jvm", "/usr/java", "/usr/local/java", "/opt/java", "/opt/jdk"] {
                found.extend(jdks_in(host, Path::new(dir)));
            }
        }
    } else if let Ok(out) = host.run(&Cmd::new("/usr/libexec/java_home").arg("-V"), None) {
        // It lists on stderr: `    17.0.2 (arm64) "Vendor" - "Name" /path/to/home`.
        for line in String::from_utf8_lossy(&out.stderr).lines() {
            if let Some(at) = line.find(" /")
                && let Some(jdk) = jdk_at(host, Path::new(line[at + 1..].trim()))
            {
                found.push(jdk);
            }
        }
    }
    let mac = host.info().is_mac();
    let mac_dirs = [home.join("Library/Java/JavaVirtualMachines"), PathBuf::from("/Library/Java/JavaVirtualMachines")];
    for dir in mac_dirs.into_iter().filter(|_| mac).chain([
        home.join(".sdkman/candidates/java"),
        home.join(".asdf/installs/java"),
        home.join(".local/share/mise/installs/java"),
    ]) {
        found.extend(jdks_in(host, &dir));
    }
    for dir in [home.join(".jdks"), home.join(".gradle/jdks"), downloads_under(host.info(), home)] {
        found.extend(jdks_within(host, &dir, 2));
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|j| seen.insert(host.canonicalize(&j.home).unwrap_or_else(|_| j.home.clone())));
    found
}

/// [`discover`], and the JDKs added by hand at `added` (their homes) that it did not find.
pub fn discover_with(host: &dyn Host, home: &Path, added: &[PathBuf]) -> Vec<Jdk> {
    let mut found = discover(host, home);
    for h in added {
        if let Some(jdk) = jdk_at(host, h)
            && !found.iter().any(|j| j.home == jdk.home)
        {
            found.push(jdk);
        }
    }
    found
}

/// What a build asks of the JDK running Gradle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requirement {
    /// The newest Java level its scripts ask for (source compatibility, toolchain, release).
    pub declared: Option<u32>,
    /// The newest Java its Gradle version runs on.
    pub gradle_max: Option<u32>,
}

impl Requirement {
    /// The JDK to download when none fits.
    pub fn wanted(&self) -> u32 {
        self.wanted_on(cfg!(target_arch = "aarch64"))
    }

    /// [`Self::wanted`] on a Mac with Apple silicon (`arm`) or not. Temurin publishes no
    /// Java 8 for Apple silicon: there a Java 8 build gets 11, the oldest it publishes, which
    /// builds and runs Java 8 code.
    pub fn wanted_on(&self, arm: bool) -> u32 {
        // A long-term support release: the newest the build's Gradle runs on.
        let lts = |max: u32| [25, 21, 17, 11, 8].into_iter().find(|v| *v <= max.min(21)).unwrap_or(8);
        let want = self.declared.or(self.gradle_max.map(lts)).unwrap_or(21);
        if arm && want < 11 && self.gradle_max.is_none_or(|m| m >= 11) { 11 } else { want }
    }

    pub fn fits(&self, major: u32) -> bool {
        self.declared.is_none_or(|d| major >= d) && self.gradle_max.is_none_or(|m| major <= m)
    }
}

/// The newest Java each Gradle release runs on, from Gradle's compatibility matrix.
pub fn gradle_max_java(gradle: &str) -> Option<u32> {
    let mut parts = gradle.split(['.', '-']).map(|p| p.parse::<u32>().unwrap_or(0));
    let v = (parts.next()?, parts.next().unwrap_or(0));
    let table = [
        ((9, 1), 25),
        ((8, 14), 24),
        ((8, 10), 23),
        ((8, 8), 22),
        ((8, 5), 21),
        ((8, 3), 20),
        ((7, 6), 19),
        ((7, 5), 18),
        ((7, 3), 17),
        ((7, 0), 16),
        ((6, 7), 15),
        ((6, 3), 14),
        ((6, 0), 13),
        ((5, 4), 12),
        ((5, 0), 11),
    ];
    table.iter().find(|(at, _)| v >= *at).map(|(_, j)| *j)
}

/// The Java levels a build script names, in the forms Gradle scripts use.
pub fn declared_levels(script: &str) -> Vec<u32> {
    let keys = [
        "JavaVersion.VERSION_",
        "sourceCompatibility",
        "targetCompatibility",
        "JavaLanguageVersion.of(",
        "jvmToolchain(",
        "release.set(",
        "options.release",
        "jvmTarget",
    ];
    let mut out = Vec::new();
    for key in keys {
        let mut rest = script;
        while let Some(at) = rest.find(key) {
            rest = &rest[at + key.len()..];
            let tail: String = rest.chars().take(24).take_while(|c| *c != '\n').collect();
            let digits: String = tail
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '_')
                .collect();
            if let Some(level) = major_of(&digits.replace('_', ".")).filter(|l| (6..=40).contains(l)) {
                out.push(level);
            }
        }
    }
    out
}

/// What the build at `root` needs: the levels its scripts declare (its own and its
/// subprojects', one level down) and its Gradle wrapper's version.
pub fn requirement(host: &dyn Host, root: &Path) -> Requirement {
    let mut scripts: Vec<PathBuf> = SETTINGS.iter().chain(BUILDS.iter()).map(|f| root.join(f)).collect();
    if let Ok(entries) = host.read_dir(root) {
        for e in entries {
            if e.is_dir {
                scripts.extend(BUILDS.iter().map(|f| e.path.join(f)));
            }
        }
    }
    let declared = scripts.iter().filter_map(|p| host.read_to_string(p).ok()).flat_map(|s| declared_levels(&s)).max();
    let wrapper = host.read_to_string(&root.join("gradle/wrapper/gradle-wrapper.properties")).unwrap_or_default();
    let gradle = wrapper
        .lines()
        .find_map(|l| l.strip_prefix("distributionUrl="))
        .and_then(|url| url.rsplit('/').next())
        .and_then(|name| name.strip_prefix("gradle-"))
        .map(|v| v.trim_end_matches(".zip").trim_end_matches("-bin").trim_end_matches("-all").to_owned());
    Requirement { declared, gradle_max: gradle.as_deref().and_then(gradle_max_java) }
}

/// The JDK to run the build on: the declared level exactly if installed, else the oldest
/// newer one Gradle still runs on; with nothing declared, the newest Gradle runs on.
pub fn choose<'a>(req: &Requirement, jdks: &'a [Jdk]) -> Option<&'a Jdk> {
    let mut fit: Vec<&Jdk> = jdks.iter().filter(|j| req.fits(j.major)).collect();
    match req.declared {
        Some(d) => {
            fit.sort_by_key(|j| (j.major != d, j.major));
            fit.first().copied()
        }
        None => fit.into_iter().max_by_key(|j| j.major),
    }
}

/// How a download is going: bytes so far, and the total once known.
#[derive(Clone, Default)]
pub struct DownloadProgress {
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
}

impl DownloadProgress {
    pub fn percent(&self) -> Option<u32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| ((self.done.load(Ordering::Relaxed) * 100) / total).min(100) as u32)
    }
}

/// Whether `host` is another machine than the one the app runs on.
pub fn is_remote(host: &dyn Host) -> bool {
    host.info().pid != std::process::id()
}

/// The text at `url`, fetched on `host` with curl (else wget).
pub(crate) fn fetch_text(host: &dyn Host, url: &str) -> anyhow::Result<Vec<u8>> {
    let curl = Cmd::new("curl").args(["-fsSL", "--max-time", "60", url]);
    let wget = Cmd::new("wget").args(["-q", "-T", "60", "-O", "-", url]);
    let mut last = anyhow::anyhow!("neither curl nor wget is installed");
    for cmd in [curl, wget] {
        match host.run(&cmd, None) {
            Ok(out) if out.status.success() => return Ok(out.stdout),
            Ok(out) if out.status.code == Some(127) => {}
            Ok(out) => last = anyhow::anyhow!("{}", String::from_utf8_lossy(&out.stderr).trim().to_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => last = e.into(),
        }
    }
    Err(last)
}

/// Fetch `url` into `out` on `host` with curl (else wget), counting its bytes in `progress`.
pub(crate) fn fetch_file(host: &dyn Host, url: &str, out: &Path, progress: &DownloadProgress) -> anyhow::Result<()> {
    let curl = Cmd::new("curl").args(["-fsSL", "-o"]).arg(out).arg(url);
    let wget = Cmd::new("wget").args(["-q", "-O"]).arg(out).arg(url);
    let mut last = anyhow::anyhow!("neither curl nor wget is installed");
    for cmd in [curl, wget] {
        let mut child = match host.spawn(&cmd.stdout(Stdio::Null).stderr(Stdio::Null)) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if let Ok(m) = host.metadata(out) {
                progress.done.store(m.len, Ordering::Relaxed);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        if status.success() {
            return Ok(());
        }
        let _ = host.remove_file(out);
        if status.code == Some(127) {
            continue;
        }
        last = anyhow::anyhow!("the download failed ({status})");
        break;
    }
    Err(last)
}

/// The size of the file at `url`, as its server's last answer (after redirects) gives it.
fn remote_size(host: &dyn Host, url: &str) -> Option<u64> {
    let out = host.run(&Cmd::new("curl").args(["-sIL", "--max-time", "15"]).arg(url), None).ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .filter_map(|(_, v)| v.trim().parse().ok())
        .next_back()
}

/// The SHA-256 of `path` on `host`, in hex: read here on the machine the app runs on, else
/// with the host's `sha256sum` or `shasum`.
pub fn sha256_on(host: &dyn Host, path: &Path) -> anyhow::Result<String> {
    if !is_remote(host) {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        std::io::copy(&mut std::fs::File::open(path)?, &mut hasher)?;
        return Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect());
    }
    for cmd in [Cmd::new("sha256sum").arg(path), Cmd::new("shasum").args(["-a", "256"]).arg(path)] {
        if let Ok(out) = host.run(&cmd, None)
            && out.status.success()
        {
            let hash = String::from_utf8_lossy(&out.stdout).split_whitespace().next().unwrap_or_default().to_owned();
            return Ok(hash.to_ascii_lowercase());
        }
    }
    anyhow::bail!("neither sha256sum nor shasum could read {}", path.display())
}

/// Copy the local file `from` to `to` on `host`, in pieces, counting them in `progress`.
pub fn upload(host: &dyn Host, from: &Path, to: &Path, progress: &DownloadProgress) -> anyhow::Result<()> {
    use std::io::Read as _;
    const PIECE: usize = 4 << 20;
    let mut file = std::fs::File::open(from)?;
    progress.total.store(file.metadata()?.len(), Ordering::Relaxed);
    progress.done.store(0, Ordering::Relaxed);
    let _ = host.remove_file(to);
    host.write(to, b"")?;
    let mut buf = vec![0u8; PIECE];
    loop {
        let mut n = 0;
        while n < PIECE {
            let got = file.read(&mut buf[n..])?;
            if got == 0 {
                break;
            }
            n += got;
        }
        if n == 0 {
            return Ok(());
        }
        host.append(to, &buf[..n])?;
        progress.done.fetch_add(n as u64, Ordering::Relaxed);
    }
}

/// Download Eclipse Temurin `major` for `host` from api.adoptium.net into `dir` there, check
/// its SHA-256, unpack it, and return the new JDK. A remote machine fetches it itself; one
/// that cannot reach the internet gets it fetched on this Mac and copied over. Blocking;
/// run it off the main thread.
pub fn download_temurin(host: &dyn Host, major: u32, dir: &Path, progress: &DownloadProgress) -> anyhow::Result<Jdk> {
    use anyhow::Context as _;
    let arch = if host.info().arch == "aarch64" { "aarch64" } else { "x64" };
    let os = if host.info().is_mac() { "mac" } else { host.info().os.as_str() };
    let api = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
    );
    let remote = is_remote(host);
    let local = snout_host::local();
    let answer = match fetch_text(host, &api) {
        Ok(a) => a,
        Err(_) if remote => fetch_text(&*local, &api).context("ask Adoptium")?,
        Err(e) => return Err(e.context("ask Adoptium")),
    };
    let assets: serde_json::Value = serde_json::from_slice(&answer).context("read Adoptium's answer")?;
    let package = assets.get(0).and_then(|a| a.pointer("/binary/package")).context("no JDK for this machine")?;
    let link = package.get("link").and_then(|v| v.as_str()).context("no download link")?;
    let sha = package.get("checksum").and_then(|v| v.as_str()).unwrap_or_default().to_ascii_lowercase();
    let size = package.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    install(host, &format!("temurin-{major}"), link, &sha, size, dir, progress)
}

/// Fetch the JDK archive at `link` onto `host` (its SHA-256 `sha` when known, `size` bytes),
/// unpack it into `dir`, and return the JDK in it. `tag` names its scratch files. A remote
/// machine fetches it itself; one that cannot reach the internet gets it fetched on this Mac
/// and copied over. Blocking.
fn install(
    host: &dyn Host,
    tag: &str,
    link: &str,
    sha: &str,
    size: u64,
    dir: &Path,
    progress: &DownloadProgress,
) -> anyhow::Result<Jdk> {
    use anyhow::{Context as _, bail};
    let remote = is_remote(host);
    let local = snout_host::local();
    // A vendor that says no size: its server does, so the progress can show a percentage.
    let size = if size > 0 { size } else { remote_size(host, link).unwrap_or(0) };
    progress.total.store(size, Ordering::Relaxed);
    let zip = link.ends_with(".zip");
    host.create_dir_all(dir)?;
    // Windows's JDKs come as zip files, the others' as gzipped tarballs; Windows's own `tar`
    // (bsdtar) unpacks both.
    let part = dir.join(format!(".{tag}.{}.part", if zip { "zip" } else { "tar.gz" }));
    if let Err(e) = fetch_file(host, link, &part, progress) {
        if !remote {
            return Err(e);
        }
        // The remote machine cannot fetch it: this Mac does, and copies it over.
        let tmp = tempfile::Builder::new().prefix("snout-jdk").tempdir()?;
        let file = tmp.path().join("jdk.archive");
        fetch_file(&*local, link, &file, progress)
            .with_context(|| format!("download the JDK on {}", crate::platform::this_machine()))?;
        if !sha.is_empty() && sha256_on(&*local, &file)? != sha {
            bail!("the download is damaged (its checksum does not match)");
        }
        upload(host, &file, &part, progress).context("copy the JDK to the remote machine")?;
    }
    progress.done.store(progress.total.load(Ordering::Relaxed).max(size), Ordering::Relaxed);
    if !sha.is_empty() && sha256_on(host, &part)? != sha {
        let _ = host.remove_file(&part);
        bail!("the download is damaged (its checksum does not match)");
    }
    let unpack = dir.join(format!(".{tag}.unpack"));
    let _ = host.remove_dir_all(&unpack);
    host.create_dir_all(&unpack)?;
    let flags = if zip { "-xf" } else { "-xzf" };
    let ok = host.run(&Cmd::new("tar").arg(flags).arg(&part).arg("-C").arg(&unpack), None)?.status.success();
    let _ = host.remove_file(&part);
    if !ok {
        bail!("the download could not be unpacked");
    }
    let top = host.read_dir(&unpack)?.into_iter().find(|e| e.is_dir).map(|e| e.path).context("an empty archive")?;
    let name = top.file_name().map(|n| n.to_owned()).context("an unnamed folder")?;
    let dest = dir.join(name);
    let _ = host.remove_dir_all(&dest);
    host.rename(&top, &dest)?;
    let _ = host.remove_dir_all(&unpack);
    // The JDK is the folder, its `Contents/Home`, or (Zulu's, Liberica's) a bundle inside it.
    [dest.join("Contents/Home"), dest.clone()]
        .iter()
        .find_map(|h| jdk_at(host, h))
        .or_else(|| jdks_within(host, &dest, 1).into_iter().next())
        .context("the download holds no JDK")
}

/// The vendors the Download JDK dialog offers, by their foojay id, in the order it lists
/// them, with the name it shows.
pub const VENDORS: [(&str, &str); 12] = [
    ("temurin", "Eclipse Temurin"),
    ("corretto", "Amazon Corretto"),
    ("zulu", "Azul Zulu"),
    ("liberica", "BellSoft Liberica"),
    ("graalvm_community", "GraalVM Community"),
    ("oracle_open_jdk", "Oracle OpenJDK"),
    ("oracle", "Oracle Java"),
    ("microsoft", "Microsoft Build of OpenJDK"),
    ("sap_machine", "SapMachine"),
    ("semeru", "IBM Semeru"),
    ("jetbrains", "JetBrains Runtime"),
    ("dragonwell", "Alibaba Dragonwell"),
];

/// A JDK one can download: a vendor's latest build of a Java version, for this host's system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JdkPackage {
    /// The vendor's foojay id (`temurin`), see [`VENDORS`].
    pub vendor: String,
    pub major: u32,
    /// "21.0.9+10".
    pub version: String,
    /// Its foojay package id, which [`download_package`] fetches.
    pub id: String,
    /// The archive's file name.
    pub file: String,
    /// Long-term support ("lts"), or not.
    pub lts: bool,
}

impl JdkPackage {
    /// The vendor's name as the dialog shows it.
    pub fn vendor_name(&self) -> &str {
        VENDORS.iter().find(|(id, _)| *id == self.vendor).map_or(self.vendor.as_str(), |(_, n)| n)
    }
}

const FOOJAY: &str = "https://api.foojay.io/disco/v3.0";

/// The JDKs one can download for `host`, from the foojay catalogue (the one IntelliJ IDEA and
/// Gradle's toolchains use): each vendor's latest of each Java version, as a plain JDK
/// archive (no JavaFX, no musl). Newest first, then by vendor. Blocking.
pub fn jdk_catalog(host: &dyn Host) -> anyhow::Result<Vec<JdkPackage>> {
    use anyhow::Context as _;
    let info = host.info();
    let os = if info.is_mac() {
        "macos"
    } else if info.is_windows() {
        "windows"
    } else {
        "linux"
    };
    let arch = if info.arch == "aarch64" { "aarch64" } else { "x64" };
    let archive = if info.is_windows() { "zip" } else { "tar.gz" };
    let url = format!(
        "{FOOJAY}/packages?os={os}&architecture={arch}&archive_type={archive}&package_type=jdk\
         &latest=available&release_status=ga&directly_downloadable=true"
    );
    let answer = match fetch_text(host, &url) {
        Ok(a) => a,
        Err(_) if is_remote(host) => fetch_text(&*snout_host::local(), &url).context("ask foojay")?,
        Err(e) => return Err(e.context("ask foojay")),
    };
    let json: serde_json::Value = serde_json::from_slice(&answer).context("read foojay's answer")?;
    Ok(catalog_from(&json, os))
}

/// [`jdk_catalog`] from foojay's answer `json`, for the system `os` ("macos", "linux", …).
pub fn catalog_from(json: &serde_json::Value, os: &str) -> Vec<JdkPackage> {
    let mut out: Vec<JdkPackage> = Vec::new();
    for p in json.get("result").and_then(|r| r.as_array()).into_iter().flatten() {
        let text = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_owned();
        let vendor = text("distribution");
        let plain = p.get("javafx_bundled").and_then(|v| v.as_bool()) == Some(false)
            && p.get("feature").and_then(|v| v.as_array()).is_none_or(|f| f.is_empty())
            && (os != "linux" || text("lib_c_type") == "glibc");
        let Some(major) = p.get("major_version").and_then(|v| v.as_u64()).map(|m| m as u32) else { continue };
        if !plain || !VENDORS.iter().any(|(id, _)| *id == vendor) {
            continue;
        }
        let package = JdkPackage {
            vendor,
            major,
            version: text("java_version"),
            id: text("id"),
            file: text("filename"),
            lts: text("term_of_support") == "lts",
        };
        // One per vendor and version: the newest build.
        match out.iter_mut().find(|q| q.vendor == package.vendor && q.major == package.major) {
            Some(q) if version_key(&package.version) > version_key(&q.version) => *q = package,
            Some(_) => {}
            None => out.push(package),
        }
    }
    let rank = |v: &str| VENDORS.iter().position(|(id, _)| *id == v).unwrap_or(usize::MAX);
    out.sort_by(|a, b| b.major.cmp(&a.major).then(rank(&a.vendor).cmp(&rank(&b.vendor))));
    out
}

/// "21.0.9+10" as numbers, to compare builds.
fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse().ok()).collect()
}

/// Download the foojay package `package` into `dir` on `host`, check it and unpack it, as
/// [`download_temurin`] does. Blocking.
pub fn download_package(
    host: &dyn Host,
    package: &JdkPackage,
    dir: &Path,
    progress: &DownloadProgress,
) -> anyhow::Result<Jdk> {
    use anyhow::Context as _;
    let url = format!("{FOOJAY}/ids/{}", package.id);
    let answer = match fetch_text(host, &url) {
        Ok(a) => a,
        Err(_) if is_remote(host) => fetch_text(&*snout_host::local(), &url).context("ask foojay")?,
        Err(e) => return Err(e.context("ask foojay")),
    };
    let json: serde_json::Value = serde_json::from_slice(&answer).context("read foojay's answer")?;
    let info = json.pointer("/result/0").context("no such JDK")?;
    let link = info.get("direct_download_uri").and_then(|v| v.as_str()).context("no download link")?;
    let sha = match info.get("checksum_type").and_then(|v| v.as_str()) {
        Some("sha256") => info.get("checksum").and_then(|v| v.as_str()).unwrap_or_default().to_ascii_lowercase(),
        _ => String::new(),
    };
    let size = info.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    let tag = format!("{}-{}", package.vendor, package.major);
    install(host, &tag, link, &sha, size, dir, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_java_levels_from_build_scripts() {
        let groovy = "java {\n\tsourceCompatibility = '17'\n}\ntargetCompatibility = JavaVersion.VERSION_1_8\n";
        assert_eq!(declared_levels(groovy), [8, 17]);
        let kts = "kotlin { jvmToolchain(21) }\njava { toolchain { languageVersion = JavaLanguageVersion.of(11) } }";
        let mut l = declared_levels(kts);
        l.sort();
        assert_eq!(l, [11, 21]);
        assert_eq!(declared_levels("sourceCompatibility = 1.8"), [8]);
        assert!(declared_levels("version = '1.0.3'").is_empty());
    }

    #[test]
    fn the_catalogue_keeps_one_plain_jdk_per_vendor_and_version() {
        let pkg = |dist: &str, major: u64, version: &str, fx: bool, lts: &str| {
            serde_json::json!({ "id": format!("{dist}-{version}"), "distribution": dist, "major_version": major,
                "java_version": version, "javafx_bundled": fx, "feature": [], "lib_c_type": "libc",
                "filename": "x.tar.gz", "term_of_support": lts })
        };
        let answer = serde_json::json!({ "result": [
            pkg("zulu", 21, "21.0.8+9", false, "lts"),
            pkg("zulu", 21, "21.0.9+10", false, "lts"),
            pkg("liberica", 21, "21.0.9+11", true, "lts"),
            pkg("temurin", 21, "21.0.9+10", false, "lts"),
            pkg("temurin", 24, "24.0.2+12", false, "sts"),
            pkg("trava", 11, "11.0.15+1", false, "lts"),
        ]});
        let got: Vec<(String, u32, String)> =
            catalog_from(&answer, "macos").into_iter().map(|p| (p.vendor, p.major, p.version)).collect();
        assert_eq!(
            got,
            [
                ("temurin".into(), 24, "24.0.2+12".into()),
                ("temurin".into(), 21, "21.0.9+10".into()),
                ("zulu".into(), 21, "21.0.9+10".into()),
            ],
            "newest version first, Temurin first, the newest build, no JavaFX, no vendor it does not list"
        );
    }

    #[test]
    fn knows_which_java_each_gradle_runs_on() {
        assert_eq!(gradle_max_java("8.1.1"), Some(19));
        assert_eq!(gradle_max_java("8.10.2"), Some(23));
        assert_eq!(gradle_max_java("7.3"), Some(17));
        assert_eq!(gradle_max_java("6.9.4"), Some(15));
        assert_eq!(gradle_max_java("4.10"), None);
        assert_eq!(major_of("1.8.0_392"), Some(8));
        assert_eq!(major_of("17.0.21"), Some(17));
    }

    fn jdk(major: u32) -> Jdk {
        Jdk { home: PathBuf::from(format!("/j/{major}")), major, version: format!("{major}.0.1") }
    }

    #[test]
    fn chooses_the_declared_jdk_or_the_nearest_newer_gradle_runs_on() {
        let jdks = [jdk(11), jdk(17), jdk(21), jdk(25)];
        let bamboo = Requirement { declared: Some(17), gradle_max: Some(19) };
        assert_eq!(choose(&bamboo, &jdks).unwrap().major, 17);
        assert_eq!(choose(&bamboo, &[jdk(11), jdk(21)]), None, "21 is too new for Gradle 8.1");
        assert_eq!(bamboo.wanted(), 17);
        let loose = Requirement { declared: Some(11), gradle_max: Some(23) };
        assert_eq!(choose(&loose, &[jdk(17), jdk(21)]).unwrap().major, 17);
        let none = Requirement { declared: None, gradle_max: Some(23) };
        assert_eq!(choose(&none, &jdks).unwrap().major, 21);
        assert_eq!(Requirement::default().wanted(), 21);
        // Java 8 on Apple silicon: 11, which Temurin has for it.
        let eight = Requirement { declared: Some(8), gradle_max: Some(23) };
        assert_eq!((eight.wanted_on(false), eight.wanted_on(true)), (8, 11));
        assert!(eight.fits(11));
    }

    #[test]
    fn finds_the_gradle_build_of_a_file_and_what_it_needs() {
        let dir = std::env::temp_dir().join(format!("snout-gradle-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("app/src/main/java/demo");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(dir.join("gradle/wrapper")).unwrap();
        std::fs::write(dir.join("settings.gradle"), "include 'app'\n").unwrap();
        std::fs::write(dir.join("app/build.gradle"), "java { sourceCompatibility = '17' }\n").unwrap();
        std::fs::write(
            dir.join("gradle/wrapper/gradle-wrapper.properties"),
            "distributionUrl=https\\://services.gradle.org/distributions/gradle-8.1.1-bin.zip\n",
        )
        .unwrap();
        assert_eq!(gradle_root(&*snout_host::local(), &src.join("App.java"), None), Some(dir.clone()));
        assert_eq!(requirement(&*snout_host::local(), &dir), Requirement { declared: Some(17), gradle_max: Some(19) });
        assert_eq!(gradle_root(&*snout_host::local(), &std::env::temp_dir().join("nowhere/X.java"), None), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
