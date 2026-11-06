//! The release signing key, on the machine that makes releases:
//!
//!   snout-update-sign keygen <key file>       make the key; prints the public half for
//!                                             `manifest::PUBLIC_KEY`. Never overwrites a key.
//!   snout-update-sign sign <key file> <file>  writes <file>.sig
//!   snout-update-sign feed <key file> <version> <notes file> <folder>
//!                                             writes the release's latest.json for the files in
//!                                             the folder, and its signature; refuses a key that is
//!                                             not the one the app carries
//!
//! Every installed app trusts this key's signature and no other, so losing the key means those
//! apps can never update again: keep it backed up, and never in a repository.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use ed25519_dalek::{Signer as _, SigningKey};
use snout_update::manifest::{
    Asset, MANIFEST, Manifest, PUBLIC_KEY, SIGNATURE, from_hex, platform_of_file, to_hex, verified,
};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["keygen", key] => keygen(Path::new(key)),
        ["sign", key, file] => sign(Path::new(key), Path::new(file)),
        ["feed", key, version, notes, folder] => feed(Path::new(key), version, Path::new(notes), Path::new(folder)),
        _ => Err("usage: snout-update-sign keygen <key file> | sign <key file> <file> | \
                  feed <key file> <version> <notes file> <folder>"
            .to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn keygen(file: &Path) -> Result<(), String> {
    use std::io::{Read as _, Write as _};
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .map_err(|e| format!("no random numbers: {e}"))?;
    let key = SigningKey::from_bytes(&seed);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = options.open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    writeln!(out, "{}", to_hex(&seed)).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("{}", to_hex(key.verifying_key().as_bytes()));
    Ok(())
}

fn load_key(key_file: &Path) -> Result<SigningKey, String> {
    let text = std::fs::read_to_string(key_file).map_err(|e| format!("{}: {e}", key_file.display()))?;
    let seed: [u8; 32] = from_hex(&text)
        .and_then(|k| k.try_into().ok())
        .ok_or_else(|| format!("{} is not a signing key", key_file.display()))?;
    Ok(SigningKey::from_bytes(&seed))
}

fn feed(key_file: &Path, version: &str, notes: &Path, folder: &Path) -> Result<(), String> {
    let key = load_key(key_file)?;
    if to_hex(key.verifying_key().as_bytes()) != PUBLIC_KEY {
        return Err("this key is not the one the app carries: no installed app would accept this feed".to_owned());
    }
    let notes = std::fs::read_to_string(notes).map_err(|e| format!("{}: {e}", notes.display()))?;
    let mut platforms = BTreeMap::new();
    let dir = std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(platform) = platform_of_file(&name, version) else { continue };
        let sha256 = snout_update::fetch::sha256(&entry.path()).map_err(|e| format!("{name}: {e}"))?;
        let size = entry.metadata().map_err(|e| format!("{name}: {e}"))?.len();
        println!("{platform}: {name}");
        platforms.insert(platform.to_owned(), Asset { file: name, sha256, size });
    }
    if platforms.is_empty() {
        return Err(format!("no release files of v{version} in {}", folder.display()));
    }
    let manifest = Manifest { version: version.to_owned(), notes: Some(notes.trim().to_owned()), platforms };
    let body = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    let file = folder.join(MANIFEST);
    std::fs::write(&file, &body).map_err(|e| format!("{}: {e}", file.display()))?;
    sign(key_file, &file)?;
    // As an installed app will read it.
    let signature = std::fs::read(folder.join(SIGNATURE)).map_err(|e| e.to_string())?;
    verified(&body, &signature, PUBLIC_KEY).map(drop).map_err(|e| e.message)
}

fn sign(key_file: &Path, file: &Path) -> Result<(), String> {
    let key = load_key(key_file)?;
    let body = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let signature = to_hex(&key.sign(&body).to_bytes());
    let mut out = file.as_os_str().to_owned();
    out.push(".sig");
    std::fs::write(&out, signature).map_err(|e| format!("{}: {e}", file.display()))
}
