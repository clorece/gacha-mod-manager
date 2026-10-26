// SPDX-License-Identifier: MIT OR Apache-2.0

//! Names inside `<XXMI>/<IMP>/Mods/` (docs/architecture.md, "Install & toggle model").

use gmm_fsguard::RelPath;

/// GMM's own folder inside `Mods/`. Everything else in `Mods/` is the user's.
pub const GMM_DIR: &str = "GMM";
/// Where installs are assembled before they're renamed into place. XXMI's `d3dx.ini` has
/// `exclude_recursive = DISABLED*`, so 3DMigoto never loads anything in here, even halfway
/// through an install.
pub const STAGING_DIR: &str = "DISABLED .staging";
/// XXMI's own convention for a switched-off mod folder.
pub const DISABLED_PREFIX: &str = "DISABLED ";
/// Written into every mod folder GMM installs. 3DMigoto only reads `.ini` files.
pub const SIDECAR_FILE: &str = "gmm.json";

const MAX_BASE_CHARS: usize = 80;
const MAX_VARIANT_CHARS: usize = 48;

/// The folder name for a mod: `<name> [gb-<id>]`, plus ` (<variant>)` when one of several
/// variants is installed. Chosen once at install and never changed afterwards, because
/// 3DMigoto keys each mod's saved toggle state by its path.
///
/// GameBanana names are untrusted: characters Windows doesn't allow are replaced, a leading
/// `DISABLED` is dropped (it would hide the mod), and the result is length-limited and
/// checked like any other path component.
pub fn mod_folder_name(name: &str, gb_mod_id: Option<u64>, variant: Option<&str>) -> String {
    let mut base = clean(name, MAX_BASE_CHARS);
    if base.is_empty() {
        base = "Mod".to_owned();
    }
    let mut out = base;
    if let Some(id) = gb_mod_id {
        out.push_str(&format!(" [gb-{id}]"));
    }
    if let Some(variant) = variant.map(|v| clean(v, MAX_VARIANT_CHARS))
        && !variant.is_empty()
    {
        out.push_str(&format!(" ({variant})"));
    }
    if valid_component(&out) {
        out
    } else {
        // E.g. a bare reserved device name like `CON` without an id.
        let fallback = mod_folder_name("Mod", gb_mod_id, None);
        if valid_component(&fallback) {
            fallback
        } else {
            "Mod".to_owned()
        }
    }
}

/// The name on disk: `folder_name`, or `DISABLED folder_name` when switched off.
pub fn on_disk_name(folder_name: &str, enabled: bool) -> String {
    if enabled {
        folder_name.to_owned()
    } else {
        format!("{DISABLED_PREFIX}{folder_name}")
    }
}

fn valid_component(name: &str) -> bool {
    RelPath::parse(name).is_ok_and(|p| p.depth() == 1)
}

fn clean(input: &str, max_chars: usize) -> String {
    let replaced: String = input
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut s = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    // Leading dots would make the folder hidden on Linux; a leading DISABLED would make
    // XXMI skip it.
    loop {
        let trimmed = s.trim_start_matches(['.', ' ', '-', '_']);
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("disabled") {
            s = trimmed["disabled".len()..].to_owned();
        } else {
            s = trimmed.to_owned();
            break;
        }
    }
    let mut s: String = s.chars().take(max_chars).collect();
    while s.ends_with(['.', ' ']) {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names() {
        assert_eq!(
            mod_folder_name("Ellen - Maid Outfit", Some(652_373), None),
            "Ellen - Maid Outfit [gb-652373]"
        );
        assert_eq!(
            mod_folder_name("Ellen", Some(1), Some("Variant B")),
            "Ellen [gb-1] (Variant B)"
        );
        assert_eq!(
            mod_folder_name("a/b\\c:d*e?\"f<g>|h", Some(1), None),
            "a b c d e f g h [gb-1]"
        );
        assert_eq!(
            mod_folder_name("DISABLED  Miyabi", Some(2), None),
            "Miyabi [gb-2]"
        );
        assert_eq!(mod_folder_name("disabled_disabled-x", None, None), "x");
        assert_eq!(
            mod_folder_name("...hidden.", Some(3), None),
            "hidden [gb-3]"
        );
        assert_eq!(mod_folder_name("   ", Some(4), None), "Mod [gb-4]");
        assert_eq!(mod_folder_name("CON", None, None), "Mod");
        assert_eq!(
            mod_folder_name("tab\there\nnew", None, None),
            "tab here new"
        );
        let long = "x".repeat(500);
        assert_eq!(
            mod_folder_name(&long, Some(5), None).chars().count(),
            80 + " [gb-5]".len()
        );
    }

    #[test]
    fn disk_names() {
        assert_eq!(on_disk_name("A [gb-1]", true), "A [gb-1]");
        assert_eq!(on_disk_name("A [gb-1]", false), "DISABLED A [gb-1]");
    }
}
