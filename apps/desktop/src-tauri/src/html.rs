// SPDX-License-Identifier: MIT OR Apache-2.0

//! The address rewrite for GameBanana's description HTML (docs/security.md, T3).
//!
//! This is not the sanitizer: the webview runs the result through DOMPurify (strict
//! allowlist), and the CSP stops anything that slips through from loading. What happens here
//! is that every address leaves the HTML before the webview sees it: image `src`s become
//! `gmmimg` URLs for keys (GameBanana's image and file hosts only; others are dropped),
//! link `href`s become `#gmm-link:<key>` for the `open_link` command, and `srcset` is
//! dropped. The scanner walks tags and their attributes (quoted, single-quoted or bare
//! values) and leaves everything else, text and comments included, as it was.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

/// The `href` prefix the webview recognises as a link to open through `open_link`.
pub const LINK_PREFIX: &str = "#gmm-link:";

/// Link addresses handed out as keys in this run (like the image keys).
#[derive(Default)]
pub struct Links(Mutex<HashMap<String, String>>);

impl Links {
    /// The key for an `http(s)` address, or `None` for anything else (`javascript:`,
    /// `file:`, relative links...).
    pub fn register(&self, url: &str) -> Option<String> {
        let url = url.trim();
        let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
        if !(lower.starts_with("https://") || lower.starts_with("http://"))
            || url.len() > gmm_xxmi::MAX_PAGE_URL
        {
            return None;
        }
        let key = hex::encode(&Sha256::digest(url.as_bytes())[..16]);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.clone(), url.to_owned());
        Some(key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .cloned()
    }
}

