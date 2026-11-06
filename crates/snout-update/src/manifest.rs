//! The release feed: `latest.json`, naming the version and each platform's file with its
//! SHA-256, and `latest.json.sig`, its Ed25519 signature in hex.
//!
//! The checksums sit in the same release as the files, so alone they vouch for nothing a release
//! could not also replace. The signature does: the private key never goes near GitHub, and the
//! public one is compiled in. So a release replaced behind our back is refused, on Windows too,
//! where the installers are not code signed yet.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::failure::{Failure, describe};

pub const MANIFEST: &str = "latest.json";
pub const SIGNATURE: &str = "latest.json.sig";

/// The public half of the release signing key, in hex (`snout-update-sign keygen` prints it).
pub const PUBLIC_KEY: &str = "07d14c359ff6ccd81dd8c4c282f0180a8773719cfd1670d576abbad2bb09f142";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    /// The release's notes, as Markdown.
    #[serde(default)]
    pub notes: Option<String>,
    /// By [`platform_key`]: the files of the systems the app installs itself on.
    #[serde(default)]
    pub platforms: BTreeMap<String, Asset>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// The file's name in the release.
    pub file: String,
    /// Its SHA-256, in hex.
    pub sha256: String,
    pub size: u64,
}

/// The key of this machine in [`Manifest::platforms`]: `macos-aarch64`, `windows-x86_64`.
pub fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The platform a release file of `version` installs on, by its name as the packaging scripts
/// write it. None for what the app does not install itself (a disk image, a Linux package).
pub fn platform_of_file(name: &str, version: &str) -> Option<&'static str> {
    let rest = name.strip_prefix(&format!("SnoutCode-{version}-"))?;
    match rest {
        "arm64-mac.zip" => Some("macos-aarch64"),
        "x86_64-mac.zip" => Some("macos-x86_64"),
        "windows-x64-setup.exe" => Some("windows-x86_64"),
        "windows-arm64-setup.exe" => Some("windows-aarch64"),
        _ => None,
    }
}

/// `hex` as bytes.
pub fn from_hex(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}

/// `bytes` in hex.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The manifest in `body`, if `signature` (hex) is the signature of exactly those bytes under
/// `public_key` (hex).
pub fn verified(body: &[u8], signature: &[u8], public_key: &str) -> Result<Manifest, Failure> {
    let refused = |why: &str| describe(&format!("the feed's signature does not verify: {why}"), None);
    let key: [u8; 32] = from_hex(public_key)
        .and_then(|k| k.try_into().ok())
        .ok_or_else(|| refused("this build carries no valid public key"))?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| refused("this build carries no valid public key"))?;
    let signature: [u8; 64] = std::str::from_utf8(signature)
        .ok()
        .and_then(from_hex)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| refused("it is not a signature"))?;
    key.verify(body, &Signature::from_bytes(&signature)).map_err(|_| refused("it was not made with our key"))?;
    let manifest: Manifest =
        serde_json::from_slice(body).map_err(|e| describe(&format!("the manifest is not the feed: {e}"), Some(500)))?;
    for asset in manifest.platforms.values() {
        // A name, not a path: it is joined to the download folder.
        if asset.file.is_empty() || asset.file.contains(['/', '\\']) || asset.file.starts_with('.') {
            return Err(describe(&format!("the manifest is not the feed: it names {:?}", asset.file), Some(500)));
        }
    }
    Ok(manifest)
}

#[cfg(test)]
pub(crate) mod tests {
    use ed25519_dalek::{Signer as _, SigningKey};

    use super::*;
    use crate::failure::Kind;

    pub fn key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    pub fn public() -> String {
        to_hex(key().verifying_key().as_bytes())
    }

    pub fn sign(body: &[u8]) -> String {
        to_hex(&key().sign(body).to_bytes())
    }

    const BODY: &[u8] =
        br#"{"version":"1.0.1","notes":"Fixes.","platforms":{"macos-aarch64":{"file":"a.zip","sha256":"ab","size":3}}}"#;

    #[test]
    fn a_signed_manifest_reads() {
        let m = verified(BODY, sign(BODY).as_bytes(), &public()).unwrap();
        assert_eq!(m.version, "1.0.1");
        assert_eq!(m.notes.as_deref(), Some("Fixes."));
        assert_eq!(m.platforms["macos-aarch64"].size, 3);
    }

    #[test]
    fn anything_changed_or_signed_by_another_key_is_refused() {
        let signature = sign(BODY);
        let changed = String::from_utf8_lossy(BODY).replace("1.0.1", "9.9.9");
        for (body, sig, public) in [
            (changed.as_bytes(), signature.as_str(), public()),
            (BODY, "abcd", public()),
            (BODY, "not hex", public()),
            (BODY, signature.as_str(), to_hex(SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes())),
            (BODY, signature.as_str(), PUBLIC_KEY[..8].to_owned()),
        ] {
            assert_eq!(verified(body, sig.as_bytes(), &public).unwrap_err().kind, Kind::Signature);
        }
    }

    #[test]
    fn a_file_name_cannot_climb_out_of_the_download_folder() {
        for name in ["../../evil", "a/b", "a\\b", ".hidden", ""] {
            let body = format!(
                r#"{{"version":"1.0.1","platforms":{{"p":{{"file":"{}","sha256":"","size":1}}}}}}"#,
                name.replace('\\', "\\\\")
            );
            let e = verified(body.as_bytes(), sign(body.as_bytes()).as_bytes(), &public()).unwrap_err();
            assert_eq!(e.kind, Kind::FeedUnavailable, "{name}");
        }
    }

    #[test]
    fn release_files_map_to_platforms_as_the_packaging_scripts_name_them() {
        let of = |n| platform_of_file(n, "1.0.1");
        assert_eq!(of("SnoutCode-1.0.1-arm64-mac.zip"), Some("macos-aarch64"));
        assert_eq!(of("SnoutCode-1.0.1-x86_64-mac.zip"), Some("macos-x86_64"));
        assert_eq!(of("SnoutCode-1.0.1-windows-x64-setup.exe"), Some("windows-x86_64"));
        assert_eq!(of("SnoutCode-1.0.1-windows-arm64-setup.exe"), Some("windows-aarch64"));
        for other in ["SnoutCode-1.0.1-arm64.dmg", "snout-code_1.0.1_amd64.deb", "SnoutCode-1.0.0-arm64-mac.zip"] {
            assert_eq!(of(other), None, "{other}");
        }
    }

    #[test]
    fn hex_goes_both_ways() {
        assert_eq!(from_hex(&to_hex(&[0, 1, 254, 255])), Some(vec![0, 1, 254, 255]));
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }
}
