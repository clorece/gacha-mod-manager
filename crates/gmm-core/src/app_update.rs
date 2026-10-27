// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app update (security rules R2 and R7): GMM reads its own releases list on GitHub and
//! says whether a newer version exists. On Windows it then downloads that release's installer
//! (`GMM-<version>-x64.msi`) and its signature, and installs it only when the signature
//! matches [`UPDATE_PUBLIC_KEY`]: the release workflow signs `GMM update <version>` and the
//! installer's SHA-256 with the private half, which only the repository's secrets hold. So a
//! swapped file, an older installer passed off as newer, or a release made without the key is
//! refused. Elsewhere the user gets the release page. Pre-releases count while GMM itself is a
//! pre-release (the betas), so a beta user hears about the next beta and the first stable
//! release; a stable build hears only about stable ones. Drafts never count.

use std::ops::ControlFlow;
use std::path::PathBuf;

use gmm_net::{
    Client, DownloadRequest, GITHUB_DOWNLOADS_HOST, GITHUB_DOWNLOADS_PATH, GITHUB_RELEASES_HOST,
    GITHUB_RELEASES_PATH, Progress, Url,
};
use ring::signature::{ED25519, UnparsedPublicKey};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::workspace::Workspace;

/// The public half of GMM's update key (ed25519, hex). The private half is the repository
/// secret `GMM_UPDATE_KEY`; `scripts/sign-update.mjs` signs each release's installer with it.
pub const UPDATE_PUBLIC_KEY: &str =
    "e33c844b6a57e5a365814cbab927fcd1b5e432483004cb753fd8cc63505e3871";

/// Most bytes an installer may have (the MSI is about 9 MB).
const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;

/// Whether this build installs updates itself (Windows, from the MSI); elsewhere the update
/// notice opens the release page.
pub const SELF_UPDATES: bool = cfg!(windows);

/// This build's version (`0.0.1-beta`): the workspace version every crate shares.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where releases are listed for people (the page the update notice opens).
pub const RELEASES_PAGE: &str = "https://github.com/clorece/gacha-mod-manager/releases";

/// How many of the newest releases GitHub is asked for.
const PER_PAGE: u32 = 20;

/// A newer GMM on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppRelease {
    /// The version, without the tag's `v` (`0.2.0-beta`).
    pub version: String,
    /// The release's title on GitHub, if it has one.
    pub name: Option<String>,
    pub prerelease: bool,
    /// ISO 8601, as GitHub sends it.
    pub published_at: Option<String>,
    /// GMM can download and install it itself ([`SELF_UPDATES`]).
    pub installable: bool,
}

impl AppRelease {
    /// Its page on GitHub, built from the version (never from the answer's own address).
    pub fn page(&self) -> String {
        release_page(&self.version)
    }
}

/// The page of the release tagged `v<version>`. `version` must be a semantic version.
pub fn release_page(version: &str) -> String {
    format!("{RELEASES_PAGE}/tag/v{version}")
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
}

/// Asks GitHub for GMM's releases and returns the newest one newer than `current`, if any.
pub async fn check_app_update(
    net: &Client,
    current: &str,
) -> Result<Option<AppRelease>, CoreError> {
    let url = Url::parse(&format!(
        "https://{GITHUB_RELEASES_HOST}{GITHUB_RELEASES_PATH}?per_page={PER_PAGE}"
    ))
    .map_err(|e| CoreError::Releases(e.to_string()))?;
    let response = net.get(&url).await?;
    if !response.is_success() {
        return Err(CoreError::Releases(format!("HTTP {}", response.status)));
    }
    newest_release(&response.body, current)
}

