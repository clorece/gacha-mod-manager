// SPDX-License-Identifier: MIT OR Apache-2.0

//! Names GMM must never open, even inside an allowed root (security rule R4).

use std::path::{Component, Path};

/// Exact names (case-insensitive) of files and folders that hold credentials or personal data.
const DENIED_NAMES: &[&str] = &[
    // Steam login and account data
    "loginusers.vdf",
    "config.vdf",
    "local.vdf",
    "userdata",
    // Game logs can contain account details
    "player.log",
    "player-prev.log",
    "output_log.txt",
    // Credentials
    ".ssh",
];

/// Name prefixes (case-insensitive). `ssfn*` files are Steam sentry/auth files.
const DENIED_PREFIXES: &[&str] = &["ssfn"];

pub(crate) fn is_denied_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    DENIED_NAMES.contains(&lower.as_str()) || DENIED_PREFIXES.iter().any(|p| lower.starts_with(p))
}

/// Returns the first denied component of `path`, if any.
pub(crate) fn denied_component(path: &Path) -> Option<String> {
    path.components().find_map(|c| match c {
        Component::Normal(os) => os
            .to_str()
            .filter(|name| is_denied_name(name))
            .map(str::to_owned),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_case_insensitively() {
        assert!(is_denied_name("LoginUsers.vdf"));
        assert!(is_denied_name("ssfn123456789"));
        assert!(is_denied_name("Player.log"));
        assert!(!is_denied_name("libraryfolders.vdf"));
        assert!(!is_denied_name("appmanifest_4162040.acf"));
        assert!(!is_denied_name("config.ini"));
    }

    #[test]
    fn finds_component_anywhere() {
        let p = Path::new("steam").join("config").join("loginusers.vdf");
        assert_eq!(denied_component(&p).as_deref(), Some("loginusers.vdf"));
        let p = Path::new("steam")
            .join("userdata")
            .join("123")
            .join("x.cfg");
        assert_eq!(denied_component(&p).as_deref(), Some("userdata"));
        assert_eq!(
            denied_component(Path::new("steam/steamapps/libraryfolders.vdf")),
            None
        );
    }
}
