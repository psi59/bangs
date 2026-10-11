//! Request-level tests ported from the Go handler, suggest, server and
//! opensearch tests.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Request, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::handler::Resp;
use crate::server::Server;

struct TestResp {
    status: StatusCode,
    headers: hyper::HeaderMap,
    body: Bytes,
}

impl TestResp {
    fn header(&self, name: &str) -> &str {
        self.headers.get(name).map_or("", |v| v.to_str().unwrap())
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

async fn collect(resp: Resp) -> TestResp {
    let (parts, body) = resp.into_parts();
    TestResp {
        status: parts.status,
        headers: parts.headers,
        body: body.collect().await.unwrap().to_bytes(),
    }
}

fn write_config(yaml: &str) -> String {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("bangs-test-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bangs.yaml");
    std::fs::write(&path, yaml).unwrap();
    path.to_string_lossy().into_owned()
}

fn server_with(yaml: &str) -> Arc<Server> {
    Server::new(&write_config(yaml))
}

async fn get(server: &Server, uri: &str) -> TestResp {
    get_with(server, Request::get(uri).header("host", "localhost")).await
}

async fn get_with(server: &Server, builder: hyper::http::request::Builder) -> TestResp {
    let (parts, ()) = builder.body(()).unwrap().into_parts();
    collect(server.handle_parts(&parts).await).await
}

const GOOGLE: &str = r#"
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
"#;

fn single_bang(trigger: &str, url: &str, default: bool) -> String {
    let default = if default {
        format!("default_bang: \"{trigger}\"\n")
    } else {
        String::new()
    };
    format!(
        "{default}bangs:\n  - trigger: \"{trigger}\"\n    name: X\n    url_template: \"{url}\"\n"
    )
}

// ---------------------------------------------------------------------------
// /search

#[tokio::test]
async fn search_bang_redirects() {
    let s = server_with(&single_bang(
        "g",
        "https://www.google.com/search?q={{{s}}}",
        false,
    ));
    let r = get(&s, "/search?q=g+hello").await;
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=hello"
    );
}

#[tokio::test]
async fn search_exclamation_trigger_redirects() {
    let s = server_with(&single_bang(
        "!g",
        "https://www.google.com/search?q={{{s}}}",
        false,
    ));
    let r = get(&s, "/search?q=!g+hello").await;
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=hello"
    );
}

#[tokio::test]
async fn search_without_bang_uses_default() {
    let s = server_with(GOOGLE);
    let r = get(&s, "/search?q=hello+world").await;
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=hello+world"
    );
    let r = get(&s, "/search?q=nonexistent+hello").await;
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=nonexistent+hello"
    );
}

