use std::sync::LazyLock;
use std::time::Duration;

use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::Full;
use hyper::header::{CONTENT_TYPE, HeaderValue, LOCATION};
use hyper::{Method, Response, StatusCode};

use crate::bang::{Repository, parse_query, resolve_bang};
use crate::gocompat::{hex_escape_non_ascii, html_escape, resolve_redirect};
use crate::gojson::{self, Value};
use crate::pages;

pub type Resp = Response<Full<Bytes>>;

const SUGGEST_MAX_BODY_SIZE: usize = 1 << 20; // 1MB
pub const SUGGEST_TIMEOUT: Duration = Duration::from_secs(2);

// Wikimedia 등은 기본 클라이언트 UA를 로봇 정책 위반으로 거부한다.
// 실사용자의 Firefox 입력을 대리하는 요청이므로 최신 Firefox UA를 사용
pub const SUGGEST_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:152.0) Gecko/20100101 Firefox/152.0";

pub const TEXT_HTML: &str = "text/html; charset=utf-8";

pub fn response(
    status: StatusCode,
    content_type: Option<&'static str>,
    body: impl Into<Bytes>,
) -> Resp {
    let mut resp = Response::new(Full::new(body.into()));
    *resp.status_mut() = status;
    if let Some(ct) = content_type {
        resp.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(ct));
    }
    resp
}

/// Request facts the handlers need, extracted once by the server.
pub struct Ctx<'a> {
    pub method: &'a Method,
    pub escaped_path: &'a [u8],
    pub raw_query: &'a [u8],
}

// ---------------------------------------------------------------------------
// Error pages

static PAGE_NO_CONFIG: LazyLock<Bytes> = LazyLock::new(|| {
    pages::error_page(
        503,
        "🔧",
        "Service Unavailable",
        "Failed to load configuration file. 😢<br>Please contact the administrator or check the config file.",
    )
    .into()
});

static PAGE_NO_QUERY: LazyLock<Bytes> = LazyLock::new(|| {
    pages::error_page(
        400,
        "🤔",
        "Bad Request",
        "Search query is required! 🔍<br>Please provide a search term in the 'q' parameter.",
    )
    .into()
});

static PAGE_NO_DEFAULT: LazyLock<Bytes> = LazyLock::new(|| {
    pages::error_page(
        503,
        "⚙️",
        "Service Unavailable",
        "No default search engine configured. 😅<br>Please check 'default_bang' in your config file.",
    )
    .into()
});

// ---------------------------------------------------------------------------
// Redirects (http.Redirect)

/// Header-safe form of a value the way Go's header writer emits it:
/// CR/LF become spaces and surrounding whitespace is trimmed. Other
/// control bytes, which hyper refuses, are percent-encoded.
fn header_value(v: &[u8]) -> HeaderValue {
    let replaced: Vec<u8> = v
        .iter()
        .map(|&c| if c == b'\r' || c == b'\n' { b' ' } else { c })
        .collect();
    let start = replaced
        .iter()
        .position(|c| !matches!(c, b' ' | b'\t'))
        .unwrap_or(replaced.len());
    let end = replaced
        .iter()
        .rposition(|c| !matches!(c, b' ' | b'\t'))
        .map_or(start, |i| i + 1);
    let trimmed = &replaced[start..end];
    HeaderValue::from_bytes(trimmed).unwrap_or_else(|_| {
        let mut out = Vec::with_capacity(trimmed.len() + 8);
        for &c in trimmed {
            if (c < 0x20 && c != b'\t') || c == 0x7f {
                out.extend_from_slice(format!("%{c:02X}").as_bytes());
            } else {
                out.push(c);
            }
        }
        HeaderValue::from_bytes(&out).unwrap_or_else(|_| HeaderValue::from_static(""))
    })
}

