//! The Java engine on a remote machine. Its jars (the engine's, jdtls and its plugins) are
//! the same everywhere; the Java runtime the Mac app bundles is not. So the app copies the
//! jars once per engine build into `~/.snout-code/engine/<hash>/` (the hash of their
//! contents), runs them on a Temurin 21 downloaded into `~/.snout-code/jdks` (linked as the
//! folder's `runtime`), and the engine then starts there as it does on the Mac.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context as _, bail};
use snout_host::{Cmd, Host};

use super::java::{self, DownloadProgress};

/// The Java release the engine runs on.
pub const ENGINE_JAVA: u32 = 21;

/// What the distribution holds besides its runtime: what is copied (`maven` holds the test
/// listener Maven runs load).
const PARTS: [&str; 6] = ["bin", "lib", "jdtls", "jdtls-plugins", "maven", "dist.properties"];

/// Marks a folder whose copy is whole.
const COMPLETE: &str = ".complete";

/// The engine distribution of this Mac.
fn local_dist() -> Option<PathBuf> {
    java::engine_dist(&*snout_host::local())
}

/// A hash of the distribution's files (all but the runtime), for its folder's name.
/// Computed once per distribution.
pub fn dist_hash(dist: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    static KNOWN: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let known = KNOWN.get_or_init(Default::default);
    if let Some(h) = known.lock().unwrap_or_else(|e| e.into_inner()).get(dist) {
        return Ok(h.clone());
    }
    let mut files = Vec::new();
    for part in PARTS {
        collect(&dist.join(part), &mut files)?;
    }
    files.sort();
    let mut hasher = Sha256::new();
    for f in &files {
        let rel = f.strip_prefix(dist).unwrap_or(f);
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(std::fs::read(f)?);
    }
    let hash: String = hasher.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
    known.lock().unwrap_or_else(|e| e.into_inner()).insert(dist.to_owned(), hash.clone());
    Ok(hash)
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    if m.is_dir() {
        for e in std::fs::read_dir(path)? {
            collect(&e?.path(), out)?;
        }
    } else {
        out.push(path.to_owned());
    }
    Ok(())
}

/// `~/.snout-code/engine` on `host`.
fn engines_dir(host: &dyn Host) -> PathBuf {
    host.info().home.join(".snout-code/engine")
}

/// The engine installed on `host` for this Mac's distribution, if it is there whole (and
/// has its Java).
pub fn installed(host: &dyn Host) -> Option<PathBuf> {
    let hash = local_dist().and_then(|d| dist_hash(&d).ok())?;
    let dir = engines_dir(host).join(hash);
    (host.is_file(&dir.join(COMPLETE)) && host.is_file(&java::java_bin(host.info(), &dir.join("runtime"))))
        .then_some(dir)
}

/// Put the engine on `host` if it is not there: the Java it runs on (downloaded when the
/// host has none of SnoutCode's), then the jars. `say` hears each step; `progress` counts
/// the bytes of a download or a copy. Returns the engine's folder there.
pub fn install(host: &dyn Host, say: &dyn Fn(&str), progress: &DownloadProgress) -> anyhow::Result<PathBuf> {
    // One at a time: the app's own install, and an engine start that waits for it.
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dist = local_dist().context("the Java engine is not built (run snout.sh)")?;
    let hash = dist_hash(&dist).context("read the engine")?;
    let dir = engines_dir(host).join(&hash);
    let java_ok = host.is_file(&java::java_bin(host.info(), &dir.join("runtime")));
    if host.is_file(&dir.join(COMPLETE)) && java_ok {
        return Ok(dir);
    }

    // The Java it runs on.
    let downloads = java::downloads_dir(host.info());
    let jdk = match java::jdks_in(host, &downloads).into_iter().find(|j| j.major == ENGINE_JAVA) {
        Some(jdk) => jdk,
        None => {
            say(&format!("Downloading Java {ENGINE_JAVA} for the engine"));
            java::download_temurin(host, ENGINE_JAVA, &downloads, progress)?
        }
    };

    // The jars, in one archive, unpacked beside the folder and moved into place.
    if !host.is_file(&dir.join(COMPLETE)) {
        say("Copying the Java engine");
        let archive = local_archive(&dist, &hash)?;
        let part = engines_dir(host).join(format!("{hash}.part"));
        let _ = host.remove_dir_all(&part);
        host.create_dir_all(&part)?;
        let remote_archive = part.join("engine.tar.gz");
        java::upload(host, &archive, &remote_archive, progress)?;
        let want = java::sha256_on(&*snout_host::local(), &archive)?;
        if java::sha256_on(host, &remote_archive)? != want {
            bail!("the engine arrived damaged");
        }
        let out = host.run(&Cmd::new("tar").arg("-xzf").arg(&remote_archive).arg("-C").arg(&part), None)?;
        if !out.status.success() {
            bail!("the engine could not be unpacked: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        host.remove_file(&remote_archive)?;
        let _ = host.remove_dir_all(&dir);
        host.rename(&part, &dir)?;
    }
    // Its runtime: the JDK, linked.
    let out = host.run(&Cmd::new("ln").arg("-sfn").arg(&jdk.home).arg(dir.join("runtime")), None)?;
    if !out.status.success() {
        bail!("could not link the engine's Java: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    host.write(&dir.join(COMPLETE), hash.as_bytes())?;
    Ok(dir)
}

/// The distribution's jars in a gzipped tar on this Mac, made once per build.
fn local_archive(dist: &Path, hash: &str) -> anyhow::Result<PathBuf> {
    let file = std::env::temp_dir().join(format!("snout-engine-{hash}.tar.gz"));
    if file.is_file() {
        return Ok(file);
    }
    let tmp = file.with_extension("part");
    // No AppleDouble files or extended attributes: the archive is for Linux.
    let status = snout_host::default_signals(&mut std::process::Command::new("tar"))
        .env("COPYFILE_DISABLE", "1")
        .arg("--no-xattrs")
        .arg("-czf")
        .arg(&tmp)
        .arg("-C")
        .arg(dist)
        .args(PARTS)
        .status()
        .context("run tar")?;
    if !status.success() {
        bail!("could not pack the engine ({status})");
    }
    std::fs::rename(&tmp, &file)?;
    Ok(file)
}