#[tokio::test]
async fn search_empty_or_missing_query_is_bad_request() {
    let s = server_with(GOOGLE);
    assert_eq!(get(&s, "/search?q=").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(get(&s, "/search").await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn search_without_config_is_unavailable_html() {
    let s = Server::new("/nonexistent/bangs.yaml");
    let r = get(&s, "/search?q=hello").await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(r.header("content-type").contains("text/html"));
    assert!(r.text().contains("<html"));
    assert!(r.text().contains("503"));
}

#[tokio::test]
async fn search_without_default_bang_is_unavailable() {
    let s = server_with(&single_bang(
        "g",
        "https://www.google.com/search?q={{{s}}}",
        false,
    ));
    let r = get(&s, "/search?q=hello").await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(r.header("content-type").contains("text/html"));
}

#[tokio::test]
async fn search_korean_trigger() {
    let s = server_with(&single_bang(
        "gh",
        "https://github.com/search?q={{{s}}}",
        true,
    ));
    let r = get(&s, "/search?q=%ED%98%B8").await; // "호"
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(r.header("location"), "https://github.com");
    let r = get(&s, "/search?q=%ED%98%B8+hello").await;
    assert_eq!(r.header("location"), "https://github.com/search?q=hello");
}

#[tokio::test]
async fn search_korean_without_match_uses_default() {
    let s = server_with(GOOGLE);
    // "안녕" → "dkssud" (존재하지 않는 트리거)
    let r = get(&s, "/search?q=%EC%95%88%EB%85%95+hello").await;
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=%EC%95%88%EB%85%95+hello"
    );
}

#[tokio::test]
async fn search_trigger_only_redirects_home() {
    let s = server_with(&single_bang(
        "yt",
        "https://www.youtube.com/results?search_query={{{s}}}",
        true,
    ));
    let r = get(&s, "/search?q=yt").await;
    assert_eq!(r.header("location"), "https://www.youtube.com");
}

#[tokio::test]
async fn redirect_body_and_method_handling_match_net_http() {
    let s = server_with(GOOGLE);
    let r = get(&s, "/search?q=g+a%26b").await;
    assert_eq!(r.header("content-type"), "text/html; charset=utf-8");
    assert_eq!(
        r.text(),
        "<a href=\"https://www.google.com/search?q=a%26b\">Found</a>.\n\n"
    );
    let r = get_with(
        &s,
        Request::post("/search?q=g+x").header("host", "localhost"),
    )
    .await;
    assert_eq!(r.status, StatusCode::FOUND);
    assert_eq!(r.header("content-type"), "");
    assert!(r.body.is_empty());
}

// ---------------------------------------------------------------------------
// /suggest

#[derive(Default)]
struct Seen {
    request_lines: Vec<String>,
    user_agents: Vec<String>,
}

/// A one-response-per-connection upstream that records what it receives.
async fn upstream(status: u16, body: &'static str) -> (String, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let recorder = Arc::clone(&seen);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let recorder = Arc::clone(&recorder);
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).into_owned();
                {
                    let mut seen = recorder.lock().unwrap();
                    seen.request_lines
                        .push(head.lines().next().unwrap_or("").to_owned());
                    if let Some(ua) = head.lines().find_map(|l| {
                        l.split_once(':')
                            .filter(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
                            .map(|(_, v)| v.trim())
                    }) {
                        seen.user_agents.push(ua.to_owned());
                    }
                }
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
    (format!("http://{addr}"), seen)
}

fn suggest_config(trigger: &str, suggest_url: &str, default: bool) -> String {
    let default = if default {
        format!("default_bang: {trigger}\n")
    } else {
        String::new()
    };
    format!(
        "{default}bangs:\n  - trigger: {trigger}\n    name: X\n    url_template: \"https://www.google.com/search?q={{{{{{s}}}}}}\"\n    suggest_url_template: \"{suggest_url}\"\n"
    )
}

fn assert_suggest(r: &TestResp, query: &str, suggestions: &[&str]) {
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), "application/x-suggestions+json");
    let mut expected = Vec::new();
    expected.push(b'[');
    crate::gojson::write_string(&mut expected, query.as_bytes());
    expected.extend_from_slice(b",[");
    for (i, s) in suggestions.iter().enumerate() {
        if i > 0 {
            expected.push(b',');
        }
        crate::gojson::write_string(&mut expected, s.as_bytes());
    }
    expected.extend_from_slice(b"]]\n");
    assert_eq!(r.text(), String::from_utf8(expected).unwrap());
}

