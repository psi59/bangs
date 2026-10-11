use std::convert::Infallible;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwapOption;
use bytes::Bytes;
use futures_util::FutureExt;
use hyper::body::Incoming;
use hyper::header::{
    CACHE_CONTROL, CONNECTION, CONTENT_LENGTH, HOST, HeaderValue, USER_AGENT,
    X_CONTENT_TYPE_OPTIONS,
};
use hyper::{Method, Request, StatusCode, Version};
use hyper_util::rt::{TokioIo, TokioTimer};
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::auth::{Access, AuthKeys, key_query};
use crate::bang::Repository;
use crate::config::Config;
use crate::gocompat::{
    self, clean_path, escape, lossy_utf8, path_clean, path_unescape, request_paths,
};
use crate::handler::{self, Ctx, Resp, TEXT_HTML, response};
use crate::pages;

static FAVICON_PNG: &[u8] = include_bytes!("../favicon.png");

pub struct Server {
    repo: ArcSwapOption<Repository>,
    config_path: String,
    client: reqwest::Client,
    auth: AuthKeys,
}

impl Server {
    #[cfg(test)]
    pub fn new(config_path: &str) -> Arc<Self> {
        Self::with_auth(config_path, AuthKeys::default())
    }

    pub fn with_auth(config_path: &str, auth: AuthKeys) -> Arc<Self> {
        // ring is the TLS crypto provider; ignore "already installed".
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(handler::SUGGEST_USER_AGENT),
        );
        let client = reqwest::Client::builder()
            .timeout(handler::SUGGEST_TIMEOUT)
            .default_headers(headers)
            .build()
            .expect("failed to build HTTP client");
        let server = Arc::new(Self {
            repo: ArcSwapOption::empty(),
            config_path: config_path.to_owned(),
            client,
            auth,
        });
        server.load_config();
        server
    }

    pub fn load_config(&self) {
        match Config::from_file(&self.config_path) {
            Ok(config) => {
                self.repo.store(Some(Arc::new(config.to_repository())));
                info!(path = %self.config_path, "config loaded");
            }
            Err(e) => {
                error!(error = %e, "failed to load config");
                self.repo.store(None);
            }
        }
    }

    /// Watches the config file's directory (works with editors' atomic
    /// saves and Docker bind mounts) and reloads on write or create.
    pub fn watch_config(self: &Arc<Self>) -> Result<notify::RecommendedWatcher, String> {
        let config_dir = go_dir(&self.config_path);
        let config_base = go_base(&self.config_path);
        let server = Arc::clone(self);
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    if !event
                        .paths
                        .iter()
                        .any(|p| p.file_name().is_some_and(|n| n == config_base.as_str()))
                    {
                        return;
                    }
                    let op = match event.kind {
                        EventKind::Create(_)
                        | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => "CREATE",
                        EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) => "WRITE",
                        EventKind::Modify(ModifyKind::Name(RenameMode::Any))
                            if Path::new(&server.config_path).exists() =>
                        {
                            "CREATE"
                        }
                        _ => return,
                    };
                    info!(path = %server.config_path, event = op, "config file changed, reloading");
                    server.load_config();
                }
                Err(e) => error!(error = %e, "watcher error"),
            })
            .map_err(|e| format!("failed to create watcher: {e}"))?;
        watcher
            .watch(Path::new(&config_dir), RecursiveMode::NonRecursive)
            .map_err(|e| format!("failed to watch config directory: {}", go_error_text(&e)))?;
        info!(dir = %config_dir, "watching config directory");
        Ok(watcher)
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener) {
        let mut builder = hyper::server::conn::http1::Builder::new();
        builder.title_case_headers(true).timer(TokioTimer::new());
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let _ = stream.set_nodelay(true);
            let server = Arc::clone(&self);
            let conn = builder.serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(move |req| {
                    let server = Arc::clone(&server);
                    async move { Ok::<_, Infallible>(server.handle(req).await) }
                }),
            );
            tokio::spawn(conn.with_upgrades());
        }
    }

    pub async fn handle(&self, req: Request<Incoming>) -> Resp {
        let (parts, _body) = req.into_parts();
        self.handle_parts(&parts).await
    }

    /// The request pipeline: net/http's request validation, then the
    /// recover → logging → mux chain of the Go server.
    pub async fn handle_parts(&self, parts: &hyper::http::request::Parts) -> Resp {
        let method = &parts.method;
        let uri = &parts.uri;
        let hosts: Vec<&HeaderValue> = parts.headers.get_all(HOST).iter().collect();
        if hosts.len() > 1 {
            return bad_request(None);
        }
        let is_asterisk = uri.authority().is_none() && uri.path() == "*";
        let raw_path = if is_asterisk {
            b"*".as_slice()
        } else {
            uri.path().as_bytes()
        };
        let Ok((path, escaped_path)) = request_paths(raw_path) else {
            return bad_request(None);
        };
        if parts.version >= Version::HTTP_11 && hosts.is_empty() && method != Method::CONNECT {
            return bad_request(Some("missing required Host header"));
        }
        if let [host] = hosts.as_slice()
            && !gocompat::valid_host_header(host.as_bytes())
        {
            return bad_request(Some("malformed Host header"));
        }
        if is_asterisk && method == Method::OPTIONS {
            let mut resp = response(StatusCode::OK, None, Bytes::new());
            resp.headers_mut()
                .insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
            return resp;
        }

        let host: &[u8] = match uri.authority() {
            Some(a) => a.as_str().as_bytes(),
            None => hosts.first().map_or(b"".as_slice(), |h| h.as_bytes()),
        };
        let ctx = Ctx {
            method,
            escaped_path: &escaped_path,
            raw_query: uri.query().unwrap_or("").as_bytes(),
        };

        let start = Instant::now();
        let routed = AssertUnwindSafe(self.route(&ctx, parts, host, is_asterisk))
            .catch_unwind()
            .await;
        let Ok(resp) = routed else {
            let mut resp = response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("text/plain; charset=utf-8"),
                "internal server error\n",
            );
            resp.headers_mut()
                .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            return resp;
        };
        info!(
            method = %method,
            path = %lossy_utf8(&path),
            status = resp.status().as_u16(),
            duration = start.elapsed().as_secs_f64() * 1000.0,
            "request"
        );
        resp
    }

    async fn route(
        &self,
        ctx: &Ctx<'_>,
        parts: &hyper::http::request::Parts,
        host: &[u8],
        asterisk: bool,
    ) -> Resp {
        if asterisk {
            let mut resp = response(StatusCode::BAD_REQUEST, None, Bytes::new());
            if parts.version >= Version::HTTP_11 {
                resp.headers_mut()
                    .insert(CONNECTION, HeaderValue::from_static("close"));
            }
            return resp;
        }
        let path = if ctx.method == Method::CONNECT {
            ctx.escaped_path.to_vec()
        } else {
            let cleaned = clean_path(ctx.escaped_path);
            if cleaned != ctx.escaped_path {
                let mut target = escape(&cleaned, gocompat::Mode::Path);
                if !ctx.raw_query.is_empty() {
                    target.push(b'?');
                    target.extend_from_slice(ctx.raw_query);
                }
                return handler::redirect(ctx, &target, StatusCode::TEMPORARY_REDIRECT);
            }
            cleaned
        };
        let route = route_name(&path);
        // /health and /favicon.ico stay open for health checks and browsers;
        // a bad key looks exactly like an unknown path.
        let key_query = match route {
            Some(Route::Health | Route::Favicon) | None => Vec::new(),
            Some(_) => match self.auth.check(ctx.raw_query) {
                Access::Open => Vec::new(),
                Access::Granted(key) => key_query(&key),
                Access::Denied => return not_found(),
            },
        };
        match route {
            Some(Route::Search) => {
                let repo = self.repo.load();
                handler::search(ctx, repo.as_deref())
            }
            Some(Route::Suggest) => {
                let repo = self.repo.load_full();
                handler::suggest(ctx, repo.as_deref(), &self.client).await
            }
            Some(Route::Health) => {
                if self.repo.load().is_some() {
                    response(
                        StatusCode::OK,
                        Some("application/json"),
                        r#"{"status":"ok"}"#,
                    )
                } else {
                    response(
                        StatusCode::SERVICE_UNAVAILABLE,
                        Some("application/json"),
                        r#"{"status":"degraded"}"#,
                    )
                }
            }
            Some(Route::Favicon) => {
                let mut resp = response(
                    StatusCode::OK,
                    Some("image/png"),
                    Bytes::from_static(FAVICON_PNG),
                );
                resp.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                );
                resp
            }
            Some(Route::OpenSearch) => {
                let proto = parts
                    .headers
                    .get("x-forwarded-proto")
                    .map_or(&b""[..], HeaderValue::as_bytes);
                let mut base = if proto.is_empty() {
                    b"http".to_vec()
                } else {
                    proto.to_vec()
                };
                base.extend_from_slice(b"://");
                base.extend_from_slice(host);
                response(
                    StatusCode::OK,
                    Some("application/opensearchdescription+xml"),
                    pages::opensearch_xml(&base, &key_query),
                )
            }
            Some(Route::Root) => response(
                StatusCode::OK,
                Some(TEXT_HTML),
                pages::install_page(&key_query),
            ),
            None => not_found(),
        }
    }
}

