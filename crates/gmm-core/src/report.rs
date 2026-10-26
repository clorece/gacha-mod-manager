// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bug reports (Phase 6, decided 2026-10-25): a pre-filled GitHub issue that the user opens
//! in their browser and finishes there. GMM never connects to GitHub (R2 unchanged): it only
//! builds the address, which the app opens after a native confirmation and the CLI prints.
//!
//! The issue holds what the user wrote, a few lines about the setup (versions and setup
//! states, never a path) and a note on attaching the error log. Browsers and GMM's opener
//! cap addresses ([`gmm_xxmi::MAX_PAGE_URL`]), so a long description is shortened to fit and
//! the report says so; the user can paste the rest on GitHub.

use gmm_net::Url;
use gmm_xxmi::GameStatus;
use serde::Serialize;

/// Where reports go: the project's issue form.
pub const ISSUES_URL: &str = "https://github.com/clorece/gacha-mod-manager/issues/new";

/// Titles longer than this are cut.
pub const MAX_TITLE_CHARS: usize = 120;

/// What the issue will contain, as the user sees it before anything opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BugReport {
    pub title: String,
    pub body: String,
    /// The address that opens the pre-filled issue.
    pub url: String,
    /// The description was cut to fit the address.
    pub shortened: bool,
}

/// A few lines about this setup: GMM's version, the system, XXMI's version and each game's
/// setup state. Nothing from a path.
pub fn system_details(statuses: &[GameStatus]) -> String {
    let mut lines = vec![format!(
        "- GMM {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )];
    match statuses.iter().find_map(|s| s.xxmi_version.as_deref()) {
        Some(v) => lines.push(format!("- XXMI Launcher {v}")),
        None if statuses.iter().any(|s| s.xxmi_root.is_some()) => {
            lines.push("- XXMI Launcher (version unknown)".to_owned());
        }
        None => lines.push("- XXMI Launcher not found".to_owned()),
    }
    for s in statuses {
        let importer = s
            .importer_info
            .as_ref()
            .and_then(|i| i.importer_version.as_deref())
            .map(|v| format!(", {} {v}", s.importer))
            .unwrap_or_default();
        lines.push(format!("- {}: {}{importer}", s.game_name, s.state.label()));
    }
    lines.join("\n")
}

fn body(description: &str, details: &str, shortened: bool) -> String {
    let description = description.trim();
    let what = if description.is_empty() {
        "(Describe what happened, what you expected, and the steps to get there.)"
    } else {
        description
    };
    let cut = if shortened {
        "\n\n(Shortened to fit the link; paste the rest here.)"
    } else {
        ""
    };
    format!(
        "### What happened\n\n{what}{cut}\n\n### Setup\n\n{details}\n\n### Error log\n\n\
         If GMM's error log shows the problem, attach `errors.log` here (Report a Bug › Open \
         log folder in GMM). Read it first: it lists errors only, with your home folder \
         replaced by `~`."
    )
}

fn address(title: &str, body: &str) -> String {
    Url::parse_with_params(
        ISSUES_URL,
        [("title", title), ("body", body), ("labels", "bug")],
    )
    .map(String::from)
    .unwrap_or_else(|_| ISSUES_URL.to_owned())
}

/// The report for `title` and `description`, shortened until its address fits.
pub fn bug_report(title: &str, description: &str, details: &str) -> BugReport {
    let title: String = title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_TITLE_CHARS)
        .collect();
    let title = match title.trim() {
        "" => "Bug: ".to_owned(),
        t => t.to_owned(),
    };
    let full = body(description, details, false);
    let url = address(&title, &full);
    if url.len() <= gmm_xxmi::MAX_PAGE_URL {
        return BugReport {
            title,
            body: full,
            url,
            shortened: false,
        };
    }
    // The longest prefix of the description that still fits (whole characters).
    let chars: Vec<char> = description.trim().chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let text: String = chars[..mid].iter().collect();
        if address(&title, &body(&format!("{text}…"), details, true)).len()
            <= gmm_xxmi::MAX_PAGE_URL
        {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let text: String = chars[..lo].iter().collect();
    let body = body(&format!("{text}…"), details, true);
    BugReport {
        url: address(&title, &body),
        title,
        body,
        shortened: true,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_prefilled_issue_address() {
        let r = bug_report(
            "Install fails",
            "It says the archive is damaged.",
            "- GMM 0.1.0",
        );
        assert!(!r.shortened);
        assert!(r.url.starts_with(
            "https://github.com/clorece/gacha-mod-manager/issues/new?title=Install+fails&body="
        ));
        assert!(r.url.ends_with("&labels=bug"));
        let parsed = Url::parse(&r.url).unwrap();
        let body = parsed.query_pairs().find(|(k, _)| k == "body").unwrap().1;
        assert_eq!(body, r.body);
        assert!(r.body.contains("It says the archive is damaged."));
        assert!(r.body.contains("- GMM 0.1.0"));
    }

    #[test]
    fn shortens_long_descriptions_to_fit_the_opener() {
        let long = "ä mod failed ".repeat(400);
        let r = bug_report(&"t".repeat(500), &long, "- GMM 0.1.0");
        assert!(r.shortened);
        assert_eq!(r.title.chars().count(), MAX_TITLE_CHARS);
        assert!(r.url.len() <= gmm_xxmi::MAX_PAGE_URL, "{}", r.url.len());
        assert!(
            r.url.len() > gmm_xxmi::MAX_PAGE_URL - 40,
            "uses the room it has"
        );
        assert!(r.body.contains("Shortened to fit the link"));
        assert!(gmm_xxmi::check_page_url(&r.url).is_ok());
    }

    #[test]
    fn an_empty_report_still_opens_the_form() {
        let r = bug_report(" \n", "", "- GMM 0.1.0");
        assert_eq!(r.title, "Bug: ");
        assert!(r.body.contains("Describe what happened"));
    }
}