pub fn redirect(ctx: &Ctx, target: &[u8], status: StatusCode) -> Resp {
    let url = resolve_redirect(target, ctx.escaped_path);
    let is_get = ctx.method == Method::GET;
    let body = if is_get {
        let mut b = Vec::with_capacity(url.len() + 48);
        b.extend_from_slice(b"<a href=\"");
        b.extend(html_escape(&url));
        b.extend_from_slice(b"\">");
        b.extend_from_slice(status.canonical_reason().unwrap_or("").as_bytes());
        b.extend_from_slice(b"</a>.\n\n");
        b
    } else {
        Vec::new()
    };
    let ct = (is_get || ctx.method == Method::HEAD).then_some(TEXT_HTML);
    let mut resp = response(status, ct, body);
    resp.headers_mut()
        .insert(LOCATION, header_value(&hex_escape_non_ascii(&url)));
    resp
}

// ---------------------------------------------------------------------------
// /search

pub fn search(ctx: &Ctx, repo: Option<&Repository>) -> Resp {
    let Some(repo) = repo else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            Some(TEXT_HTML),
            PAGE_NO_CONFIG.clone(),
        );
    };
    let query = crate::gocompat::query_get(ctx.raw_query, b"q");
    if query.is_empty() {
        return response(
            StatusCode::BAD_REQUEST,
            Some(TEXT_HTML),
            PAGE_NO_QUERY.clone(),
        );
    }
    let parsed = parse_query(&query);
    let resolved = resolve_bang(repo, &query, &parsed);
    let Some(bang) = resolved.bang else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            Some(TEXT_HTML),
            PAGE_NO_DEFAULT.clone(),
        );
    };
    redirect(
        ctx,
        &bang.build_url(resolved.search_term),
        StatusCode::FOUND,
    )
}

// ---------------------------------------------------------------------------
// /suggest

pub async fn suggest(ctx: &Ctx<'_>, repo: Option<&Repository>, client: &reqwest::Client) -> Resp {
    let query = crate::gocompat::query_get(ctx.raw_query, b"q");
    let suggestions = fetch_suggestions(repo, &query, client).await;
    let mut body = Vec::with_capacity(64 + query.len());
    body.push(b'[');
    gojson::write_string(&mut body, &query);
    body.extend_from_slice(b",[");
    for (i, s) in suggestions.iter().enumerate() {
        if i > 0 {
            body.push(b',');
        }
        gojson::write_string(&mut body, s);
    }
    body.extend_from_slice(b"]]\n");
    response(StatusCode::OK, Some("application/x-suggestions+json"), body)
}

async fn fetch_suggestions(
    repo: Option<&Repository>,
    query: &[u8],
    client: &reqwest::Client,
) -> Vec<Vec<u8>> {
    let Some(repo) = repo else { return Vec::new() };
    if query.is_empty() {
        return Vec::new();
    }
    let parsed = parse_query(query);
    let resolved = resolve_bang(repo, query, &parsed);
    let Some(bang) = resolved.bang else {
        return Vec::new();
    };
    if bang.suggest_url_template.is_empty() || resolved.search_term.is_empty() {
        return Vec::new();
    }
    let url = bang.build_suggest_url(resolved.search_term);
    let upstream = fetch_upstream(client, &url).await;
    if !resolved.via_trigger {
        return upstream.into_iter().map(String::into_bytes).collect();
    }
    upstream
        .into_iter()
        .map(|s| {
            let mut v = Vec::with_capacity(parsed.first_word.len() + 1 + s.len());
            v.extend_from_slice(parsed.first_word);
            v.push(b' ');
            v.extend_from_slice(s.as_bytes());
            v
        })
        .collect()
}

async fn fetch_upstream(client: &reqwest::Client, url: &[u8]) -> Vec<String> {
    let Ok(url) = std::str::from_utf8(url) else {
        return Vec::new();
    };
    let Ok(resp) = client.get(url).send().await else {
        return Vec::new();
    };
    if resp.status() != reqwest::StatusCode::OK {
        return Vec::new();
    }
    let mut body = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { return Vec::new() };
        let room = SUGGEST_MAX_BODY_SIZE - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if body.len() == SUGGEST_MAX_BODY_SIZE {
            break;
        }
    }
    parse_suggestions(&body).unwrap_or_default()
}

