// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;
use std::path::PathBuf;

use crate::error::PathError;

/// Longest single path component we accept (NTFS and ext4 both cap names at 255).
const MAX_COMPONENT_BYTES: usize = 255;

/// Device names Windows reserves in every directory, with or without an extension.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A validated relative path.
///
/// Accepts `/` or `\` as separators and drops empty and `.` components. Rejects anything that
/// could point outside the directory it is joined to, or that Windows would interpret
/// differently than it reads: `..`, absolute paths, drive letters, alternate data streams,
/// reserved device names, and names ending in a dot or space.
///
/// The same rules apply to entry names inside mod archives.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct RelPath {
    components: Vec<String>,
}

impl RelPath {
    /// The empty path (the root directory itself).
    pub fn root() -> Self {
        Self::default()
    }

    pub fn parse(input: &str) -> Result<Self, PathError> {
        if input.contains('\0') {
            return Err(PathError::Nul);
        }
        let normalized = input.replace('\\', "/");
        if normalized.starts_with('/') {
            return Err(PathError::Absolute);
        }
        let mut components = Vec::new();
        for part in normalized.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            validate_component(part)?;
            components.push(part.to_owned());
        }
        Ok(Self { components })
    }

    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    pub fn depth(&self) -> usize {
        self.components.len()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.components.last().map(String::as_str)
    }

    pub fn parent(&self) -> Option<Self> {
        let (_, rest) = self.components.split_last()?;
        Some(Self {
            components: rest.to_vec(),
        })
    }

    pub fn join(&self, other: &Self) -> Self {
        let mut components = self.components.clone();
        components.extend(other.components.iter().cloned());
        Self { components }
    }

    /// Path relative to the root, using the platform separator.
    pub fn to_path_buf(&self) -> PathBuf {
        self.components.iter().collect()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.components.join("/"))
    }
}

fn validate_component(part: &str) -> Result<(), PathError> {
    if part == ".." {
        return Err(PathError::ParentDir);
    }
    if part.len() > MAX_COMPONENT_BYTES {
        return Err(PathError::ComponentTooLong {
            max: MAX_COMPONENT_BYTES,
        });
    }
    if part.contains(':') {
        return Err(PathError::Colon(part.to_owned()));
    }
    if part
        .chars()
        .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(PathError::InvalidChar(part.to_owned()));
    }
    // Windows silently strips trailing dots and spaces, so "a." and "a" would be the same file.
    if part.ends_with('.') || part.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace(part.to_owned()));
    }
    let stem = part.split('.').next().unwrap_or(part).trim_end();
    if RESERVED_NAMES
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name))
    {
        return Err(PathError::Reserved(part.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Component;

    use proptest::prelude::*;

    use super::*;

    #[test]
    fn accepts_and_normalizes() {
        let p = RelPath::parse(r"Mods\GMM/./Ellen - Maid [gb-1]//Ellen.ini").unwrap();
        assert_eq!(p.to_string(), "Mods/GMM/Ellen - Maid [gb-1]/Ellen.ini");
        assert_eq!(p.depth(), 4);
        assert_eq!(p.file_name(), Some("Ellen.ini"));
        assert_eq!(
            p.parent().unwrap().to_string(),
            "Mods/GMM/Ellen - Maid [gb-1]"
        );
    }

    #[test]
    fn empty_is_root() {
        assert!(RelPath::parse("").unwrap().is_root());
        assert!(RelPath::parse("./").unwrap().is_root());
        assert_eq!(RelPath::root().parent(), None);
    }

    #[test]
    fn rejects_escapes() {
        assert_eq!(RelPath::parse("../x"), Err(PathError::ParentDir));
        assert_eq!(RelPath::parse(r"a\..\..\x"), Err(PathError::ParentDir));
        assert_eq!(RelPath::parse("/etc/passwd"), Err(PathError::Absolute));
        assert_eq!(RelPath::parse(r"\\server\share"), Err(PathError::Absolute));
        assert!(matches!(
            RelPath::parse("C:/Windows"),
            Err(PathError::Colon(_))
        ));
        assert!(matches!(RelPath::parse("C:x"), Err(PathError::Colon(_))));
        assert!(matches!(
            RelPath::parse("a.ini:hidden"),
            Err(PathError::Colon(_))
        ));
        assert_eq!(RelPath::parse("a\0b"), Err(PathError::Nul));
    }

    #[test]
    fn rejects_windows_quirks() {
        for name in ["CON", "con.txt", "Nul", "COM1.ini", "lpt9", "AUX .dds"] {
            assert!(
                matches!(RelPath::parse(name), Err(PathError::Reserved(_))),
                "{name} should be reserved"
            );
        }
        for name in ["CONSOLE", "COM10", "nullable.ini", "auxiliary"] {
            assert!(RelPath::parse(name).is_ok(), "{name} should be allowed");
        }
        assert!(matches!(
            RelPath::parse("a."),
            Err(PathError::TrailingDotOrSpace(_))
        ));
        assert!(matches!(
            RelPath::parse("a "),
            Err(PathError::TrailingDotOrSpace(_))
        ));
        assert!(matches!(
            RelPath::parse("..."),
            Err(PathError::TrailingDotOrSpace(_))
        ));
        assert!(matches!(
            RelPath::parse("a?b"),
            Err(PathError::InvalidChar(_))
        ));
        assert!(matches!(
            RelPath::parse("a\u{7}b"),
            Err(PathError::InvalidChar(_))
        ));
        assert!(matches!(
            RelPath::parse(&"x".repeat(256)),
            Err(PathError::ComponentTooLong { .. })
        ));
    }

    proptest! {
        // Whatever the input, a successfully parsed path only ever has plain name components.
        #[test]
        fn parsed_paths_never_escape(input in ".{0,64}") {
            if let Ok(p) = RelPath::parse(&input) {
                for c in p.to_path_buf().components() {
                    prop_assert!(matches!(c, Component::Normal(_)), "bad component {c:?}");
                }
                prop_assert!(p.components().iter().all(|c| !c.contains(['/', '\\', ':'])));
            }
        }

        #[test]
        fn display_round_trips(input in "[a-zA-Z0-9 _()\\[\\]-]{1,12}(/[a-zA-Z0-9 _()-]{1,12}){0,4}") {
            if let Ok(p) = RelPath::parse(&input) {
                prop_assert_eq!(RelPath::parse(&p.to_string()).unwrap(), p);
            }
        }
    }
}
