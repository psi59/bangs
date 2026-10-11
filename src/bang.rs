use std::collections::HashMap;
use std::sync::Arc;

use crate::gocompat::{parse_url, query_escape, trim_space};
use crate::korean::korean_to_qwerty;

const PLACEHOLDER: &[u8] = b"{{{s}}}";

#[derive(Debug, Clone, Default)]
pub struct Bang {
    pub trigger: String,
    #[allow(dead_code)] // part of the config schema; not used for routing
    pub name: String,
    pub url_template: String,
    pub home_url: String,
    pub suggest_url_template: String,
}

impl Bang {
    pub fn new(trigger: &str, name: &str, url_template: &str, home_url: &str) -> Self {
        Self {
            trigger: trigger.into(),
            name: name.into(),
            url_template: url_template.into(),
            home_url: home_url.into(),
            suggest_url_template: String::new(),
        }
    }

    /// Search URL for `query`, or the home URL when the query is empty.
    pub fn build_url(&self, query: &[u8]) -> Vec<u8> {
        if query.is_empty() {
            return self.home_url();
        }
        fill_template(&self.url_template, query)
    }

    /// Autocomplete URL for `query`; empty when no template is configured.
    pub fn build_suggest_url(&self, query: &[u8]) -> Vec<u8> {
        if self.suggest_url_template.is_empty() {
            return Vec::new();
        }
        fill_template(&self.suggest_url_template, query)
    }

    /// The configured home URL, else scheme://host of the URL template.
    pub fn home_url(&self) -> Vec<u8> {
        if !self.home_url.is_empty() {
            return self.home_url.as_bytes().to_vec();
        }
        match parse_url(&self.url_template) {
            Ok(u) => {
                let mut s = u.scheme.into_bytes();
                s.extend_from_slice(b"://");
                s.extend_from_slice(&u.host);
                s
            }
            Err(_) => Vec::new(),
        }
    }
}

/// Replaces the first placeholder with the query-escaped search term.
fn fill_template(template: &str, query: &[u8]) -> Vec<u8> {
    let t = template.as_bytes();
    let encoded = query_escape(query);
    match t.windows(PLACEHOLDER.len()).position(|w| w == PLACEHOLDER) {
        Some(i) => {
            let mut out = Vec::with_capacity(t.len() + encoded.len());
            out.extend_from_slice(&t[..i]);
            out.extend_from_slice(&encoded);
            out.extend_from_slice(&t[i + PLACEHOLDER.len()..]);
            out
        }
        None => t.to_vec(),
    }
}

#[derive(Debug, Default)]
pub struct Repository {
    bangs: HashMap<Vec<u8>, Arc<Bang>>,
    default_bang: Option<Arc<Bang>>,
}

impl Repository {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a bang; a later bang with the same trigger replaces it.
    pub fn add(&mut self, bang: Bang) {
        self.bangs
            .insert(bang.trigger.as_bytes().to_vec(), Arc::new(bang));
    }

    pub fn find_by_trigger(&self, trigger: &[u8]) -> Option<&Arc<Bang>> {
        self.bangs.get(trigger)
    }

    pub fn set_default(&mut self, trigger: &str) {
        self.default_bang = self.bangs.get(trigger.as_bytes()).cloned();
    }

    pub fn default_bang(&self) -> Option<&Arc<Bang>> {
        self.default_bang.as_ref()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedQuery<'a> {
    pub first_word: &'a [u8],
    pub search_term: &'a [u8],
}

/// Splits a query into its first word (the potential trigger) and the rest.
pub fn parse_query(query: &[u8]) -> ParsedQuery<'_> {
    let query = trim_space(query);
    match query.iter().position(|&c| c == b' ' || c == b'\t') {
        None => ParsedQuery {
            first_word: query,
            search_term: b"",
        },
        Some(i) => ParsedQuery {
            first_word: &query[..i],
            search_term: trim_space(&query[i + 1..]),
        },
    }
}

