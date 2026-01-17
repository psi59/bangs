# Bangs

> 100% written by LLM (Claude)

DuckDuckGo Bangs-style search redirect service.

## Features

- **Bang search**: `g hello` → Google search for "hello"
- **Default engine**: Search without bang uses your default engine
- **Homepage**: `yt` (no query) → YouTube home
- **Custom triggers**: `!g`, `@g`, `#g` - any format you want
- **Auto-reload**: Config changes apply without restart

## Quick Start

### 1. Create `bangs.yaml`

```yaml
default_bang: g

bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"

  - trigger: yt
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"

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

### 3. Register in Chrome

#### As Custom Search Engine

1. Settings → Search engine → Manage search engines → Add
2. Configure:
   - **Search engine**: `Bangs`
   - **Shortcut**: `b`
   - **URL**: `http://localhost:8080/search?q=%s`
3. Type `b g hello` in address bar → Google search for "hello"

#### As Default Search Engine

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

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BANGS_CONFIG_PATH` | Config file path | `./bangs.yaml` |
| `BANGS_PORT` | Server port | `8080` |

## Alternative Installation

```bash
# Install with Go
go install github.com/psi59/bangs@latest

# Or build from source
git clone https://github.com/psi59/bangs.git
cd bangs && go build .

# Run
BANGS_CONFIG_PATH=./bangs.yaml ./bangs
```

## License

MIT
