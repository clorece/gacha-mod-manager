// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plain-text output for the terminal.

use std::fmt::Write;

use gmm_catalog::{Category, ModFile, ModProfile, ModSummary, Page};

pub fn mod_page(page: &Page<ModSummary>, hidden: usize) -> String {
    let mut out = String::new();
    for m in &page.items {
        let character = m.category.as_ref().map(|c| c.name.as_str()).unwrap_or("-");
        let flag = if m.visibility.is_sensitive() || m.has_content_ratings {
            " [NSFW?]"
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "{:>8}  {}{}\n          by {} · {} · ♥ {} · 👁 {} · updated {}",
            m.id,
            m.name,
            flag,
            m.submitter.name,
            character,
            m.stats.likes,
            m.stats.views,
            date(m.date_updated.unwrap_or(m.date_modified)),
        );
    }
    if hidden > 0 {
        let _ = writeln!(
            out,
            "\n({hidden} sensitive mod(s) on this page hidden; use --show-nsfw to include them)"
        );
    }
    let _ = writeln!(
        out,
        "\npage {} · {} per page · {} mods total{}",
        page.page,
        page.per_page,
        page.total,
        if page.is_last { " · last page" } else { "" }
    );
    out
}

pub fn profile(p: &ModProfile) -> String {
    let s = &p.summary;
    let mut out = String::new();
    let _ = writeln!(out, "{}  (mod {})", s.name, s.id);
    let _ = writeln!(out, "by {}", s.submitter.name);
    let category = [&s.root_category, &s.category]
        .iter()
        .filter_map(|c| c.as_ref().map(|c| c.name.as_str()))
        .collect::<Vec<_>>()
        .join(" › ");
    if !category.is_empty() {
        let _ = writeln!(out, "{category}");
    }
    let _ = writeln!(
        out,
        "added {} · updated {} · ♥ {} · ⬇ {} · 👁 {}",
        date(s.date_added),
        date(s.date_updated.unwrap_or(s.date_modified)),
        s.stats.likes,
        s.stats.downloads.unwrap_or(0),
        s.stats.views,
    );
    if s.visibility.is_sensitive() || s.has_content_ratings {
        let _ = writeln!(out, "content warning: may be NSFW");
    }
    if !p.is_installable() {
        let _ = writeln!(out, "not installable (trashed, withheld or obsolete)");
    }
    let _ = writeln!(out, "{} preview image(s)", s.images.len());
    if let Some(banner) = s.banner() {
        let _ = writeln!(out, "banner: {}", banner.url_for_width(530));
    }
    if !p.license.yes.is_empty() || !p.license.no.is_empty() {
        let _ = writeln!(
            out,
            "license: yes: {} | ask: {} | no: {}",
            p.license.yes.join(", "),
            p.license.ask.join(", "),
            p.license.no.join(", ")
        );
    }
    for group in &p.credits {
        let names: Vec<String> = group
            .authors
            .iter()
            .map(|a| match &a.role {
                Some(role) => format!("{} ({role})", a.name),
                None => a.name.clone(),
            })
            .collect();
        let _ = writeln!(out, "{}: {}", group.name, names.join(", "));
    }
    let _ = writeln!(out, "\nfiles:");
    let default = p.files.default_choice().map(|f| f.id);
    for f in &p.files.files {
        file_line(&mut out, f, default == Some(f.id));
    }
    if !p.files.archived_files.is_empty() {
        let _ = writeln!(out, "archived files:");
        for f in &p.files.archived_files {
            file_line(&mut out, f, false);
        }
    }
    let _ = writeln!(out, "\n{}", s.profile_url);
    out
}

fn file_line(out: &mut String, f: &ModFile, is_default: bool) {
    let scan = if f.is_clean() {
        "clean".to_owned()
    } else {
        format!(
            "not cleared ({})",
            f.av_result.as_deref().unwrap_or("pending")
        )
    };
    let _ = writeln!(
        out,
        "  {} {:>9}  {}  {}  v{}  {}  {}",
        if is_default { "*" } else { " " },
        f.id,
        f.name,
        bytes(f.size),
        f.version.as_deref().unwrap_or("?"),
        date(f.date_added),
        scan,
    );
    if let Some(d) = &f.description {
        let _ = writeln!(out, "              {d}");
    }
}

pub fn categories(cats: &[Category]) -> String {
    let mut out = String::new();
    for c in cats {
        let _ = writeln!(
            out,
            "{:>8}  {}  ({} mods{})",
            c.id,
            c.name,
            c.item_count,
            if c.subcategory_count > 0 {
                format!(", {} subcategories", c.subcategory_count)
            } else {
                String::new()
            }
        );
    }
    out
}

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Unix seconds to `YYYY-MM-DD` (UTC), without a date library.
pub fn date(ts: i64) -> String {
    if ts <= 0 {
        return "?".to_owned();
    }
    // Howard Hinnant's civil-from-days algorithm.
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_sizes() {
        assert_eq!(date(0), "?");
        assert_eq!(date(86_400), "1970-01-02");
        assert_eq!(date(1_792_719_184), "2026-10-23");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(27_757_910), "26.5 MiB");
    }
}
