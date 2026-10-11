//! Static HTML/XML bodies, byte-identical to the Go version.

pub const INSTALL_PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bangs</title>
    <link rel="search" type="application/opensearchdescription+xml" title="Bangs" href="/opensearch.xml">
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #eee;
        }
        .container { text-align: center; padding: 2rem; max-width: 600px; }
        .emoji { font-size: 5rem; margin-bottom: 1rem; }
        h1 { margin-bottom: 1.5rem; }
        .steps {
            text-align: left;
            margin-top: 1.5rem;
            padding: 1.5rem;
            background: rgba(255,255,255,0.1);
            border-radius: 10px;
            line-height: 1.8;
            color: #aaa;
        }
        code {
            background: rgba(255,255,255,0.15);
            padding: 0.1rem 0.4rem;
            border-radius: 4px;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="emoji">💣</div>
        <h1>Bangs</h1>
        <div class="steps">
            <strong>Add to Firefox:</strong><br>
            1. Right-click the address bar on this page<br>
            2. Click "Add Bangs" (or Settings → Search → add from this page)<br>
            3. Set Bangs as your default search engine to get bang redirects
            and search suggestions as you type
        </div>
    </div>
</body>
</html>
"##;

const OPENSEARCH_TEMPLATE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>Bangs</ShortName>
  <Description>Bangs search redirect service</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <Image width="16" height="16" type="image/png">{base}/favicon.ico</Image>
  <Url type="text/html" method="get" template="{base}/search?q={searchTerms}{key}"/>
  <Url type="application/x-suggestions+json" template="{base}/suggest?q={searchTerms}{key}"/>
</OpenSearchDescription>
"##;

/// The OpenSearch descriptor for `base`. `key_query` (`key=...`, or empty
/// when authentication is off) is appended to both URL templates.
pub fn opensearch_xml(base: &[u8], key_query: &[u8]) -> Vec<u8> {
    let mut key = Vec::new();
    if !key_query.is_empty() {
        key.extend_from_slice(b"&amp;");
        key.extend_from_slice(key_query);
    }
    let mut out = Vec::with_capacity(OPENSEARCH_TEMPLATE.len() + 3 * base.len() + 2 * key.len());
    let mut rest = OPENSEARCH_TEMPLATE;
    while let Some(i) = rest.find('{') {
        out.extend_from_slice(&rest.as_bytes()[..i]);
        rest = &rest[i..];
        if let Some(r) = rest.strip_prefix("{base}") {
            out.extend_from_slice(base);
            rest = r;
        } else if let Some(r) = rest.strip_prefix("{key}") {
            out.extend_from_slice(&key);
            rest = r;
        } else {
            out.push(b'{');
            rest = &rest[1..];
        }
    }
    out.extend_from_slice(rest.as_bytes());
    out
}

const OPENSEARCH_LINK: &str = r#"href="/opensearch.xml""#;

/// The install page; with a key, its OpenSearch link carries `key_query`
/// so the engine Firefox registers is already authenticated.
pub fn install_page(key_query: &[u8]) -> Vec<u8> {
    if key_query.is_empty() {
        return INSTALL_PAGE.as_bytes().to_vec();
    }
    let mut link = b"href=\"/opensearch.xml?".to_vec();
    link.extend_from_slice(key_query);
    link.push(b'"');
    let i = INSTALL_PAGE
        .find(OPENSEARCH_LINK)
        .expect("install page links opensearch.xml");
    let mut out = INSTALL_PAGE.as_bytes()[..i].to_vec();
    out.extend(link);
    out.extend_from_slice(&INSTALL_PAGE.as_bytes()[i + OPENSEARCH_LINK.len()..]);
    out
}

pub fn error_page(code: u16, emoji: &str, title: &str, message: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="ko">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{code} {title}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #eee;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            max-width: 600px;
        }}
        .emoji {{
            font-size: 5rem;
            margin-bottom: 1rem;
            animation: bounce 2s ease-in-out infinite;
        }}
        @keyframes bounce {{
            0%, 100% {{ transform: translateY(0); }}
            50% {{ transform: translateY(-10px); }}
        }}
        .code {{
            font-size: 6rem;
            font-weight: bold;
            color: #e94560;
            text-shadow: 0 0 20px rgba(233, 69, 96, 0.5);
        }}
        .title {{
            font-size: 1.5rem;
            margin: 1rem 0;
            color: #0f3460;
            background: #eee;
            padding: 0.5rem 1.5rem;
            border-radius: 25px;
            display: inline-block;
        }}
        .message {{
            margin-top: 1.5rem;
            padding: 1rem;
            background: rgba(255,255,255,0.1);
            border-radius: 10px;
            font-size: 1rem;
            line-height: 1.6;
            color: #aaa;
        }}
        .footer {{
            margin-top: 2rem;
            font-size: 0.8rem;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="emoji">{emoji}</div>
        <div class="code">{code}</div>
        <div class="title">{title}</div>
        <div class="message">{message}</div>
        <div class="footer">Bangs Search Service</div>
    </div>
</body>
</html>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opensearch_without_key_is_unchanged() {
        let xml = String::from_utf8(opensearch_xml(b"http://h", b"")).unwrap();
        assert!(xml.contains(r#"template="http://h/search?q={searchTerms}"/>"#));
        assert!(xml.contains(r#"template="http://h/suggest?q={searchTerms}"/>"#));
        assert!(!xml.contains("{key}"));
    }

    #[test]
    fn opensearch_with_key_escapes_ampersand() {
        let xml = String::from_utf8(opensearch_xml(b"https://h", b"key=abc")).unwrap();
        assert!(xml.contains(r#"template="https://h/search?q={searchTerms}&amp;key=abc"/>"#));
        assert!(xml.contains(r#"template="https://h/suggest?q={searchTerms}&amp;key=abc"/>"#));
    }

    #[test]
    fn install_page_link_carries_key() {
        assert_eq!(install_page(b""), INSTALL_PAGE.as_bytes());
        let page = String::from_utf8(install_page(b"key=abc")).unwrap();
        assert!(page.contains(r#"href="/opensearch.xml?key=abc">"#));
    }
}
