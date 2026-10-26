// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app update check (security rule R2): GMM reads its own releases list
//! on GitHub and says whether a newer version exists. It never downloads or runs anything:
//! the user gets the release page in their browser (after a confirmation) and installs the
//! new MSI themselves. Pre-releases count while GMM itself is a pre-release (the betas), so
//! a beta user hears about the next beta and the first stable release; a stable build hears
//! only about stable ones. Drafts never count.

use gmm_net::{Client, GITHUB_RELEASES_HOST, GITHUB_RELEASES_PATH, Url};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

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
    }))
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
    fn garbage_is_an_error() {
        assert!(newest_release(b"{\"message\":\"Not Found\"}", "0.1.0").is_err());
        assert!(newest_release(b"[]", "not a version").is_err());
    }
}
