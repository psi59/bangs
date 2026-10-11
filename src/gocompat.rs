//! Byte-exact ports of the Go standard library behaviour the original
//! server relied on (net/url, net/http redirects, path.Clean, unicode
//! decoding). Request data is handled as raw bytes because Go strings may
//! carry invalid UTF-8 and every step below preserves that.

use std::net::{Ipv4Addr, Ipv6Addr};

// ---------------------------------------------------------------------------
// UTF-8 (unicode/utf8)

/// utf8.DecodeRune: invalid input yields `None` with width 1.
pub fn decode_rune(s: &[u8]) -> (Option<char>, usize) {
    let Some(&b0) = s.first() else {
        return (None, 0);
    };
    if b0 < 0x80 {
        return (Some(b0 as char), 1);
    }
    let (need, min, init) = match b0 {
        0xC2..=0xDF => (1, 0x80, u32::from(b0 & 0x1F)),
        0xE0..=0xEF => (2, 0x800, u32::from(b0 & 0x0F)),
        0xF0..=0xF4 => (3, 0x10000, u32::from(b0 & 0x07)),
        _ => return (None, 1),
    };
    if s.len() <= need {
        return (None, 1);
    }
    let mut cp = init;
    for &c in &s[1..=need] {
        if c & 0xC0 != 0x80 {
            return (None, 1);
        }
        cp = (cp << 6) | u32::from(c & 0x3F);
    }
    if cp < min || (0xD800..=0xDFFF).contains(&cp) {
        return (None, 1);
    }
    match char::from_u32(cp) {
        Some(c) => (Some(c), need + 1),
        None => (None, 1),
    }
}

/// utf8.DecodeLastRune.
fn decode_last_rune(s: &[u8]) -> (Option<char>, usize) {
    let end = s.len();
    if end == 0 {
        return (None, 0);
    }
    if s[end - 1] < 0x80 {
        return (Some(s[end - 1] as char), 1);
    }
    let lim = end.saturating_sub(4);
    let mut start = end - 1;
    while start > lim {
        start -= 1;
        if s[start] & 0xC0 != 0x80 {
            break;
        }
    }
    let (r, size) = decode_rune(&s[start..end]);
    if start + size != end {
        return (None, 1);
    }
    (r, size)
}

/// Go's string(bytes) iteration semantics: every invalid byte becomes U+FFFD.
pub fn lossy_utf8(s: &[u8]) -> String {
    match std::str::from_utf8(s) {
        Ok(v) => v.to_owned(),
        Err(_) => {
            let mut out = String::with_capacity(s.len() + 8);
            let mut i = 0;
            while i < s.len() {
                let (r, w) = decode_rune(&s[i..]);
                out.push(r.unwrap_or('\u{FFFD}'));
                i += w;
            }
            out
        }
    }
}

fn is_space(r: Option<char>) -> bool {
    // char::is_whitespace is the Unicode White_Space property, which is
    // exactly the set unicode.IsSpace accepts.
    r.is_some_and(char::is_whitespace)
}

/// strings.TrimSpace.
pub fn trim_space(s: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < s.len() {
        let (r, w) = decode_rune(&s[start..]);
        if !is_space(r) {
            break;
        }
        start += w;
    }
    let mut end = s.len();
    while end > start {
        let (r, w) = decode_last_rune(&s[start..end]);
        if !is_space(r) {
            break;
        }
        end -= w;
    }
    &s[start..end]
}

// ---------------------------------------------------------------------------
// net/url encoding

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Path,
    PathSegment,
    Host,
    Zone,
    UserPassword,
    QueryComponent,
    Fragment,
}

pub fn should_escape(c: u8, mode: Mode) -> bool {
    if c.is_ascii_alphanumeric() {
        return false;
    }
    if matches!(mode, Mode::Host | Mode::Zone)
        && matches!(
            c,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'['
                | b']'
                | b'<'
                | b'>'
                | b'"'
        )
    {
        return false;
    }
    match c {
        b'-' | b'_' | b'.' | b'~' => return false,
        b'$' | b'&' | b'+' | b',' | b'/' | b':' | b';' | b'=' | b'?' | b'@' => match mode {
            Mode::Path => return c == b'?',
            Mode::PathSegment => return matches!(c, b'/' | b';' | b',' | b'?'),
            Mode::UserPassword => return matches!(c, b'@' | b'/' | b'?' | b':'),
            Mode::QueryComponent => return true,
            Mode::Fragment => return false,
            Mode::Host | Mode::Zone => {}
        },
        _ => {}
    }
    if mode == Mode::Fragment && matches!(c, b'!' | b'(' | b')' | b'*') {
        return false;
    }
    true
}

