// SPDX-License-Identifier: MIT OR Apache-2.0

use url::{Host, Url};

use crate::error::NetError;

/// Hosts GMM may contact, compiled in. Adding one needs a `security-review` PR
/// (docs/04-security.md).
const EXACT_HOSTS: &[&str] = &[
    "gamebanana.com",
    // `gamebanana.com/dl/<id>` redirects here, then to a numbered `filecacheN` mirror.
    "files.gamebanana.com",
    // Preview images, avatars, category icons.
    "images.gamebanana.com",
];

/// Download mirrors are `filecache<digits>.gamebanana.com` (observed: `filecache42`).
const NUMBERED_HOSTS: &[(&str, &str)] = &[("filecache", "gamebanana.com")];

/// The one GitHub address GMM reads: its own releases list, for the app update check
/// (security rule R2, amended 2026-10-26). Nothing else on GitHub, and nothing is downloaded.
pub const GITHUB_RELEASES_HOST: &str = "api.github.com";
pub const GITHUB_RELEASES_PATH: &str = "/repos/clorece/gacha-mod-manager/releases";

/// What a client is for. Each kind of client reaches only its own hosts, so the GameBanana
/// clients can't reach GitHub and the update check can't reach anything but the releases list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    GameBanana,
    GitHubReleases,
}

#[derive(Debug, Clone)]
pub struct Allowlist {
    scope: Scope,
    /// Test builds only: allow plain HTTP to 127.0.0.1 (mock servers).
    loopback_http: bool,
    /// Refuse every URL ([`Allowlist::deny_all`]).
    deny_all: bool,
}

impl Allowlist {
    /// GameBanana's hosts: the allowlist of every catalogue, image and download client.
    pub fn production() -> Self {
        Self {
            scope: Scope::GameBanana,
            loopback_http: false,
            deny_all: false,
        }
    }

    /// GMM's own releases list on GitHub, and nothing else: the app update check's client.
    pub fn github_releases() -> Self {
        Self {
            scope: Scope::GitHubReleases,
            loopback_http: false,
            deny_all: false,
        }
    }

    /// The same rules for GMM's releases list. A deny-all list stays deny-all (the IPC fuzz
    /// harness builds every client from one list).
    pub fn for_github_releases(&self) -> Self {
        Self {
            scope: Scope::GitHubReleases,
            ..self.clone()
        }
    }

    /// Refuses every URL before a connection is made. For harnesses that must prove nothing
    /// reaches the network (the desktop app's IPC fuzz harness); it can only take hosts away,
    /// so it is safe in any build.
    pub fn deny_all() -> Self {
        Self {
            scope: Scope::GameBanana,
            loopback_http: false,
            deny_all: true,
        }
    }

    /// Production rules plus `http://127.0.0.1:<any port>`, for mock-server tests.
    #[cfg(feature = "test-util")]
    pub fn with_loopback_for_tests() -> Self {
        Self {
            scope: Scope::GameBanana,
            loopback_http: true,
            deny_all: false,
        }
    }

    pub fn check(&self, url: &Url) -> Result<(), NetError> {
        let denied = || NetError::Disallowed(redact(url));
        if self.deny_all {
            return Err(denied());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(denied());
        }
        if self.loopback_http
            && url.scheme() == "http"
            && url.host() == Some(Host::Ipv4(std::net::Ipv4Addr::LOCALHOST))
        {
            return Ok(());
        }
        if url.scheme() != "https" || url.port().is_some_and(|p| p != 443) {
            return Err(denied());
        }
        // IP literals never match: only the domain names below are allowed.
        let Some(Host::Domain(host)) = url.host() else {
            return Err(denied());
        };
        let host = host.to_ascii_lowercase();
        let host = host.strip_suffix('.').unwrap_or(&host);
        let allowed = match self.scope {
            Scope::GameBanana => {
                EXACT_HOSTS.contains(&host)
                    || NUMBERED_HOSTS.iter().any(|r| matches_numbered(host, r))
            }
            // The exact path; a query (`per_page`) is fine.
            Scope::GitHubReleases => {
                host == GITHUB_RELEASES_HOST && url.path() == GITHUB_RELEASES_PATH
            }
        };
        if allowed { Ok(()) } else { Err(denied()) }
    }
}