/// http.NotFound.
fn not_found() -> Resp {
    let mut resp = response(
        StatusCode::NOT_FOUND,
        Some("text/plain; charset=utf-8"),
        "404 page not found\n",
    );
    resp.headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    resp
}

#[derive(Clone, Copy)]
enum Route {
    Search,
    Suggest,
    Health,
    Favicon,
    OpenSearch,
    Root,
}

/// ServeMux matching for single-segment patterns: segments are compared
/// after path-unescaping, so `/%73earch` reaches /search and `/%2F` the
/// root pattern's trailing-slash segment.
fn route_name(path: &[u8]) -> Option<Route> {
    if path == b"/" {
        return Some(Route::Root);
    }
    let rest = path.strip_prefix(b"/")?;
    if rest.contains(&b'/') {
        return None;
    }
    match path_unescape(rest).as_slice() {
        b"/" => Some(Route::Root),
        b"search" => Some(Route::Search),
        b"suggest" => Some(Route::Suggest),
        b"health" => Some(Route::Health),
        b"favicon.ico" => Some(Route::Favicon),
        b"opensearch.xml" => Some(Route::OpenSearch),
        _ => None,
    }
}

/// The raw responses net/http writes for requests it rejects before any
/// handler runs: plain "400 Bad Request", or a statusError with detail.
fn bad_request(detail: Option<&'static str>) -> Resp {
    let (reason, body) = match detail {
        None => ("Bad Request".to_owned(), "400 Bad Request".to_owned()),
        Some(d) => (format!("Bad Request: {d}"), format!("400 Bad Request: {d}")),
    };
    let mut resp = response(
        StatusCode::BAD_REQUEST,
        Some("text/plain; charset=utf-8"),
        body,
    );
    resp.headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    if let Ok(phrase) = hyper::ext::ReasonPhrase::try_from(reason.into_bytes()) {
        resp.extensions_mut().insert(phrase);
    }
    resp
}