const UPPER_HEX: &[u8; 16] = b"0123456789ABCDEF";

/// url.escape.
pub fn escape(s: &[u8], mode: Mode) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 16);
    for &c in s {
        if c == b' ' && mode == Mode::QueryComponent {
            out.push(b'+');
        } else if should_escape(c, mode) {
            out.extend_from_slice(&[
                b'%',
                UPPER_HEX[usize::from(c >> 4)],
                UPPER_HEX[usize::from(c & 15)],
            ]);
        } else {
            out.push(c);
        }
    }
    out
}

/// url.QueryEscape.
pub fn query_escape(s: &[u8]) -> Vec<u8> {
    escape(s, Mode::QueryComponent)
}

fn unhex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// url.unescape. Errors carry Go's message text.
pub fn unescape(s: &[u8], mode: Mode) -> Result<Vec<u8>, String> {
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'%' => {
                if i + 2 >= s.len()
                    || !s[i + 1].is_ascii_hexdigit()
                    || !s[i + 2].is_ascii_hexdigit()
                {
                    let bad = &s[i..s.len().min(i + 3)];
                    return Err(format!("invalid URL escape {}", go_quote(bad)));
                }
                if mode == Mode::Host && unhex(s[i + 1]) < 8 && &s[i..i + 3] != b"%25" {
                    return Err(format!("invalid URL escape {}", go_quote(&s[i..i + 3])));
                }
                if mode == Mode::Zone {
                    let v = (unhex(s[i + 1]) << 4) | unhex(s[i + 2]);
                    if &s[i..i + 3] != b"%25" && v != b' ' && should_escape(v, Mode::Host) {
                        return Err(format!("invalid URL escape {}", go_quote(&s[i..i + 3])));
                    }
                }
                i += 3;
            }
            c => {
                if matches!(mode, Mode::Host | Mode::Zone) && c < 0x80 && should_escape(c, mode) {
                    return Err(format!(
                        "invalid character {} in host name",
                        go_quote(&s[i..=i])
                    ));
                }
                i += 1;
            }
        }
    }
    let plus = if mode == Mode::QueryComponent {
        b' '
    } else {
        b'+'
    };
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'%' => {
                out.push((unhex(s[i + 1]) << 4) | unhex(s[i + 2]));
                i += 3;
            }
            b'+' => {
                out.push(plus);
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// url.validEncoded.
fn valid_encoded(s: &[u8], mode: Mode) -> bool {
    s.iter().all(|&c| {
        matches!(
            c,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
                | b'['
                | b']'
                | b'%'
        ) || !should_escape(c, mode)
    })
}

/// URL.EscapedPath for a request whose raw (still escaped) path is `raw`.
/// Returns (Path, EscapedPath).
pub fn request_paths(raw: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let path = unescape(raw, Mode::Path)?;
    let default = escape(&path, Mode::Path);
    // RawPath is only set when it differs from the default encoding; it
    // decodes to Path by construction, so validity is the remaining check.
    if raw != default.as_slice() && valid_encoded(raw, Mode::Path) {
        return Ok((path, raw.to_vec()));
    }
    if path == b"*" {
        return Ok((path, b"*".to_vec()));
    }
    Ok((path, default))
}

/// URL.Query().Get(key) on a raw query string.
pub fn query_get(raw_query: &[u8], key: &[u8]) -> Vec<u8> {
    // Go refuses to parse queries with more than 10000 parameters.
    if raw_query.iter().filter(|&&c| c == b'&').count() + 1 > 10000 {
        return Vec::new();
    }
    for pair in raw_query.split(|&c| c == b'&') {
        if pair.is_empty() || pair.contains(&b';') {
            continue;
        }
        let (k, v) = match pair.iter().position(|&c| c == b'=') {
            Some(i) => (&pair[..i], &pair[i + 1..]),
            None => (pair, &b""[..]),
        };
        let Ok(k) = unescape(k, Mode::QueryComponent) else {
            continue;
        };
        let Ok(v) = unescape(v, Mode::QueryComponent) else {
            continue;
        };
        if k == key {
            return v;
        }
    }
    Vec::new()
}

/// net/http pathUnescape: url.PathUnescape falling back to the input.
pub fn path_unescape(s: &[u8]) -> Vec<u8> {
    unescape(s, Mode::PathSegment).unwrap_or_else(|_| s.to_vec())
}

// ---------------------------------------------------------------------------
// net/url parsing (url.Parse), reduced to the fields this service reads.

#[derive(Debug, Default, Clone)]
pub struct Url {
    pub scheme: String,
    pub host: Vec<u8>,
}

/// url.Parse. The error string matches Go's `*url.Error` text.
pub fn parse_url(raw: &str) -> Result<Url, String> {
    let (u, frag) = match raw.find('#') {
        Some(i) => (&raw[..i], &raw[i + 1..]),
        None => (raw, ""),
    };
    let url = parse_inner(u).map_err(|e| format!("parse {}: {}", go_quote(u.as_bytes()), e))?;
    if !frag.is_empty() {
        unescape(frag.as_bytes(), Mode::Fragment)
            .map_err(|e| format!("parse {}: {}", go_quote(raw.as_bytes()), e))?;
    }
    Ok(url)
}

fn parse_inner(raw: &str) -> Result<Url, String> {
    if raw.bytes().any(|b| b < b' ' || b == 0x7f) {
        return Err("net/url: invalid control character in URL".into());
    }
    let mut url = Url::default();
    if raw == "*" {
        return Ok(url);
    }
    let (scheme, mut rest) = get_scheme(raw)?;
    url.scheme = scheme.to_ascii_lowercase();
    if rest.ends_with('?') && rest.matches('?').count() == 1 {
        rest = &rest[..rest.len() - 1];
    } else if let Some(i) = rest.find('?') {
        rest = &rest[..i];
    }
    if !rest.starts_with('/') {
        if !url.scheme.is_empty() {
            return Ok(url);
        }
        let segment = rest.split('/').next().unwrap_or("");
        if segment.contains(':') {
            return Err("first path segment in URL cannot contain colon".into());
        }
    }
    if (!url.scheme.is_empty() || !rest.starts_with("///")) && rest.starts_with("//") {
        let authority = &rest[2..];
        let (authority, path) = match authority.find('/') {
            Some(i) => (&authority[..i], &authority[i..]),
            None => (authority, ""),
        };
        url.host = parse_authority(&url.scheme, authority)?;
        rest = path;
    }
    unescape(rest.as_bytes(), Mode::Path)?;
    Ok(url)
}

fn get_scheme(raw: &str) -> Result<(&str, &str), String> {
    for (i, c) in raw.bytes().enumerate() {
        match c {
            b'a'..=b'z' | b'A'..=b'Z' => {}
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                if i == 0 {
                    return Ok(("", raw));
                }
            }
            b':' => {
                if i == 0 {
                    return Err("missing protocol scheme".into());
                }
                return Ok((&raw[..i], &raw[i + 1..]));
            }
            _ => return Ok(("", raw)),
        }
    }
    Ok(("", raw))
}

