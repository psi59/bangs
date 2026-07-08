package main

import (
	"fmt"
	"net/http"
)

func openSearchHandler(w http.ResponseWriter, r *http.Request) {
	base := baseURL(r)
	w.Header().Set("Content-Type", "application/opensearchdescription+xml")
	fmt.Fprintf(w, `<?xml version="1.0" encoding="UTF-8"?>
<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/">
  <ShortName>Bangs</ShortName>
  <Description>Bangs search redirect service</Description>
  <InputEncoding>UTF-8</InputEncoding>
  <Image width="16" height="16" type="image/png">%s/favicon.ico</Image>
  <Url type="text/html" method="get" template="%s/search?q={searchTerms}"/>
  <Url type="application/x-suggestions+json" template="%s/suggest?q={searchTerms}"/>
</OpenSearchDescription>
`, base, base, base)
}

func rootHandler(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	fmt.Fprint(w, `<!DOCTYPE html>
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
`)
}

func baseURL(r *http.Request) string {
	scheme := r.Header.Get("X-Forwarded-Proto")
	if scheme == "" {
		if r.TLS != nil {
			scheme = "https"
		} else {
			scheme = "http"
		}
	}
	return scheme + "://" + r.Host
}
