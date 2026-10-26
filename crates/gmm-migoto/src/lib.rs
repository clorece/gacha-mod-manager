// SPDX-License-Identifier: MIT OR Apache-2.0

//! A tolerant reader for 3DMigoto mod `.ini` files, as XXMI's importers load them.
//!
//! GMM only needs a few facts from a mod's ini files (docs/architecture.md, "Conflict
//! detection"):
//!
//! - which resources it overrides: `[TextureOverride*]` / `[ShaderOverride*]` sections with
//!   their `hash`, optional `match_first_index`, and whether the section replaces geometry
//!   (`ib`, `vb<n>`, `handling`, vertex count overrides, draw calls) or textures
//!   (`ps-t<n>`, `vs-t<n>`, ...);
//! - the `namespace` it declares (shared library mods are recognised by it);
//! - its key bindings (`[Key*]` sections), for the keybind viewer.
//!
//! The format is loose and mods are untrusted, so parsing never fails: lines that don't fit
//! are skipped. Section names and keys are case-insensitive. Lines starting with `;` are
//! comments. Control flow (`if`/`elif`/`else`/`endif`) is ignored: a key counts wherever it
//! appears in its section, since GMM can't know which branch runs.
//!
//! This crate does no file access; callers read the text through `gmm-fsguard`.

use serde::Serialize;

/// Bigger inputs are cut here; real mod inis are well below.
pub const MAX_INI_BYTES: usize = 4 * 1024 * 1024;

/// What kind of section a `[Name]` header opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    TextureOverride,
    ShaderOverride,
    Key,
    Other,
}

/// What an override section replaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Replaces {
    /// Index/vertex buffers, draw handling or vertex counts: the model itself.
    pub geometry: bool,
    /// Shader texture slots: how the model looks.
    pub textures: bool,
}

impl Replaces {
    pub fn any(self) -> bool {
        self.geometry || self.textures
    }
}

/// One `[TextureOverride*]` or `[ShaderOverride*]` section with a hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Override {
    /// The section name without brackets, as written.
    pub section: String,
    pub kind: SectionKind,
    /// Lowercase, as written (usually 8 hex digits).
    pub hash: String,
    /// `match_first_index`, when the override only applies to part of an index buffer.
    pub first_index: Option<u64>,
    pub replaces: Replaces,
}

impl Override {
    /// Whether two overrides hit the same thing in game: same hash, and the same part of the
    /// buffer (an override without `match_first_index` covers every part).
    pub fn overlaps(&self, other: &Self) -> bool {
        self.hash == other.hash
            && match (self.first_index, other.first_index) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

/// One `[Key*]` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyBinding {
    pub section: String,
    /// Every `key = ...` line (a binding can have several).
    pub keys: Vec<String>,
    pub back: Vec<String>,
    /// `type = cycle | toggle | hold | activate` (as written, lowercase).
    pub kind: Option<String>,
}

/// What GMM keeps from one ini file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IniSummary {
    pub namespace: Option<String>,
    pub overrides: Vec<Override>,
    pub keys: Vec<KeyBinding>,
    /// Number of `[...]` sections, of any kind.
    pub sections: usize,
}

struct Open {
    name: String,
    kind: SectionKind,
    hash: Option<String>,
    first_index: Option<u64>,
    replaces: Replaces,
    keys: Vec<String>,
    back: Vec<String>,
    key_type: Option<String>,
}

impl Open {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            kind: section_kind(name),
            hash: None,
            first_index: None,
            replaces: Replaces::default(),
            keys: Vec::new(),
            back: Vec::new(),
            key_type: None,
        }
    }
}

/// Reads an ini file's text. Never fails; see the crate docs.
pub fn parse(text: &str) -> IniSummary {
    let text = truncate(text, MAX_INI_BYTES);
    let mut out = IniSummary::default();
    let mut current: Option<Open> = None;
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                continue;
            };
            finish(&mut out, current.take());
            out.sections += 1;
            current = Some(Open::new(rest[..end].trim()));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match current.as_mut() {
            None => {
                if key == "namespace" && !value.is_empty() {
                    out.namespace = Some(value.to_owned());
                }
            }
            Some(section) => apply(section, &key, value),
        }
    }
    finish(&mut out, current);
    out
}

fn apply(section: &mut Open, key: &str, value: &str) {
    match section.kind {
        SectionKind::TextureOverride | SectionKind::ShaderOverride => match key {
            "hash" => {
                if !value.is_empty() {
                    section.hash = Some(value.to_ascii_lowercase());
                }
            }
            "match_first_index" => section.first_index = value.parse().ok(),
            _ => {
                if is_geometry_key(key) {
                    section.replaces.geometry = true;
                } else if is_texture_key(key) {
                    section.replaces.textures = true;
                }
            }
        },
        SectionKind::Key => match key {
            "key" => section.keys.push(value.to_owned()),
            "back" => section.back.push(value.to_owned()),
            "type" => section.key_type = Some(value.to_ascii_lowercase()),
            _ => {}
        },
        SectionKind::Other => {}
    }
}

fn finish(out: &mut IniSummary, section: Option<Open>) {
    let Some(s) = section else {
        return;
    };
    match s.kind {
        SectionKind::TextureOverride | SectionKind::ShaderOverride => {
            if let Some(hash) = s.hash {
                out.overrides.push(Override {
                    section: s.name,
                    kind: s.kind,
                    hash,
                    first_index: s.first_index,
                    replaces: s.replaces,
                });
            }
        }
        SectionKind::Key => out.keys.push(KeyBinding {
            section: s.name,
            keys: s.keys,
            back: s.back,
            kind: s.key_type,
        }),
        SectionKind::Other => {}
    }
}

fn section_kind(name: &str) -> SectionKind {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("textureoverride") {
        SectionKind::TextureOverride
    } else if lower.starts_with("shaderoverride") {
        SectionKind::ShaderOverride
    } else if lower.starts_with("key") {
        SectionKind::Key
    } else {
        SectionKind::Other
    }
}

/// `ib`, `vb0`..., `handling`, vertex/stride overrides and draw calls.
fn is_geometry_key(key: &str) -> bool {
    key == "ib"
        || slot(key, "vb")
        || matches!(
            key,
            "handling"
                | "override_vertex_count"
                | "override_byte_stride"
                | "draw"
                | "drawindexed"
                | "drawindexedinstanced"
                | "drawinstanced"
        )
}

/// Texture slots of any shader stage: `ps-t0`, `vs-t3`, `cs-t1`...
fn is_texture_key(key: &str) -> bool {
    ["ps-t", "vs-t", "gs-t", "hs-t", "ds-t", "cs-t"]
        .iter()
        .any(|prefix| slot(key, prefix))
}

/// `prefix` followed by one or more digits.
fn slot(key: &str, prefix: &str) -> bool {
    key.strip_prefix(prefix)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots() {
        assert!(slot("vb0", "vb"));
        assert!(slot("ps-t12", "ps-t"));
        assert!(!slot("vb", "vb"));
        assert!(!slot("vbx", "vb"));
        assert!(is_geometry_key("ib"));
        assert!(!is_geometry_key("checktextureoverride"));
        assert!(is_texture_key("ps-t100"));
        assert!(!is_texture_key("ps-cb0"));
    }

    #[test]
    fn truncates_on_char_boundaries() {
        assert_eq!(truncate("aé", 2), "a");
        assert_eq!(truncate("abc", 10), "abc");
    }
}