fn parse_authority(scheme: &str, authority: &str) -> Result<Vec<u8>, String> {
    let at = authority.rfind('@');
    let host = parse_host(scheme, &authority[at.map_or(0, |i| i + 1)..])?;
    let Some(at) = at else {
        return Ok(host);
    };
    let userinfo = &authority[..at];
    let valid = userinfo.chars().all(|r| {
        r.is_ascii_alphanumeric()
            || matches!(
                r,
                '-' | '.'
                    | '_'
                    | ':'
                    | '~'
                    | '!'
                    | '$'
                    | '&'
                    | '\''
                    | '('
                    | ')'
                    | '*'
                    | '+'
                    | ','
                    | ';'
                    | '='
                    | '%'
                    | '@'
            )
    });
    if !valid {
        return Err("net/url: invalid userinfo".into());
    }
    match userinfo.split_once(':') {
        None => {
            unescape(userinfo.as_bytes(), Mode::UserPassword)?;
        }
        Some((user, pass)) => {
            unescape(user.as_bytes(), Mode::UserPassword)?;
            unescape(pass.as_bytes(), Mode::UserPassword)?;
        }
    }
    Ok(host)
}

fn valid_optional_port(port: &str) -> bool {
    match port.strip_prefix(':') {
        None => port.is_empty(),
        Some(digits) => digits.bytes().all(|b| b.is_ascii_digit()),
    }
}