fn matches_numbered(host: &str, (prefix, domain): &(&str, &str)) -> bool {
    let Some(label) = host
        .strip_suffix(domain)
        .and_then(|rest| rest.strip_suffix('.'))
    else {
        return false;
    };
    label
        .strip_prefix(prefix)
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// URL without query or fragment, for error messages and logs.
pub(crate) fn redact(url: &Url) -> String {
    let mut shown = url.clone();
    shown.set_query(None);
    shown.set_fragment(None);
    let _ = shown.set_username("");
    let _ = shown.set_password(None);
    shown.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> bool {
        Allowlist::production()
            .check(&Url::parse(s).unwrap())
            .is_ok()
    }

    #[test]
    fn allows_gamebanana_https() {
        assert!(ok("https://gamebanana.com/apiv11/Mod/1/ProfilePage"));
        assert!(ok("https://GameBanana.com/dl/1860169"));
        assert!(ok("https://files.gamebanana.com/mods/x.zip"));
        assert!(ok("https://filecache42.gamebanana.com/mods/x.zip"));
        assert!(ok("https://images.gamebanana.com/img/ss/mods/a.jpg"));
        assert!(ok("https://gamebanana.com:443/x"));
        assert!(ok("https://gamebanana.com./x"));
    }

    #[test]
    fn rejects_everything_else() {
        assert!(!ok("http://gamebanana.com/x"), "plain http");
        assert!(!ok("https://gamebanana.com:8443/x"), "other port");
        assert!(!ok("https://evil.com/x"));
        assert!(!ok("https://gamebanana.com.evil.com/x"), "suffix trick");
        assert!(!ok("https://evilgamebanana.com/x"));
        assert!(!ok("https://x.images.gamebanana.com/x"), "deeper subdomain");
        assert!(!ok("https://filecache.gamebanana.com/x"), "no digits");
        assert!(!ok("https://filecache4a.gamebanana.com/x"));
        assert!(!ok("https://filecache1.evil.com/x"));
        assert!(!ok("https://user:pw@gamebanana.com/x"), "credentials");
        assert!(!ok("https://104.18.0.1/x"), "IP literal");
        assert!(!ok("https://[::1]/x"));
        assert!(
            !ok("http://127.0.0.1:8080/x"),
            "loopback only in test builds"
        );
        assert!(!ok("file:///C:/Windows/win.ini"));
        assert!(!ok("ftp://gamebanana.com/x"));
    }

    #[test]
    fn github_releases_reach_only_gmms_releases_list() {
        let gh = |s: &str| {
            Allowlist::github_releases()
                .check(&Url::parse(s).unwrap())
                .is_ok()
        };
        assert!(gh(
            "https://api.github.com/repos/clorece/gacha-mod-manager/releases?per_page=20"
        ));
        assert!(!gh("https://api.github.com/repos/clorece/gacha-mod-manager/releases/1"));
        assert!(!gh("https://api.github.com/repos/other/gacha-mod-manager/releases"));
        assert!(!gh("https://github.com/clorece/gacha-mod-manager/releases"));
        assert!(!gh("http://api.github.com/repos/clorece/gacha-mod-manager/releases"));
        assert!(!gh("https://gamebanana.com/apiv11/Mod/1/ProfilePage"), "no GameBanana");
        assert!(
            !ok("https://api.github.com/repos/clorece/gacha-mod-manager/releases"),
            "the GameBanana clients can't reach GitHub"
        );
        let denied = Allowlist::deny_all().for_github_releases();
        assert!(
            denied
                .check(
                    &Url::parse("https://api.github.com/repos/clorece/gacha-mod-manager/releases")
                        .unwrap()
                )
                .is_err(),
            "deny-all stays deny-all"
        );
    }

    #[test]
    fn deny_all_refuses_even_gamebanana() {
        let none = Allowlist::deny_all();
        for url in [
            "https://gamebanana.com/apiv11/Mod/1/ProfilePage",
            "https://images.gamebanana.com/img/ss/mods/a.jpg",
            "https://filecache42.gamebanana.com/mods/x.zip",
        ] {
            assert!(none.check(&Url::parse(url).unwrap()).is_err(), "{url}");
        }
    }

    #[test]
    fn redact_drops_query_and_credentials() {
        let url = Url::parse("https://u:p@gamebanana.com/a?token=secret#frag").unwrap();
        assert_eq!(redact(&url), "https://gamebanana.com/a");
    }
}
