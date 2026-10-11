# Bangs

> 100% written by LLM (Claude)

DuckDuckGo Bangs-style search redirect service.

## Features

- **Bang search**: `g hello` → Google search for "hello"
- **Default engine**: Search without bang uses your default engine
- **Homepage**: `yt` (no query) → YouTube home
- **Custom triggers**: `!g`, `@g`, `#g` - any format you want
- **Search suggestions**: live autocomplete from the target engine as you type (Firefox)
- **Auto-reload**: Config changes apply without restart

## Quick Start

### 1. Create `bangs.yaml`

```yaml
default_bang: g

bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
    suggest_url_template: "https://suggestqueries.google.com/complete/search?client=firefox&q={{{s}}}"

  - trigger: yt
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"
    suggest_url_template: "https://suggestqueries.google.com/complete/search?client=firefox&ds=yt&q={{{s}}}"

  - trigger: gh
    name: GitHub
    url_template: "https://github.com/search?q={{{s}}}"
```

### 2. Run with Docker

```bash
docker run -d -p 8080:8080 \
  -v $(pwd)/bangs.yaml:/app/bangs.yaml \
  ghcr.io/psi59/bangs:latest
```

### 3. Register in your browser

#### Firefox

1. Open `http://localhost:8080/` in Firefox
2. Right-click the address bar → **Add "Bangs"**
   (or Settings → Search → Search Shortcuts, the page offers the engine automatically)
3. Set Bangs as your default search engine
4. Type `g hello` in the address bar — suggestions from the target engine
   appear as you type (for bangs with `suggest_url_template`)

#### Chrome

##### As Custom Search Engine

1. Settings → Search engine → Manage search engines → Add
2. Configure:
   - **Search engine**: `Bangs`
   - **Shortcut**: `b`
   - **URL**: `http://localhost:8080/search?q=%s`
3. Type `b g hello` in address bar → Google search for "hello"

##### As Default Search Engine

1. After adding as custom search engine
2. Find "Bangs" in the list, click ⋮ → **Make default**
3. Now type `g hello` directly in address bar

## Configuration

| Field | Description | Required |
|-------|-------------|----------|
| `trigger` | Bang trigger (e.g., `g`, `!g`, `@yt`) | Yes |
| `name` | Search engine name | Yes |
| `url_template` | Search URL. `{{{s}}}` is replaced with query | Yes |
| `home_url` | URL when no query. Auto-extracted if omitted | No |
| `suggest_url_template` | Autocomplete API URL ([OpenSearch suggestions](https://developer.mozilla.org/en-US/docs/Web/XML/Guides/OpenSearch) or Naver `ac.search.naver.com` format). Enables live suggestions | No |

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BANGS_CONFIG_PATH` | Config file path | `./bangs.yaml` |
| `BANGS_PORT` | Server port | `8080` |
| `BANGS_AUTH_KEYS` | Comma-separated access keys. Empty disables authentication | (empty) |

## Authentication

Set `BANGS_AUTH_KEYS` to require a `key` query parameter on `/`, `/search`,
`/suggest` and `/opensearch.xml`. Requests without a valid key get a plain
`404`. `/health` and `/favicon.ico` stay open for health checks.

```bash
docker run -d -p 8080:8080 \
  -v $(pwd)/bangs.yaml:/app/bangs.yaml \
  -e BANGS_AUTH_KEYS="$(openssl rand -hex 32)" \
  ghcr.io/psi59/bangs:latest
```

Register the engine from `https://<host>/?key=<key>`: the page then serves an
OpenSearch description whose search and suggestion URLs already carry the key,
so Firefox sends it automatically. For Chrome, append the key to the search
engine URL: `https://<host>/search?q=%s&key=<key>`.

The key travels in the URL because browsers cannot add headers to address-bar
searches, so it can end up in browser history and synced settings. Use a long
random key, serve over HTTPS, and rotate by listing the new key next to the old
one (`BANGS_AUTH_KEYS=new,old`) until every browser is re-registered. The
server never logs query strings.

## Alternative Installation

```bash
# Install with Cargo
cargo install --git https://github.com/psi59/bangs

# Or build from source
git clone https://github.com/psi59/bangs.git
cd bangs && cargo build --release

# Run
BANGS_CONFIG_PATH=./bangs.yaml ./target/release/bangs
```

## Development

```bash
git config core.hooksPath .githooks   # fmt, clippy and tests before each commit
cargo test
```

## License

MIT
