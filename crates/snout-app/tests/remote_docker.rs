//! The engine on a real Linux machine: an Ubuntu container with sshd, git and no JDK (see
//! `crates/snout-server/tests/docker`), reached with the system's ssh, a scratch key and a
//! scratch ssh config. The app's `RemoteHost` installs the engine there (the remote cannot
//! download Java itself here: this Mac fetches it and copies it over), finds the JDK it put
//! there, and syncs the container's Gradle project with the engine.
//!
//! Runs only with `SNOUT_REMOTE_DOCKER_TEST=1` (it needs Docker, the Linux server binaries
//! and the engine built, and it downloads a JDK and Gradle):
//!
//! ```sh
//! SNOUT_REMOTE_DOCKER_TEST=1 cargo test -p snout-app --test remote_docker -- --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use snout_host::Host as _;
use snout_remote::{RemoteHost, RemoteSession, SshTarget};

const IMAGE: &str = "snout-remote-test:2";

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn run(cmd: &mut Command) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
    assert!(out.status.success(), "{cmd:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}

#[test]
fn the_engine_installs_and_syncs_on_a_linux_machine() {
    if std::env::var_os("SNOUT_REMOTE_DOCKER_TEST").is_none() {
        eprintln!("skipped: set SNOUT_REMOTE_DOCKER_TEST=1 to run it (needs Docker)");
        return;
    }
    let Some(binaries) = snout_app::remote::server_binaries() else {
        panic!("no Linux server binaries: run scripts/build-server-linux.sh");
    };
    assert!(snout_app::lang::java::engine_dist(&*snout_host::local()).is_some(), "build the engine (snout.sh)");

    let scratch = tempfile::Builder::new().prefix("snt").tempdir().unwrap();
    let dir = scratch.path();
    let key = dir.join("key");
    run(Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-C", "snout-test", "-f"]).arg(&key));
    let public = std::fs::read_to_string(dir.join("key.pub")).unwrap();
    run(Command::new("docker")
        .args(["build", "-q", "-t", IMAGE])
        .arg(workspace().join("crates/snout-server/tests/docker")));
    let name = format!("snout-engine-test-{}", std::process::id());
    let container = Container(name.clone());
    run(Command::new("docker")
        .args(["run", "-d", "--rm", "--name", &name, "-p", "127.0.0.1::22", "-e"])
        .arg(format!("AUTHORIZED_KEY={}", public.trim()))
        .arg(IMAGE));
    // No curl nor wget: the remote cannot download, so the app does it for it.
    run(Command::new("docker").args(["exec", &name, "rm", "-f", "/usr/bin/curl", "/usr/bin/wget"]));
    let port = run(Command::new("docker").args(["port", &name, "22"]));
    let port = port.lines().next().unwrap().rsplit(':').next().unwrap().to_owned();
    let config = dir.join("ssh_config");
    std::fs::write(
        &config,
        format!(
            "Host snout-test\n  HostName 127.0.0.1\n  Port {port}\n  User dev\n  IdentityFile {}\n  IdentitiesOnly yes\n  \
             IdentityAgent none\n  StrictHostKeyChecking no\n  UserKnownHostsFile /dev/null\n  LogLevel ERROR\n",
            key.display()
        ),
    )
    .unwrap();
    let mut target = SshTarget::new("snout-test");
    target.config_file = Some(config);
    target.control_dir = Some(dir.join("c"));
    target.connect_timeout = Duration::from_secs(20);
    let start = Instant::now();
    while !target.command("true").output().map(|o| o.status.success()).unwrap_or(false) {
        assert!(start.elapsed() < Duration::from_secs(30), "sshd did not come up");
        std::thread::sleep(Duration::from_millis(200));
    }

    let session = Arc::new(RemoteSession::open(target.clone(), &binaries).unwrap());
    let host = RemoteHost::connect(session).unwrap();
    assert_eq!(host.info().os, "linux");
    assert!(snout_app::lang::java::engine_dist(&host).is_none(), "nothing installed yet");

    let progress = snout_app::lang::java::DownloadProgress::default();
    let started = Instant::now();
    let engine = snout_app::lang::remote_engine::install(&host, &|step| eprintln!("install: {step}"), &progress)
        .expect("the engine installs");
    eprintln!("installed in {:.1} s at {}", started.elapsed().as_secs_f32(), engine.display());
    assert_eq!(engine.parent().unwrap(), Path::new("/home/dev/.snout-code/engine"));
    assert_eq!(snout_app::lang::java::engine_dist(&host).as_deref(), Some(engine.as_path()));
    // Installed: a second time is free.
    let again = Instant::now();
    snout_app::lang::remote_engine::install(&host, &|step| panic!("nothing to do, yet: {step}"), &progress).unwrap();
    assert!(again.elapsed() < Duration::from_secs(5));

    // The JDK it downloaded is the one discovery finds there.
    let jdks = snout_app::lang::java::discover(&host, &host.info().home);
    assert!(jdks.iter().any(|j| j.major == 21 && j.home.starts_with("/home/dev/.snout-code/jdks")), "{jdks:?}");

    // The engine syncs the container's Gradle project.
    let root = Path::new("/home/dev/demo");
    let out = snout_app::project_model::synced_model(host.info(), root);
    assert!(out.starts_with("/home/dev/.cache/SnoutCode"), "{}", out.display());
    let started = Instant::now();
    let synced = snout_app::project_model::run_sync(&host, &engine, root, &out, None);
    let log = host.read_to_string(&out.parent().unwrap().join("sync.log")).unwrap_or_default();
    assert!(synced.is_ok(), "{synced:?}\n{log}");
    let model = host.read_to_string(&out).unwrap();
    assert!(model.contains("\"rootDir\""), "{model}");
    assert!(model.contains("GreeterTest") || model.contains("src/test/java"), "{model}");
    eprintln!("synced in {:.1} s", started.elapsed().as_secs_f32());
    host.close();
    target.close_master();
    drop(container);
}
