// SPDX-License-Identifier: MIT OR Apache-2.0

//! One importer folder (`<XXMI>/ZZMI/`, ...): installed? intact? which versions? how many mods?

use gmm_adapters::GameManifest;
use gmm_fsguard::{Guard, RelPath, ScopedPath};
use serde::Serialize;

use crate::find::XxmiInstall;

/// Files every importer has once XXMI installed it (observed for ZZMI and WWMI).
const CRITICAL_FILES: &[&str] = &["d3d11.dll", "d3dx.ini"];
const MAX_INI_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DLL_BYTES: u64 = 64 * 1024 * 1024;
/// Stop walking `Mods/` after this many entries (sizes are then reported as "at least").
const MAX_WALK_ENTRIES: usize = 200_000;
/// GMM's folder inside `Mods/` and its staging folder (names owned by `gmm-core`; repeated
/// here because `gmm-xxmi` sits below it).
const GMM_DIR: &str = "GMM";
const GMM_STAGING_DIR: &str = "DISABLED .staging";

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ImporterInfo {
    pub folder_exists: bool,
    /// Critical files that are missing (relative to the importer folder).
    pub missing_files: Vec<String>,
    /// `target = ...` from `d3dx.ini`.
    pub d3dx_target: Option<String>,
    /// Whether `d3dx.ini` loads mods from `Mods/` (`include_recursive = Mods`).
    pub d3dx_loads_mods: bool,
    /// `global $version` from `Core/<IMP>/main.ini`.
    pub importer_version: Option<String>,
    /// File version of the XXMI DLL (`d3d11.dll`).
    pub dll_version: Option<String>,
    pub mods: ModsSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ModsSummary {
    /// Top-level folders in `Mods/` (each is usually one mod).
    pub enabled: usize,
    /// Top-level folders whose name starts with `DISABLED`.
    pub disabled: usize,
    pub total_bytes: u64,
    /// True when the walk stopped early.
    pub truncated: bool,
}

pub fn inspect_importer(xxmi: &XxmiInstall, game: &GameManifest) -> ImporterInfo {
    let guard = &xxmi.guard;
    let importer = game.xxmi.importer.as_str();
    let Ok(dir) = guard.path(xxmi.root_id, importer) else {
        return ImporterInfo::default();
    };
    let mut info = ImporterInfo {
        folder_exists: guard.metadata(&dir).is_ok_and(|m| m.is_dir()),
        ..ImporterInfo::default()
    };
    if !info.folder_exists {
        return info;
    }
    for file in CRITICAL_FILES {
        if !dir
            .join(file)
            .is_ok_and(|p| guard.exists(&p).unwrap_or(false))
        {
            info.missing_files.push((*file).to_owned());
        }
    }
    // The importer's core ini differs per importer (ZZMI: main.ini, WWMI:
    // WuWa-Model-Importer.ini; observed), so take it from d3dx.ini's own include line.
    let mut core_ini = None;
    if let Some(text) = read_small_text(guard, &dir, "d3dx.ini") {
        let d3dx = parse_d3dx(&text);
        info.d3dx_target = d3dx.target;
        info.d3dx_loads_mods = d3dx.loads_mods;
        let prefix = format!("core/{}/", importer.to_ascii_lowercase());
        core_ini = d3dx
            .includes
            .into_iter()
            .filter_map(|inc| RelPath::parse(&inc).ok())
            .find(|p| {
                p.to_string().to_ascii_lowercase().starts_with(&prefix)
                    && p.file_name()
                        .is_some_and(|n| n.to_ascii_lowercase().ends_with(".ini"))
            });
        match &core_ini {
            Some(ini) if !guard.exists(&dir.join_rel(ini)).unwrap_or(false) => {
                info.missing_files.push(ini.to_string());
            }
            None => info.missing_files.push(format!(
                "Core/{importer}/<core>.ini (d3dx.ini includes none)"
            )),
            Some(_) => {}
        }
    }
    if let Some(text) = core_ini.and_then(|ini| read_small_text(guard, &dir, &ini.to_string())) {
        info.importer_version = parse_importer_version(&text);
    }
    info.dll_version = dll_version(guard, &dir);
    info.mods = summarize_mods(guard, &dir);
    info
}

fn read_small_text(guard: &Guard, dir: &ScopedPath, rel: &str) -> Option<String> {
    let path = dir.join(rel).ok()?;
    if guard.metadata(&path).ok()?.len() > MAX_INI_BYTES {
        return None;
    }
    Some(String::from_utf8_lossy(&guard.read(&path).ok()?).into_owned())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct D3dx {
    /// `target = <exe>`
    pub target: Option<String>,
    /// `include_recursive = Mods` present
    pub loads_mods: bool,
    /// Every `include = <file>`, in order.
    pub includes: Vec<String>,
}

/// The few `d3dx.ini` keys the setup check needs. `;` starts a comment.
pub(crate) fn parse_d3dx(text: &str) -> D3dx {
    let mut out = D3dx::default();
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.eq_ignore_ascii_case("target") && out.target.is_none() {
            out.target = Some(value.to_owned());
        } else if key.eq_ignore_ascii_case("include_recursive")
            && value.eq_ignore_ascii_case("Mods")
        {
            out.loads_mods = true;
        } else if key.eq_ignore_ascii_case("include") && !value.is_empty() {
            out.includes.push(value.to_owned());
        }
    }
    out
}

/// The importer's own version from its core ini: `global $version = 1.50` (ZZMI) or
/// `global $<name>_version = 1.10` (WWMI: `$wwmi_version`). `$required_*` keys are ignored.
pub(crate) fn parse_importer_version(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.split(';').next()?.trim();
        let rest = line.strip_prefix("global")?.trim_start();
        let (key, value) = rest.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let is_version = key == "$version"
            || (key.starts_with('$') && key.ends_with("_version") && !key.starts_with("$required"));
        is_version
            .then(|| value.trim().to_owned())
            .filter(|v| !v.is_empty())
    })
}