/// Extracts the completion list from an OpenSearch suggestions response
/// (`[query, [completions], ...]`) or, failing that, a Naver autocomplete
/// response (`{"items": [[["completion"], ...]]}`). `None` means Go's
/// decoder would have reported an error (or nil).
pub fn parse_suggestions(body: &[u8]) -> Option<Vec<String>> {
    match gojson::parse(body) {
        Some(Value::Array(elements)) => {
            if elements.len() < 2 {
                return None;
            }
            string_slice(&elements[1])
        }
        Some(Value::Null) => None,
        _ => parse_naver_suggestions(body),
    }
}

/// Decodes a []string the way encoding/json does: null is a nil slice,
/// null elements are "", anything else is a type error.
fn string_slice(v: &Value) -> Option<Vec<String>> {
    match v {
        Value::Null => None,
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(s) => Some(s.clone()),
                Value::Null => Some(String::new()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// encoding/json's case-insensitive field match for the key "items".
fn is_items_key(k: &str) -> bool {
    let mut folded = k.chars().map(|c| match c {
        'ſ' => 's',
        c => c.to_ascii_lowercase(),
    });
    "items".chars().all(|c| folded.next() == Some(c)) && folded.next().is_none()
}

fn parse_naver_suggestions(body: &[u8]) -> Option<Vec<String>> {
    let fields = match gojson::parse(body)? {
        Value::Object(fields) => fields,
        _ => return None,
    };
    let mut items: Option<&Vec<Value>> = None;
    for (k, v) in &fields {
        if !is_items_key(k) {
            continue;
        }
        match v {
            Value::Null => items = None,
            Value::Array(a) => items = Some(a),
            _ => return None,
        }
    }
    let first = items?.first()?;
    let entries = match first {
        Value::Null => return None,
        Value::Array(entries) => entries,
        _ => return None,
    };
    let mut suggestions = Vec::new();
    for entry in entries {
        let entry = match entry {
            Value::Null => continue,
            other => string_slice(other)?,
        };
        if let Some(s) = entry.first()
            && !s.is_empty()
        {
            suggestions.push(s.clone());
        }
    }
    Some(suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Option<Vec<String>> {
        Some(v.iter().map(|x| x.to_string()).collect())
    }

    #[test]
    fn opensearch_format() {
        assert_eq!(
            parse_suggestions(br#"["hello",["hello world","hello kitty"]]"#),
            s(&["hello world", "hello kitty"])
        );
        assert_eq!(
            parse_suggestions(br#"["a",["x",null],[],[]]"#),
            s(&["x", ""])
        );
        assert_eq!(parse_suggestions(br#"["a"]"#), None);
        assert_eq!(parse_suggestions(br#"["a",null]"#), None);
        assert_eq!(parse_suggestions(br#"["a",["x",1]]"#), None);
        assert_eq!(parse_suggestions(b"null"), None);
        assert_eq!(parse_suggestions(b"not json"), None);
    }

    #[test]
    fn naver_format() {
        let body =
            r#"{"query":["춘천"],"answer":[],"intend":[],"items":[[["춘천 날씨"],["춘천 맛집"]]]}"#;
        assert_eq!(
            parse_suggestions(body.as_bytes()),
            s(&["춘천 날씨", "춘천 맛집"])
        );
        assert_eq!(
            parse_suggestions(br#"{"ITEMS":[[["a"],[""],null,["b","x"]]]}"#),
            s(&["a", "b"])
        );
        assert_eq!(parse_suggestions(br#"{"items":[]}"#), None);
        assert_eq!(parse_suggestions(br#"{"items":"x"}"#), None);
        assert_eq!(parse_suggestions(br#"{"items":[[[1]]]}"#), None);
        assert_eq!(
            parse_suggestions(br#"{"items":[[["a"]]],"items":null}"#),
            None
        );
    }
}