fn parse_host(_scheme: &str, host: &str) -> Result<Vec<u8>, String> {
    match host.rfind('[') {
        Some(i) if i > 0 => return Err("invalid IP-literal".into()),
        Some(_) => {
            let Some(close) = host.rfind(']') else {
                return Err("missing ']' in host".into());
            };
            let colon_port = &host[close + 1..];
            if !valid_optional_port(colon_port) {
                return Err(format!(
                    "invalid port {} after host",
                    go_quote(colon_port.as_bytes())
                ));
            }
            let port = unescape(colon_port.as_bytes(), Mode::Host)?;
            let hostname = &host[1..close];
            let unescaped = match hostname.find("%25") {
                Some(z) => {
                    let mut h = unescape(&hostname.as_bytes()[..z], Mode::Host)?;
                    h.extend(unescape(&hostname.as_bytes()[z..], Mode::Zone)?);
                    h
                }
                None => unescape(hostname.as_bytes(), Mode::Host)?,
            };
            match parse_addr(&unescaped) {
                Err(e) => return Err(format!("invalid host: {e}")),
                Ok(true) => return Err("invalid IP-literal".into()),
                Ok(false) => {}
            }
            let mut out = Vec::with_capacity(unescaped.len() + port.len() + 2);
            out.push(b'[');
            out.extend(unescaped);
            out.push(b']');
            out.extend(port);
            return Ok(out);
        }
        None => {
            // go.mod targets go1.25, so urlstrictcolons=0: the port starts
            // at the last colon for every scheme.
            if let Some(i) = host.rfind(':') {
                let colon_port = &host[i..];
                if !valid_optional_port(colon_port) {
                    return Err(format!(
                        "invalid port {} after host",
                        go_quote(colon_port.as_bytes())
                    ));
                }
            }
        }
    }
    unescape(host.as_bytes(), Mode::Host)
}

/// netip.ParseAddr reduced to "valid?" and "is IPv4?".
fn parse_addr(s: &[u8]) -> Result<bool, String> {
    let fail = || format!("ParseAddr({}): unable to parse IP", go_quote(s));
    let text = std::str::from_utf8(s).map_err(|_| fail())?;
    if text.contains(':') {
        let (ip, zone) = match text.split_once('%') {
            Some((ip, zone)) => (ip, Some(zone)),
            None => (text, None),
        };
        if zone == Some("") {
            return Err(format!(
                "ParseAddr({}): zone must be a non-empty string",
                go_quote(s)
            ));
        }
        ip.parse::<Ipv6Addr>().map(|_| false).map_err(|_| fail())
    } else if text.contains('.') {
        text.parse::<Ipv4Addr>().map(|_| true).map_err(|_| fail())
    } else {
        Err(fail())
    }
}

// ---------------------------------------------------------------------------
// path

/// path.Clean.
pub fn path_clean(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        return b".".to_vec();
    }
    let rooted = path[0] == b'/';
    let n = path.len();
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let (mut r, mut dotdot) = (0, 0);
    if rooted {
        out.push(b'/');
        r = 1;
        dotdot = 1;
    }
    while r < n {
        if path[r] == b'/' || (path[r] == b'.' && (r + 1 == n || path[r + 1] == b'/')) {
            // empty or "." element
            r += 1;
        } else if path[r] == b'.' && path[r + 1] == b'.' && (r + 2 == n || path[r + 2] == b'/') {
            r += 2;
            if out.len() > dotdot {
                out.pop();
                while out.len() > dotdot && out[out.len() - 1] != b'/' {
                    out.pop();
                }
            } else if !rooted {
                if !out.is_empty() {
                    out.push(b'/');
                }
                out.extend_from_slice(b"..");
                dotdot = out.len();
            }
        } else {
            if (rooted && out.len() != 1) || (!rooted && !out.is_empty()) {
                out.push(b'/');
            }
            while r < n && path[r] != b'/' {
                out.push(path[r]);
                r += 1;
            }
        }
    }
    if out.is_empty() {
        return b".".to_vec();
    }
    out
}