/// filepath.Dir.
fn go_dir(path: &str) -> String {
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "",
    };
    String::from_utf8_lossy(&path_clean(dir.as_bytes())).into_owned()
}

/// filepath.Base.
fn go_base(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if path.is_empty() {
        return ".".into();
    }
    if trimmed.is_empty() {
        return "/".into();
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_owned()
}

/// Go-style lowercase OS error text ("no such file or directory").
fn go_error_text(e: &notify::Error) -> String {
    if let notify::ErrorKind::Io(io) = &e.kind
        && let Some(code) = io.raw_os_error()
    {
        let msg = std::io::Error::from_raw_os_error(code).to_string();
        let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
        let mut chars = msg.chars();
        return chars
            .next()
            .map(|c| c.to_lowercase().chain(chars).collect())
            .unwrap_or(msg);
    }
    e.to_string()
}

/// Resolves BANGS_PORT like Go's net package: a number, or a service name
/// from /etc/services (falling back to Go's built-in table).
pub fn resolve_port(port: &str) -> Result<u16, String> {
    if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) {
        return port
            .parse::<u16>()
            .map_err(|_| format!("listen tcp: address {port}: invalid port"));
    }
    let lower = port.to_ascii_lowercase();
    if let Ok(services) = std::fs::read_to_string("/etc/services") {
        for line in services.lines() {
            let line = line.split('#').next().unwrap_or("");
            let mut fields = line.split_whitespace();
            let (Some(name), Some(portproto)) = (fields.next(), fields.next()) else {
                continue;
            };
            let Some((num, "tcp")) = portproto.split_once('/') else {
                continue;
            };
            if (name == lower || fields.any(|alias| alias == lower))
                && let Ok(n) = num.parse()
            {
                return Ok(n);
            }
        }
    }
    let builtin = match lower.as_str() {
        "domain" => Some(53),
        "ftp" => Some(21),
        "ftps" => Some(990),
        "gopher" => Some(70),
        "http" => Some(80),
        "https" => Some(443),
        "imap2" => Some(143),
        "imap3" => Some(220),
        "imaps" => Some(993),
        "pop3" => Some(110),
        "pop3s" => Some(995),
        "smtp" => Some(25),
        "submissions" => Some(465),
        "ssh" => Some(22),
        "telnet" => Some(23),
        _ => None,
    };
    builtin.ok_or_else(|| format!("listen tcp: lookup tcp/{port}: unknown port"))
}