/// Rewrites the address attributes of every tag in `html`. `image` maps an image URL to the
/// URL the webview should load (or `None` to drop the attribute); links go through `links`.
pub fn rewrite(html: &str, image: impl Fn(&str) -> Option<String>, links: &Links) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").map_or(rest.len(), |i| i + 4 + 3);
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        let end = tag_end(rest);
        match rest.as_bytes().get(1) {
            // A start tag: the only kind whose attributes count.
            Some(b) if b.is_ascii_alphabetic() => {
                rewrite_tag(&rest[..end], &image, links, &mut out)
            }
            // End tags and `<!doctype>`-like markup, as they are.
            Some(b'/' | b'!' | b'?') => out.push_str(&rest[..end]),
            // `a < b`: text, as in a browser.
            _ => {
                out.push('<');
                rest = &rest[1..];
                continue;
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Length of the tag at the start of `s` (through its `>`, quotes respected; the whole rest
/// if it never closes).
fn tag_end(s: &str) -> usize {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return i + 1,
            _ => {}
        }
    }
    s.len()
}

fn rewrite_tag(
    tag: &str,
    image: &impl Fn(&str) -> Option<String>,
    links: &Links,
    out: &mut String,
) {
    let bytes = tag.as_bytes();
    // `<` then the tag name.
    let mut i = 1;
    while i < bytes.len() && !is_space(bytes[i]) && bytes[i] != b'>' && bytes[i] != b'/' {
        i += 1;
    }
    out.push_str(&tag[..i]);
    loop {
        let ws = i;
        while i < bytes.len() && (is_space(bytes[i]) || bytes[i] == b'/') {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len() && !is_space(bytes[i]) && !matches!(bytes[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        if i == name_start {
            // `>`, or the end of an unclosed tag.
            out.push_str(&tag[ws..]);
            return;
        }
        let name = &tag[name_start..i];
        let mut j = i;
        while j < bytes.len() && is_space(bytes[j]) {
            j += 1;
        }
        let mut value = None;
        if j < bytes.len() && bytes[j] == b'=' {
            j += 1;
            while j < bytes.len() && is_space(bytes[j]) {
                j += 1;
            }
            let (v, next) = match bytes.get(j) {
                Some(&q @ (b'"' | b'\'')) => {
                    let close = tag[j + 1..]
                        .find(q as char)
                        .map_or(tag.len(), |k| j + 1 + k);
                    (&tag[(j + 1).min(close)..close], (close + 1).min(tag.len()))
                }
                _ => {
                    let mut k = j;
                    while k < bytes.len() && !is_space(bytes[k]) && bytes[k] != b'>' {
                        k += 1;
                    }
                    (&tag[j..k], k)
                }
            };
            value = Some(v);
            i = next;
        }
        let lower = name.to_ascii_lowercase();
        let replaced = match (lower.as_str(), value) {
            ("src", Some(v)) => Some(image(&decode(v))),
            ("href", Some(v)) => Some(
                links
                    .register(&decode(v))
                    .map(|key| format!("{LINK_PREFIX}{key}")),
            ),
            ("srcset", _) => Some(None),
            _ => None,
        };
        match replaced {
            // Kept as written.
            None => out.push_str(&tag[ws..i]),
            Some(None) => {}
            Some(Some(new)) => {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(&escape(&new));
                out.push('"');
            }
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}

/// The entities that show up in GameBanana's attribute values.
fn decode(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut rest = v;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&s| s <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let ch = match entity {
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str) -> (String, Links) {
        let links = Links::default();
        let out = rewrite(
            html,
            |url| {
                url.starts_with("https://images.gamebanana.com/")
                    .then(|| "img:ok".to_owned())
            },
            &links,
        );
        (out, links)
    }

    #[test]
    fn images_become_keys_and_others_are_dropped() {
        let (out, _) = run(
            r#"<p>Hi</p><img src="https://images.gamebanana.com/img/a.png" alt="a"><IMG SRC='https://evil.example/x.png' width=10><img src=https://images.gamebanana.com/b.png>"#,
        );
        assert_eq!(
            out,
            r#"<p>Hi</p><img src="img:ok" alt="a"><IMG width=10><img src="img:ok">"#
        );
    }

    #[test]
    fn links_become_keys_and_scripts_lose_their_href() {
        let (out, links) = run(
            r#"<a href="https://ko-fi.com/x?a=1&amp;b=2" target="_blank">tip</a> <a href="javascript:alert(1)">x</a> <a href = '/relative'>y</a>"#,
        );
        let key = out
            .split(LINK_PREFIX)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .unwrap();
        assert_eq!(
            links.get(key).as_deref(),
            Some("https://ko-fi.com/x?a=1&b=2")
        );
        assert!(
            out.contains(r#" target="_blank">tip</a> <a>x</a> <a>y</a>"#),
            "{out}"
        );
        assert!(!out.contains("javascript"));
    }

    #[test]
    fn srcset_and_entity_tricks() {
        let (out, _) = run(
            r#"<img srcset="https://evil.example/1x.png 1x" src="&#x6A;avascript:1"><a href="&#106;avascript:alert(1)">z</a>"#,
        );
        assert_eq!(out, "<img><a>z</a>");
    }

    #[test]
    fn text_comments_and_broken_tags_survive() {
        let (out, _) = run("a < b and <!-- <img src=\"https://evil.example\"> --> <b>bold");
        assert_eq!(
            out,
            "a < b and <!-- <img src=\"https://evil.example\"> --> <b>bold"
        );
        let (out, _) = run(r#"<img src="https://evil.example/x.png"#);
        assert_eq!(out, "<img");
        let (out, _) = run("<br/><hr /><p class=\"x\">ok</p>");
        assert_eq!(out, "<br/><hr /><p class=\"x\">ok</p>");
    }

    #[test]
    fn rewritten_values_are_escaped() {
        let links = Links::default();
        let out = rewrite(
            r#"<img src="https://images.gamebanana.com/a.png">"#,
            |_| Some("x\"onerror=\"alert(1)".to_owned()),
            &links,
        );
        assert_eq!(out, r#"<img src="x&quot;onerror=&quot;alert(1)">"#);
    }
}