/// net/http cleanPath.
pub fn clean_path(p: &[u8]) -> Vec<u8> {
    if p.is_empty() {
        return b"/".to_vec();
    }
    let mut owned;
    let p = if p[0] == b'/' {
        p
    } else {
        owned = Vec::with_capacity(p.len() + 1);
        owned.push(b'/');
        owned.extend_from_slice(p);
        &owned[..]
    };
    let mut np = path_clean(p);
    if p[p.len() - 1] == b'/' && np != b"/" {
        np.push(b'/');
    }
    np
}

// ---------------------------------------------------------------------------
// net/http

/// http.Redirect's target rewriting: resolves a scheme- and host-less URL
/// against the request's escaped path. Returns the final URL bytes.
pub fn resolve_redirect(target: &[u8], request_escaped_path: &[u8]) -> Vec<u8> {
    let parsed = std::str::from_utf8(target)
        .ok()
        .and_then(|t| parse_url(t).ok());
    let Some(u) = parsed else {
        return target.to_vec();
    };
    if !u.scheme.is_empty() || !u.host.is_empty() {
        return target.to_vec();
    }
    let oldpath: &[u8] = if request_escaped_path.is_empty() {
        b"/"
    } else {
        request_escaped_path
    };
    let mut url = Vec::new();
    if target.first() != Some(&b'/') {
        let dir_end = oldpath
            .iter()
            .rposition(|&c| c == b'/')
            .map_or(0, |i| i + 1);
        url.extend_from_slice(&oldpath[..dir_end]);
    }
    url.extend_from_slice(target);
    let (path, query) = match url.iter().position(|&c| c == b'?') {
        Some(i) => (url[..i].to_vec(), url[i..].to_vec()),
        None => (url, Vec::new()),
    };
    let trailing = path.last() == Some(&b'/');
    let mut cleaned = path_clean(&path);
    if trailing && cleaned.last() != Some(&b'/') {
        cleaned.push(b'/');
    }
    cleaned.extend(query);
    cleaned
}

/// net/http hexEscapeNonASCII (lowercase hex, as strconv.AppendInt emits).
pub fn hex_escape_non_ascii(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for &c in s {
        if c >= 0x80 {
            out.push(b'%');
            out.extend_from_slice(format!("{c:x}").as_bytes());
        } else {
            out.push(c);
        }
    }
    out
}