/// The newest release in GitHub's answer that is newer than `current` (see the module docs
/// for which ones count). Tags that aren't `v<semantic version>` are ignored.
pub fn newest_release(body: &[u8], current: &str) -> Result<Option<AppRelease>, CoreError> {
    let current =
        Version::parse(current).map_err(|e| CoreError::Releases(format!("this build: {e}")))?;
    let raw: Vec<RawRelease> =
        serde_json::from_slice(body).map_err(|e| CoreError::Releases(e.to_string()))?;
    let take_prereleases = !current.pre.is_empty();
    let newest = raw
        .into_iter()
        .filter(|r| !r.draft && (take_prereleases || !r.prerelease))
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.strip_prefix('v')?).ok()?;
            (take_prereleases || version.pre.is_empty()).then_some((version, r))
        })
        .filter(|(version, _)| *version > current)
        .max_by(|(a, _), (b, _)| a.cmp(b));
    Ok(newest.map(|(version, r)| AppRelease {
        version: version.to_string(),
        name: r.name.filter(|n| !n.trim().is_empty()),
        prerelease: r.prerelease || !version.pre.is_empty(),
        published_at: r.published_at,
        installable: SELF_UPDATES,
    }))
}

/// What the release workflow signs for `version`'s installer with SHA-256 `sha256` (hex).
fn signed_message(version: &str, sha256: &str) -> String {
    format!("GMM update {version}\n{}\n", sha256.to_ascii_lowercase())
}

/// Whether `signature` (hex) is `public_key`'s signature of `version`'s installer.
fn signature_matches(public_key: &[u8], version: &str, sha256: &str, signature: &str) -> bool {
    let Ok(signature) = hex::decode(signature.trim()) else {
        return false;
    };
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(signed_message(version, sha256).as_bytes(), &signature)
        .is_ok()
}

/// Whether `signature` is GMM's signature of `version`'s installer.
pub fn update_signature_matches(version: &str, sha256: &str, signature: &str) -> bool {
    hex::decode(UPDATE_PUBLIC_KEY)
        .is_ok_and(|key| signature_matches(&key, version, sha256, signature))
}

fn download_url(file: &str, version: &Version) -> Result<Url, CoreError> {
    Url::parse(&format!(
        "https://{GITHUB_DOWNLOADS_HOST}{GITHUB_DOWNLOADS_PATH}v{version}/{file}"
    ))
    .map_err(|e| CoreError::BadUpdate(e.to_string()))
}

