//! Optional shared-key authentication via the `key` query parameter.
//!
//! Browsers cannot attach headers to address-bar searches or OpenSearch
//! suggestion requests, so the key travels in the URL. It is therefore
//! meant to be a long random value (e.g. `openssl rand -hex 32`) that can be
//! rotated by listing several keys.

use crate::gocompat::{query_escape, query_get};

pub const KEY_PARAM: &[u8] = b"key";

/// Keys shorter than this get a startup warning.
pub const MIN_RECOMMENDED_LEN: usize = 32;

#[derive(Debug, Default, Clone)]
pub struct AuthKeys {
    keys: Vec<Vec<u8>>,
}

pub enum Access {
    /// Authentication is disabled.
    Open,
    /// The request carried this valid key.
    Granted(Vec<u8>),
    Denied,
}

impl AuthKeys {
    /// Parses a comma-separated key list; blank entries are ignored and an
    /// empty list disables authentication.
    pub fn parse(value: &str) -> Self {
        let keys = value
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(|k| k.as_bytes().to_vec())
            .collect();
        Self { keys }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn has_short_key(&self) -> bool {
        self.keys.iter().any(|k| k.len() < MIN_RECOMMENDED_LEN)
    }

    pub fn check(&self, raw_query: &[u8]) -> Access {
        if self.keys.is_empty() {
            return Access::Open;
        }
        let provided = query_get(raw_query, KEY_PARAM);
        // Check every key without short-circuiting so timing does not
        // reveal which key (or how much of it) matched.
        let valid = self
            .keys
            .iter()
            .fold(false, |acc, k| acc | constant_time_eq(&provided, k));
        if valid && !provided.is_empty() {
            Access::Granted(provided)
        } else {
            Access::Denied
        }
    }
}

/// Byte comparison whose running time depends only on the lengths.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `key=<escaped>` for embedding the key in generated URLs.
pub fn key_query(key: &[u8]) -> Vec<u8> {
    let mut out = KEY_PARAM.to_vec();
    out.push(b'=');
    out.extend(query_escape(key));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn granted(a: Access) -> Option<Vec<u8>> {
        match a {
            Access::Granted(k) => Some(k),
            _ => None,
        }
    }

    #[test]
    fn empty_list_disables_auth() {
        assert!(matches!(AuthKeys::parse("").check(b"q=x"), Access::Open));
        assert!(matches!(AuthKeys::parse(" , ").check(b"q=x"), Access::Open));
    }

    #[test]
    fn accepts_any_listed_key() {
        let auth = AuthKeys::parse("alpha, beta ,");
        assert_eq!(auth.len(), 2);
        assert_eq!(
            granted(auth.check(b"q=x&key=alpha")),
            Some(b"alpha".to_vec())
        );
        assert_eq!(granted(auth.check(b"key=beta&q=x")), Some(b"beta".to_vec()));
    }

    #[test]
    fn rejects_missing_wrong_or_partial_keys() {
        let auth = AuthKeys::parse("alpha");
        for q in [
            &b"q=x"[..],
            b"key=",
            b"key=alph",
            b"key=alphaa",
            b"key=ALPHA",
            b"q=key%3Dalpha",
        ] {
            assert!(matches!(auth.check(q), Access::Denied), "{q:?}");
        }
    }

    #[test]
    fn key_is_url_decoded() {
        let auth = AuthKeys::parse("a+b/c");
        assert!(granted(auth.check(b"key=a%2Bb%2Fc")).is_some());
        assert_eq!(key_query(b"a+b/c"), b"key=a%2Bb%2Fc");
    }

    #[test]
    fn short_key_detection() {
        assert!(AuthKeys::parse("short").has_short_key());
        assert!(!AuthKeys::parse(&"x".repeat(MIN_RECOMMENDED_LEN)).has_short_key());
    }
}