/// net/http htmlEscape.
pub fn html_escape(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 16);
    for &c in s {
        match c {
            b'&' => out.extend_from_slice(b"&amp;"),
            b'<' => out.extend_from_slice(b"&lt;"),
            b'>' => out.extend_from_slice(b"&gt;"),
            b'"' => out.extend_from_slice(b"&#34;"),
            b'\'' => out.extend_from_slice(b"&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// httpguts.ValidHostHeader.
pub fn valid_host_header(h: &[u8]) -> bool {
    h.iter().all(|&c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                b'!' | b'$'
                    | b'%'
                    | b'&'
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b'-'
                    | b'.'
                    | b':'
                    | b';'
                    | b'='
                    | b'['
                    | b'\''
                    | b']'
                    | b'_'
                    | b'~'
            )
    })
}

// ---------------------------------------------------------------------------
// strconv

/// strconv.Quote (printability approximated by "not a control character").
pub fn go_quote(s: &[u8]) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut i = 0;
    while i < s.len() {
        let (r, w) = decode_rune(&s[i..]);
        match r {
            None => out.push_str(&format!("\\x{:02x}", s[i])),
            Some('"') => out.push_str("\\\""),
            Some('\\') => out.push_str("\\\\"),
            Some('\u{7}') => out.push_str("\\a"),
            Some('\u{8}') => out.push_str("\\b"),
            Some('\u{c}') => out.push_str("\\f"),
            Some('\n') => out.push_str("\\n"),
            Some('\r') => out.push_str("\\r"),
            Some('\t') => out.push_str("\\t"),
            Some('\u{b}') => out.push_str("\\v"),
            Some(c) if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            Some(c) if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            Some(c) => out.push(c),
        }
        i += w;
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_escape_matches_go() {
        assert_eq!(query_escape(b"hello & goodbye"), b"hello+%26+goodbye");
        assert_eq!(
            query_escape("호 a~b-c_d.e".as_bytes()),
            b"%ED%98%B8+a~b-c_d.e"
        );
        assert_eq!(query_escape(b"a/b?c=d"), b"a%2Fb%3Fc%3Dd");
    }

    #[test]
    fn query_get_follows_go_parse_rules() {
        assert_eq!(query_get(b"q=g+hello", b"q"), b"g hello");
        assert_eq!(query_get(b"x=1&q=a&q=b", b"q"), b"a");
        assert_eq!(query_get(b"%71=hi", b"q"), b"hi");
        // A semicolon drops the whole pair, a bad escape skips it.
        assert_eq!(query_get(b"q=a;b", b"q"), b"");
        assert_eq!(query_get(b"q=%zz&q=ok", b"q"), b"ok");
        assert_eq!(query_get(b"q", b"q"), b"");
        assert_eq!(query_get(b"q=%FF", b"q"), vec![0xFF]);
    }

    #[test]
    fn trim_space_handles_unicode_and_invalid_bytes() {
        assert_eq!(trim_space(" \t a b \n".as_bytes()), b"a b");
        assert_eq!(trim_space("\u{3000}x\u{a0}".as_bytes()), b"x");
        assert_eq!(trim_space(b"\xff x \xff"), b"\xff x \xff");
        assert_eq!(trim_space(b"   "), b"");
    }

    #[test]
    fn parse_url_validation() {
        let u = parse_url("https://www.google.com/search?q={{{s}}}").unwrap();
        assert_eq!(u.scheme, "https");
        assert_eq!(u.host, b"www.google.com");
        let u = parse_url("HTTP://Example.COM:8080/x").unwrap();
        assert_eq!(u.scheme, "http");
        assert_eq!(u.host, b"Example.COM:8080");
        assert!(parse_url("not-a-url").unwrap().host.is_empty());
        assert_eq!(
            parse_url("://x").unwrap_err(),
            r#"parse "://x": missing protocol scheme"#
        );
        assert!(parse_url("http://a b/").is_err());
        assert!(parse_url("http://x/%zz").is_err());
        assert!(parse_url("http://[::1]:80/").is_ok());
        assert!(parse_url("http://[1.2.3.4]/").is_err());
        assert!(parse_url("http://x:port/").is_err());
        assert!(parse_url("a:b/c").unwrap().host.is_empty());
        assert!(parse_url("a/b:c").is_ok());
        assert!(parse_url("ab:c").is_ok());
        assert!(parse_url("1a:c").is_err());
    }

    #[test]
    fn clean_path_matches_net_http() {
        assert_eq!(clean_path(b""), b"/");
        assert_eq!(clean_path(b"/a/../b/"), b"/b/");
        assert_eq!(clean_path(b"//search"), b"/search");
        assert_eq!(clean_path(b"/./search"), b"/search");
        assert_eq!(clean_path(b"/.."), b"/");
    }

    #[test]
    fn request_paths_keep_valid_raw_escapes() {
        let (p, e) = request_paths(b"/%73earch").unwrap();
        assert_eq!(p, b"/search");
        assert_eq!(e, b"/%73earch");
        let (p, e) = request_paths(b"/a b").unwrap();
        assert_eq!(p, b"/a b");
        assert_eq!(e, b"/a%20b");
        assert!(request_paths(b"/%zz").is_err());
    }

    #[test]
    fn resolve_redirect_relative() {
        assert_eq!(
            resolve_redirect(b"https://x.com/a", b"/search"),
            b"https://x.com/a"
        );
        assert_eq!(resolve_redirect(b"home", b"/search"), b"/home");
        assert_eq!(resolve_redirect(b"/a/../b/?x=1", b"/search"), b"/b/?x=1");
        assert_eq!(resolve_redirect(b"", b"/search"), b"/");
    }

    #[test]
    fn hex_escape_is_lowercase() {
        assert_eq!(hex_escape_non_ascii("a호".as_bytes()), b"a%ed%98%b8");
    }
}