/// Listens on all interfaces, IPv4 and IPv6, like Go's ":port".
pub fn listen(port: u16) -> Result<TcpListener, String> {
    use socket2::{Domain, Protocol, Socket, Type};
    let bind = |domain: Domain, addr: SocketAddr| -> std::io::Result<std::net::TcpListener> {
        let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
        if domain == Domain::IPV6 {
            socket.set_only_v6(false)?;
        }
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        socket.bind(&addr.into())?;
        socket.listen(4096)?;
        Ok(socket.into())
    };
    let listener = bind(Domain::IPV6, SocketAddr::from(([0u16; 8], port)))
        .or_else(|_| bind(Domain::IPV4, SocketAddr::from(([0u8; 4], port))))
        .map_err(|e| format!("listen tcp :{port}: bind: {}", go_io_text(&e)))?;
    TcpListener::from_std(listener).map_err(|e| e.to_string())
}

fn go_io_text(e: &std::io::Error) -> String {
    let msg = e.to_string();
    let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
    let mut chars = msg.chars();
    chars
        .next()
        .map(|c| c.to_lowercase().chain(chars).collect())
        .unwrap_or(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_dir_and_base() {
        assert_eq!(go_dir("./bangs.yaml"), ".");
        assert_eq!(go_dir("bangs.yaml"), ".");
        assert_eq!(go_dir("/app/bangs.yaml"), "/app");
        assert_eq!(go_dir("a//b/c.yaml"), "a/b");
        assert_eq!(go_base("/app/bangs.yaml"), "bangs.yaml");
        assert_eq!(go_base("bangs.yaml"), "bangs.yaml");
    }

    #[test]
    fn route_matching() {
        assert!(matches!(route_name(b"/"), Some(Route::Root)));
        assert!(matches!(route_name(b"/%73earch"), Some(Route::Search)));
        assert!(route_name(b"/search/").is_none());
        assert!(route_name(b"/nonexistent").is_none());
    }

    #[test]
    fn port_resolution() {
        assert_eq!(resolve_port("8080"), Ok(8080));
        assert_eq!(resolve_port("http"), Ok(80));
        assert!(resolve_port("99999").is_err());
        assert!(resolve_port("nope-not-a-service").is_err());
    }
}