#[tokio::test]
async fn suggest_proxies_and_prefixes_trigger() {
    let (url, seen) = upstream(200, r#"["hello",["hello world","hello kitty"]]"#).await;
    let s = server_with(&suggest_config(
        "g",
        &format!("{url}/complete?q={{{{{{s}}}}}}"),
        false,
    ));
    let r = get(&s, "/suggest?q=g+hello").await;
    assert_suggest(&r, "g hello", &["g hello world", "g hello kitty"]);
    assert_eq!(
        seen.lock().unwrap().request_lines,
        ["GET /complete?q=hello HTTP/1.1"]
    );
}

#[tokio::test]
async fn suggest_without_template_is_empty() {
    let s = server_with(GOOGLE);
    assert_suggest(&get(&s, "/suggest?q=g+hello").await, "g hello", &[]);
}

#[tokio::test]
async fn suggest_default_bang_has_no_prefix() {
    let (url, seen) = upstream(200, r#"["hello world",["hello world lyrics"]]"#).await;
    let s = server_with(&suggest_config(
        "g",
        &format!("{url}/complete?q={{{{{{s}}}}}}"),
        true,
    ));
    assert_suggest(
        &get(&s, "/suggest?q=hello+world").await,
        "hello world",
        &["hello world lyrics"],
    );
    assert_eq!(
        seen.lock().unwrap().request_lines,
        ["GET /complete?q=hello+world HTTP/1.1"]
    );
}

#[tokio::test]
async fn suggest_missing_query_or_config_is_empty() {
    let s = server_with(GOOGLE);
    assert_suggest(&get(&s, "/suggest").await, "", &[]);
    let s = Server::new("/nonexistent/bangs.yaml");
    assert_suggest(&get(&s, "/suggest?q=g+hello").await, "g hello", &[]);
}

#[tokio::test]
async fn suggest_upstream_failures_are_empty() {
    for (status, body) in [(500, r#"["x",["y"]]"#), (200, "not json")] {
        let (url, _) = upstream(status, body).await;
        let s = server_with(&suggest_config(
            "g",
            &format!("{url}/complete?q={{{{{{s}}}}}}"),
            false,
        ));
        assert_suggest(&get(&s, "/suggest?q=g+hello").await, "g hello", &[]);
    }
}

#[tokio::test]
async fn suggest_trigger_only_skips_upstream() {
    let (url, seen) = upstream(200, r#"["",["something"]]"#).await;
    let s = server_with(&suggest_config(
        "g",
        &format!("{url}/complete?q={{{{{{s}}}}}}"),
        false,
    ));
    assert_suggest(&get(&s, "/suggest?q=g").await, "g", &[]);
    assert!(seen.lock().unwrap().request_lines.is_empty());
}

#[tokio::test]
async fn suggest_parses_naver_format() {
    let (url, _) = upstream(
        200,
        r#"{"query":["춘천"],"answer":[],"intend":[],"items":[[["춘천 날씨"],["춘천 맛집"]]]}"#,
    )
    .await;
    let s = server_with(&suggest_config(
        "n",
        &format!("{url}/nx/ac?q={{{{{{s}}}}}}"),
        false,
    ));
    assert_suggest(
        &get(&s, "/suggest?q=n+%EC%B6%98%EC%B2%9C").await,
        "n 춘천",
        &["n 춘천 날씨", "n 춘천 맛집"],
    );
}

#[tokio::test]
async fn suggest_sends_firefox_user_agent() {
    // Wikimedia 등은 기본 클라이언트 UA를 로봇 정책 위반으로 거부한다
    let (url, seen) = upstream(200, r#"["hello",["hello world"]]"#).await;
    let s = server_with(&suggest_config(
        "g",
        &format!("{url}/complete?q={{{{{{s}}}}}}"),
        false,
    ));
    get(&s, "/suggest?q=g+hello").await;
    let ua = seen
        .lock()
        .unwrap()
        .user_agents
        .first()
        .cloned()
        .unwrap_or_default();
    assert!(
        ua.starts_with("Mozilla/5.0") && ua.contains("Firefox/"),
        "unexpected UA {ua:?}"
    );
}

#[tokio::test]
async fn suggest_korean_trigger_prefixes_as_typed() {
    let (url, _) = upstream(200, r#"["hello",["hello world"]]"#).await;
    let s = server_with(&suggest_config(
        "gh",
        &format!("{url}/complete?q={{{{{{s}}}}}}"),
        false,
    ));
    // "호" is "gh" typed with the Korean 2-set layout
    assert_suggest(
        &get(&s, "/suggest?q=%ED%98%B8+hello").await,
        "호 hello",
        &["호 hello world"],
    );
}

// ---------------------------------------------------------------------------
// server routes

#[tokio::test]
async fn health_reports_config_state() {
    let r = get(&server_with(GOOGLE), "/health").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.text(), r#"{"status":"ok"}"#);
    let r = get(&Server::new("/nonexistent/bangs.yaml"), "/health").await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(r.text(), r#"{"status":"degraded"}"#);
}

#[tokio::test]
async fn opensearch_descriptor() {
    let s = Server::new("/nonexistent/bangs.yaml");
    let r = get_with(
        &s,
        Request::get("/opensearch.xml").header("host", "localhost:8080"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        r.header("content-type"),
        "application/opensearchdescription+xml"
    );
    let body = r.text();
    assert!(body.contains(r#"template="http://localhost:8080/search?q={searchTerms}""#));
    assert!(body.contains(r#"template="http://localhost:8080/suggest?q={searchTerms}""#));
    assert!(body.contains(r#"type="application/x-suggestions+json""#));

    let r = get_with(
        &s,
        Request::get("/opensearch.xml")
            .header("host", "bangs.example.com")
            .header("x-forwarded-proto", "https"),
    )
    .await;
    assert!(
        r.text()
            .contains(r#"template="https://bangs.example.com/search?q={searchTerms}""#)
    );
}

#[tokio::test]
async fn root_serves_install_page() {
    let r = get(&Server::new("nonexistent.yaml"), "/").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header("content-type"), "text/html; charset=utf-8");
    assert!(r.text().contains(
        r#"<link rel="search" type="application/opensearchdescription+xml" title="Bangs" href="/opensearch.xml">"#
    ));
}

#[tokio::test]
async fn unknown_path_is_not_found() {
    let r = get(&Server::new("nonexistent.yaml"), "/nonexistent").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.text(), "404 page not found\n");
}

#[tokio::test]
async fn unclean_paths_redirect_like_serve_mux() {
    let s = server_with(GOOGLE);
    let r = get(&s, "/a/../search?q=g+x").await;
    assert_eq!(r.status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(r.header("location"), "/search?q=g+x");
    let r = get(&s, "/%73earch?q=g+x").await;
    assert_eq!(r.status, StatusCode::FOUND);
}

#[tokio::test]
async fn favicon_is_served() {
    let r = get(&Server::new("nonexistent.yaml"), "/favicon.ico").await;
    assert_eq!(r.header("content-type"), "image/png");
    assert_eq!(r.header("cache-control"), "public, max-age=86400");
    assert!(r.body.starts_with(b"\x89PNG"));
}

#[tokio::test]
async fn missing_host_is_rejected_like_net_http() {
    let s = server_with(GOOGLE);
    let r = get_with(&s, Request::get("/health")).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.text(), "400 Bad Request: missing required Host header");
}

// ---------------------------------------------------------------------------
// query key authentication

const KEY: &str = "0123456789abcdef0123456789abcdef";

fn auth_server(yaml: &str) -> Arc<Server> {
    Server::with_auth(
        &write_config(yaml),
        crate::auth::AuthKeys::parse(&format!("old-key, {KEY}")),
    )
}

#[tokio::test]
async fn auth_rejects_missing_or_wrong_key_as_not_found() {
    let s = auth_server(GOOGLE);
    for uri in [
        "/search?q=g+hello",
        "/search?q=g+hello&key=wrong",
        "/suggest?q=g+hello",
        "/opensearch.xml",
        "/",
        "/%73earch?q=g+x",
    ] {
        let r = get(&s, uri).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(r.text(), "404 page not found\n");
    }
}

#[tokio::test]
async fn auth_accepts_any_configured_key() {
    let s = auth_server(GOOGLE);
    let r = get(&s, &format!("/search?q=g+hello&key={KEY}")).await;
    assert_eq!(r.status, StatusCode::FOUND);
    // The key is not part of the search term.
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=hello"
    );
    let r = get(&s, "/search?key=old-key&q=hello").await;
    assert_eq!(
        r.header("location"),
        "https://www.google.com/search?q=hello"
    );
}

#[tokio::test]
async fn auth_leaves_health_and_favicon_open() {
    let s = auth_server(GOOGLE);
    assert_eq!(get(&s, "/health").await.status, StatusCode::OK);
    assert_eq!(get(&s, "/favicon.ico").await.status, StatusCode::OK);
}

#[tokio::test]
async fn auth_embeds_key_in_opensearch_and_install_page() {
    let s = auth_server(GOOGLE);
    let r = get(&s, &format!("/opensearch.xml?key={KEY}")).await;
    assert_eq!(r.status, StatusCode::OK);
    let body = r.text();
    assert!(body.contains(&format!(
        r#"template="http://localhost/search?q={{searchTerms}}&amp;key={KEY}""#
    )));
    assert!(body.contains(&format!(
        r#"template="http://localhost/suggest?q={{searchTerms}}&amp;key={KEY}""#
    )));
    let r = get(&s, &format!("/?key={KEY}")).await;
    assert!(
        r.text()
            .contains(&format!(r#"href="/opensearch.xml?key={KEY}">"#))
    );
}

#[tokio::test]
async fn auth_key_is_not_forwarded_upstream() {
    let (url, seen) = upstream(200, r#"["hello",["hello world"]]"#).await;
    let s = Server::with_auth(
        &write_config(&suggest_config(
            "g",
            &format!("{url}/complete?q={{{{{{s}}}}}}"),
            false,
        )),
        crate::auth::AuthKeys::parse(KEY),
    );
    let r = get(&s, &format!("/suggest?q=g+hello&key={KEY}")).await;
    assert_suggest(&r, "g hello", &["g hello world"]);
    assert_eq!(
        seen.lock().unwrap().request_lines,
        ["GET /complete?q=hello HTTP/1.1"]
    );
}