pub struct Resolved<'a> {
    pub bang: Option<&'a Arc<Bang>>,
    pub search_term: &'a [u8],
    pub via_trigger: bool,
}

/// Finds the bang for a parsed query: trigger lookup first, then
/// Korean-to-QWERTY conversion, then the default bang with the full query
/// as search term. `via_trigger` reports whether the first word matched.
pub fn resolve_bang<'a>(
    repo: &'a Repository,
    query: &'a [u8],
    parsed: &ParsedQuery<'a>,
) -> Resolved<'a> {
    let mut bang = repo.find_by_trigger(parsed.first_word);
    if bang.is_none() {
        let converted = korean_to_qwerty(parsed.first_word);
        if converted != parsed.first_word {
            bang = repo.find_by_trigger(&converted);
        }
    }
    match bang {
        Some(b) => Resolved {
            bang: Some(b),
            search_term: parsed.search_term,
            via_trigger: true,
        },
        None => Resolved {
            bang: repo.default_bang(),
            search_term: query,
            via_trigger: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn google() -> Bang {
        Bang::new("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
    }

    #[test]
    fn build_url_escapes_query() {
        let b = google();
        assert_eq!(
            b.build_url(b"hello"),
            b"https://www.google.com/search?q=hello"
        );
        assert_eq!(
            b.build_url(b"hello world"),
            b"https://www.google.com/search?q=hello+world"
        );
        assert_eq!(
            b.build_url(b"hello & goodbye"),
            b"https://www.google.com/search?q=hello+%26+goodbye"
        );
    }

    #[test]
    fn empty_query_returns_home_url() {
        let b = Bang::new(
            "yt",
            "YouTube",
            "https://www.youtube.com/results?search_query={{{s}}}",
            "https://www.youtube.com",
        );
        assert_eq!(b.build_url(b""), b"https://www.youtube.com");
        assert_eq!(google().home_url(), b"https://www.google.com");
    }

    #[test]
    fn build_suggest_url() {
        let mut b = google();
        assert!(b.build_suggest_url(b"hello").is_empty());
        b.suggest_url_template =
            "https://suggestqueries.google.com/complete/search?client=firefox&q={{{s}}}".into();
        assert_eq!(
            b.build_suggest_url(b"hello world"),
            b"https://suggestqueries.google.com/complete/search?client=firefox&q=hello+world"
        );
    }

    #[test]
    fn only_first_placeholder_is_replaced() {
        let b = Bang::new("x", "X", "https://x.com/?a={{{s}}}&b={{{s}}}", "");
        assert_eq!(b.build_url(b"q"), b"https://x.com/?a=q&b={{{s}}}");
    }

    #[test]
    fn repository_lookup_and_default() {
        let mut repo = Repository::new();
        assert!(repo.find_by_trigger(b"g").is_none());
        repo.add(google());
        assert_eq!(repo.find_by_trigger(b"g").unwrap().name, "Google");
        assert!(repo.default_bang().is_none());
        repo.set_default("g");
        assert_eq!(repo.default_bang().unwrap().trigger, "g");
    }

    #[test]
    fn parse_query_cases() {
        let p = |s: &'static str| {
            let r = parse_query(s.as_bytes());
            (
                std::str::from_utf8(r.first_word).unwrap(),
                std::str::from_utf8(r.search_term).unwrap(),
            )
        };
        assert_eq!(p("hello"), ("hello", ""));
        assert_eq!(p("hello world"), ("hello", "world"));
        assert_eq!(p("!g hello world"), ("!g", "hello world"));
        assert_eq!(p(""), ("", ""));
        assert_eq!(p("   "), ("", ""));
        assert_eq!(p("  g hello"), ("g", "hello"));
        assert_eq!(p("g   hello   world"), ("g", "hello   world"));
        assert_eq!(p("g\thello"), ("g", "hello"));
    }
}