/// Downloads `version`'s installer into `<cache>/updates/` with `net` (a client scoped to
/// GMM's release downloads), checks its signature, and returns its path for Windows
/// Installer. Earlier installers there are removed first; a bad one is removed too.
pub async fn download_app_update(
    net: &Client,
    ws: &Workspace,
    version: &str,
    on_progress: impl FnMut(Progress) -> ControlFlow<()>,
) -> Result<PathBuf, CoreError> {
    let version =
        Version::parse(version).map_err(|e| CoreError::BadUpdate(format!("version: {e}")))?;
    let guard = ws.guard();
    let dir = guard.path(ws.cache_root(), "updates")?;
    if guard.exists(&dir)? {
        guard.remove_dir_all(&dir)?;
    }
    guard.create_dir_all(&dir)?;

    let file = format!("GMM-{version}-x64.msi");
    let dest = dir.join(&file)?;
    let outcome = net
        .download(
            guard,
            &DownloadRequest {
                url: download_url(&file, &version)?,
                dest: dest.clone(),
                expected_size: None,
                expected_md5: None,
                max_bytes: MAX_INSTALLER_BYTES,
            },
            on_progress,
        )
        .await?;

    let refuse = |why: &str| {
        let _ = guard.remove_file(&dest);
        CoreError::BadUpdate(why.to_owned())
    };
    let sig = match net
        .get(&download_url(&format!("{file}.sig"), &version)?)
        .await
    {
        Ok(r) if r.is_success() && r.body.len() <= 1024 => r.body,
        Ok(r) => {
            return Err(refuse(&format!(
                "its signature couldn't be read (HTTP {})",
                r.status
            )));
        }
        Err(e) => return Err(refuse(&format!("its signature couldn't be read: {e}"))),
    };
    let sig = String::from_utf8_lossy(&sig);
    if !update_signature_matches(&version.to_string(), &outcome.sha256, &sig) {
        return Err(refuse(
            "its signature doesn't match GMM's update key, so it may not come from GMM",
        ));
    }
    Ok(guard.os_path_for_library(&dest)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(tags: &[(&str, bool, bool)]) -> Vec<u8> {
        let items: Vec<String> = tags
            .iter()
            .map(|(tag, draft, pre)| {
                format!(
                    r#"{{"tag_name":"{tag}","name":"GMM {tag}","draft":{draft},"prerelease":{pre},
                        "published_at":"2026-11-01T10:00:00Z","html_url":"https://evil.example/"}}"#
                )
            })
            .collect();
        format!("[{}]", items.join(",")).into_bytes()
    }

    #[test]
    fn a_beta_hears_about_the_next_beta_and_stable_releases() {
        let body = list(&[
            ("v0.1.0-beta", false, true),
            ("v0.2.0-beta", false, true),
            ("v0.3.0", true, false), // a draft
            ("nightly", false, true),
        ]);
        let found = newest_release(&body, "0.1.0-beta").unwrap().unwrap();
        assert_eq!(found.version, "0.2.0-beta");
        assert!(found.prerelease);
        assert_eq!(
            found.page(),
            "https://github.com/clorece/gacha-mod-manager/releases/tag/v0.2.0-beta",
            "the page comes from the version, not the answer's html_url"
        );

        let stable = list(&[("v0.1.0", false, false), ("v0.1.1-beta", false, true)]);
        assert_eq!(
            newest_release(&stable, "0.1.0-beta")
                .unwrap()
                .unwrap()
                .version,
            "0.1.1-beta"
        );
    }

    #[test]
    fn a_stable_build_ignores_betas_and_the_current_version() {
        let body = list(&[("v0.1.0", false, false), ("v0.2.0-beta", false, true)]);
        assert_eq!(newest_release(&body, "0.1.0").unwrap(), None);
        assert_eq!(newest_release(&list(&[]), "0.1.0-beta").unwrap(), None);
    }

    #[test]
    fn only_the_keys_signature_of_this_version_and_file_counts() {
        use ring::rand::SystemRandom;
        use ring::signature::{Ed25519KeyPair, KeyPair};

        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let key = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let public = key.public_key().as_ref();
        let sha = "ab".repeat(32);
        let sig = hex::encode(key.sign(signed_message("0.0.3-beta", &sha).as_bytes()));

        assert!(signature_matches(public, "0.0.3-beta", &sha, &sig));
        assert!(signature_matches(
            public,
            "0.0.3-beta",
            &sha.to_uppercase(),
            &format!("{sig}\n")
        ));
        assert!(
            !signature_matches(public, "0.0.2-beta", &sha, &sig),
            "an older version"
        );
        assert!(
            !signature_matches(public, "0.0.3-beta", &"cd".repeat(32), &sig),
            "another file"
        );
        let mut flipped = sig.clone().into_bytes();
        flipped[0] = if flipped[0] == b'0' { b'1' } else { b'0' };
        assert!(!signature_matches(
            public,
            "0.0.3-beta",
            &sha,
            &String::from_utf8(flipped).unwrap()
        ));
        assert!(!signature_matches(public, "0.0.3-beta", &sha, "not hex"));
        assert!(!signature_matches(public, "0.0.3-beta", &sha, ""));
        assert!(
            !update_signature_matches("0.0.3-beta", &sha, &sig),
            "another key's signature isn't GMM's"
        );
        assert_eq!(hex::decode(UPDATE_PUBLIC_KEY).unwrap().len(), 32);
    }

    #[test]
    fn reads_what_the_release_script_signs() {
        // Made by Node's crypto.sign with a throwaway key, as scripts/sign-update.mjs signs.
        let public =
            hex::decode("d08fccc042d7b97c6b086cc767ad40185638c2bec27d64ef8f0fa9457cda8513")
                .unwrap();
        let sig = "33e3ceb33fcffd4482dc35c1e3757a8a3a5ab81da97dd636c30a9f775a5ed15b\
                   2cfd8f7ecfc22ee5e04481ed4d191b89ebd1ef6d320b0fed39aa186d4f8a6706\n";
        assert!(signature_matches(
            &public,
            "0.0.3-beta",
            &"ab".repeat(32),
            sig
        ));
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(newest_release(b"{\"message\":\"Not Found\"}", "0.1.0").is_err());
        assert!(newest_release(b"[]", "not a version").is_err());
    }
}
