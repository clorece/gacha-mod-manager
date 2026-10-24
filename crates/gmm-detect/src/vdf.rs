// SPDX-License-Identifier: MIT OR Apache-2.0

//! Minimal parser for Valve's text KeyValues format (`libraryfolders.vdf`, `appmanifest_*.acf`):
//!
//! ```text
//! "AppState"
//! {
//!     "appid"      "4162040"
//!     "installdir" "Zenless Zone Zero"
//! }
//! ```
//!
//! Quoted or bare tokens, `{` `}` blocks, `//` comments, and `\\` `\"` `\n` `\t` escapes.
//! Untrusted input: depth and size are bounded, and errors never panic.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Block(Vec<(String, Value)>),
}

impl Value {
    /// First child with this key (case-insensitive, like Steam).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Block(items) => items
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Self::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Block(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Self::Block(items) => items,
            Self::Text(_) => &[],
        }
    }
}

const MAX_DEPTH: usize = 32;
const MAX_INPUT: usize = 16 * 1024 * 1024;

/// Parses a whole document into a top-level block.
pub fn parse(input: &str) -> Result<Value, String> {
    if input.len() > MAX_INPUT {
        return Err("file too large".to_owned());
    }
    let mut p = Parser {
        chars: input.char_indices().peekable(),
        input,
    };
    let items = p.block(0, false)?;
    Ok(Value::Block(items))
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    input: &'a str,
}

enum Token {
    Str(String),
    Open,
    Close,
}

impl Parser<'_> {
    fn block(&mut self, depth: usize, nested: bool) -> Result<Vec<(String, Value)>, String> {
        if depth > MAX_DEPTH {
            return Err("nested too deeply".to_owned());
        }
        let mut items = Vec::new();
        loop {
            match self.token()? {
                None if nested => return Err("unexpected end of file inside a block".to_owned()),
                None => return Ok(items),
                Some(Token::Close) if nested => return Ok(items),
                Some(Token::Close) => return Err("unexpected '}'".to_owned()),
                Some(Token::Open) => return Err("block without a key".to_owned()),
                Some(Token::Str(key)) => {
                    let value = match self.token()? {
                        Some(Token::Str(text)) => Value::Text(text),
                        Some(Token::Open) => Value::Block(self.block(depth + 1, true)?),
                        Some(Token::Close) | None => {
                            return Err(format!("key {key:?} has no value"));
                        }
                    };
                    items.push((key, value));
                }
            }
        }
    }

    fn token(&mut self) -> Result<Option<Token>, String> {
        loop {
            let Some(&(i, c)) = self.chars.peek() else {
                return Ok(None);
            };
            match c {
                c if c.is_whitespace() => {
                    self.chars.next();
                }
                '/' if self.input[i..].starts_with("//") => {
                    for (_, c) in self.chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                '{' => {
                    self.chars.next();
                    return Ok(Some(Token::Open));
                }
                '}' => {
                    self.chars.next();
                    return Ok(Some(Token::Close));
                }
                '"' => {
                    self.chars.next();
                    return self.quoted().map(|s| Some(Token::Str(s)));
                }
                _ => {
                    let mut s = String::new();
                    while let Some(&(_, c)) = self.chars.peek() {
                        if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                            break;
                        }
                        s.push(c);
                        self.chars.next();
                    }
                    return Ok(Some(Token::Str(s)));
                }
            }
        }
    }

    fn quoted(&mut self) -> Result<String, String> {
        let mut s = String::new();
        while let Some((_, c)) = self.chars.next() {
            match c {
                '"' => return Ok(s),
                '\\' => match self.chars.next() {
                    Some((_, 'n')) => s.push('\n'),
                    Some((_, 't')) => s.push('\t'),
                    Some((_, other)) => s.push(other),
                    None => break,
                },
                c => s.push(c),
            }
        }
        Err("unterminated string".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn parses_library_folders() {
        let doc = parse(
            r#"
            "libraryfolders"
            {
                "0"
                {
                    "path"      "C:\\Program Files (x86)\\Steam"   // main library
                    "apps" { "228980" "1234" }
                }
                "1" { "path" "D:\\SteamLibrary" }
            }"#,
        )
        .unwrap();
        let libs = doc.get("LibraryFolders").unwrap();
        let paths: Vec<_> = libs
            .entries()
            .iter()
            .filter_map(|(_, v)| v.get("path")?.text())
            .collect();
        assert_eq!(paths, [r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]);
    }

    #[test]
    fn parses_app_manifest() {
        let doc = parse(
            "\"AppState\"\n{\n\t\"appid\"\t\t\"4162040\"\n\t\"installdir\"\t\t\"Zenless Zone Zero\"\n}\n",
        )
        .unwrap();
        let app = doc.get("AppState").unwrap();
        assert_eq!(
            app.get("installdir").unwrap().text(),
            Some("Zenless Zone Zero")
        );
        assert_eq!(app.get("APPID").unwrap().text(), Some("4162040"));
    }

    #[test]
    fn rejects_broken_input() {
        assert!(parse("\"a\" { \"b\" \"c\"").is_err(), "unclosed block");
        assert!(parse("\"a\" \"unterminated").is_err());
        assert!(parse("}").is_err());
        assert!(parse("\"key\"").is_err(), "key without value");
        let deep = "\"k\" {".repeat(100) + &"}".repeat(100);
        assert!(parse(&deep).is_err(), "depth bounded");
    }

    proptest! {
        #[test]
        fn never_panics(input in "[\"{}a-z0-9 \\\\/\\n\\t]{0,200}") {
            let _ = parse(&input);
        }
    }
}
