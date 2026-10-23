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

#[derive(Debug, Clone)]
pub struct Allowlist {
    /// Test builds only: allow plain HTTP to 127.0.0.1 (mock servers).
    loopback_http: bool,
}

impl Allowlist {
    /// The only allowlist release builds can construct.
    pub fn production() -> Self {
        Self {
            loopback_http: false,
        }
    }

    /// Production rules plus `http://127.0.0.1:<any port>`, for mock-server tests.
    #[cfg(feature = "test-util")]
    pub fn with_loopback_for_tests() -> Self {
        Self {
            loopback_http: true,
        }
    }

    pub fn check(&self, url: &Url) -> Result<(), NetError> {
        let denied = || NetError::Disallowed(redact(url));
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
        if EXACT_HOSTS.contains(&host) || NUMBERED_HOSTS.iter().any(|r| matches_numbered(host, r)) {
            Ok(())
        } else {
            Err(denied())
        }
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
    fn redact_drops_query_and_credentials() {
        let url = Url::parse("https://u:p@gamebanana.com/a?token=secret#frag").unwrap();
        assert_eq!(redact(&url), "https://gamebanana.com/a");
    }
}