fn dll_version(guard: &Guard, dir: &ScopedPath) -> Option<String> {
    let path = dir.join("d3d11.dll").ok()?;
    if guard.metadata(&path).ok()?.len() > MAX_DLL_BYTES {
        return None;
    }
    let bytes = guard.read(&path).ok()?;
    let pe = pelite::PeFile::from_bytes(&bytes).ok()?;
    let fixed = pe.resources().ok()?.version_info().ok()?.fixed()?;
    let v = fixed.dwFileVersion;
    Some(format!("{}.{}.{}.{}", v.Major, v.Minor, v.Patch, v.Build))
}

fn summarize_mods(guard: &Guard, dir: &ScopedPath) -> ModsSummary {
    let mut summary = ModsSummary::default();
    let Ok(mods) = dir.join("Mods") else {
        return summary;
    };
    let Ok(top) = guard.read_dir(&mods) else {
        return summary;
    };
    let mut stack = Vec::new();
    let count = |name: &str, summary: &mut ModsSummary| {
        if name.to_ascii_lowercase().starts_with("disabled") {
            summary.disabled += 1;
        } else if !name.starts_with('.') {
            summary.enabled += 1;
        }
    };
    for entry in top {
        if !entry.is_dir {
            summary.total_bytes += entry.len;
            continue;
        }
        // GMM's own folder holds one mod per sub-folder (plus its staging folder).
        if entry.name.eq_ignore_ascii_case(GMM_DIR) && !entry.is_symlink {
            for inner in guard.read_dir(&entry.path).unwrap_or_default() {
                if !inner.is_dir {
                    summary.total_bytes += inner.len;
                    continue;
                }
                if !inner.name.eq_ignore_ascii_case(GMM_STAGING_DIR) {
                    count(&inner.name, &mut summary);
                }
                stack.push(inner.path);
            }
            continue;
        }
        count(&entry.name, &mut summary);
        stack.push(entry.path);
    }
    let mut seen = 0usize;
    while let Some(next) = stack.pop() {
        let Ok(entries) = guard.read_dir(&next) else {
            continue;
        };
        for entry in entries {
            seen += 1;
            if seen > MAX_WALK_ENTRIES {
                summary.truncated = true;
                return summary;
            }
            // Don't follow links: fsguard would refuse escapes anyway, and sizes would double.
            if entry.is_symlink {
                continue;
            }
            if entry.is_dir {
                stack.push(entry.path);
            } else {
                summary.total_bytes += entry.len;
            }
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_d3dx() {
        let d3dx = "[Loader]\ntarget = ZenlessZoneZero.exe ; the game\nloader = XXMI Launcher.exe\n\
                    [Include]\ninclude = Core\\ZZMI\\main.ini\ninclude_recursive = Mods\n\
                    exclude_recursive = DISABLED*\n";
        assert_eq!(
            parse_d3dx(d3dx),
            D3dx {
                target: Some("ZenlessZoneZero.exe".to_owned()),
                loads_mods: true,
                includes: vec![r"Core\ZZMI\main.ini".to_owned()],
            }
        );
        assert_eq!(
            parse_d3dx("; target = X\n"),
            D3dx::default(),
            "comments ignored"
        );
    }

    #[test]
    fn reads_importer_versions() {
        // ZZMI (observed): Core\ZZMI\main.ini
        assert_eq!(
            parse_importer_version("[Constants]\nglobal $version = 1.50\n").as_deref(),
            Some("1.50")
        );
        // WWMI (observed): Core\WWMI\WuWa-Model-Importer.ini
        assert_eq!(
            parse_importer_version(
                "global $required_wwmi_version = 0\nglobal $wwmi_version = 1.10\n"
            )
            .as_deref(),
            Some("1.10")
        );
        assert_eq!(parse_importer_version("global persist $other = 1\n"), None);
    }
}
