package main

import (
	"fmt"
	"net/http"
)

type SearchHandler struct {
	repo   *Repository
	parser *Parser
}

func NewSearchHandler(repo *Repository, parser *Parser) *SearchHandler {
	return &SearchHandler{
		repo:   repo,
		parser: parser,
	}
}

func (h *SearchHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	if h.repo == nil {
		renderErrorPage(w, http.StatusServiceUnavailable,
			"🔧",
			"Service Unavailable",
			"Failed to load configuration file. 😢<br>Please contact the administrator or check the config file.")
		return
	}

	query := r.URL.Query().Get("q")
	if query == "" {
		renderErrorPage(w, http.StatusBadRequest,
			"🤔",
			"Bad Request",
			"Search query is required! 🔍<br>Please provide a search term in the 'q' parameter.")
		return
	}
	parsed := h.parser.Parse(query)

	bang := h.repo.FindByTrigger(parsed.FirstWord)
	searchTerm := parsed.SearchTerm
	if bang == nil {
		converted := KoreanToQwerty(parsed.FirstWord)
		if converted != parsed.FirstWord {
			bang = h.repo.FindByTrigger(converted)
		}
		if bang == nil {
			bang = h.repo.GetDefault()
			searchTerm = query
		}
	}
	if bang == nil {
		renderErrorPage(w, http.StatusServiceUnavailable,
			"⚙️",
			"Service Unavailable",
			"No default search engine configured. 😅<br>Please check 'default_bang' in your config file.")
		return
	}
	redirectURL := bang.BuildURL(searchTerm)

	http.Redirect(w, r, redirectURL, http.StatusFound)
}

func renderErrorPage(w http.ResponseWriter, code int, emoji, title, message string) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(code)
	html := fmt.Sprintf(`<!DOCTYPE html>
<html lang="ko">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>%d %s</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%%, #16213e 100%%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #eee;
        }
        .container {
            text-align: center;
            padding: 2rem;
            max-width: 600px;
        }
        .emoji {
            font-size: 5rem;
            margin-bottom: 1rem;
            animation: bounce 2s ease-in-out infinite;
        }
        @keyframes bounce {
            0%%, 100%% { transform: translateY(0); }
            50%% { transform: translateY(-10px); }
        }
        .code {
            font-size: 6rem;
            font-weight: bold;
            color: #e94560;
            text-shadow: 0 0 20px rgba(233, 69, 96, 0.5);
        }
        .title {
            font-size: 1.5rem;
            margin: 1rem 0;
            color: #0f3460;
            background: #eee;
            padding: 0.5rem 1.5rem;
            border-radius: 25px;
            display: inline-block;
        }
        .message {
            margin-top: 1.5rem;
            padding: 1rem;
            background: rgba(255,255,255,0.1);
            border-radius: 10px;
            font-size: 1rem;
            line-height: 1.6;
            color: #aaa;
        }
        .footer {
            margin-top: 2rem;
            font-size: 0.8rem;
            color: #666;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="emoji">%s</div>
        <div class="code">%d</div>
        <div class="title">%s</div>
        <div class="message">%s</div>
        <div class="footer">Bangs Search Service</div>
    </div>
</body>
</html>`, code, title, emoji, code, title, message)
	fmt.Fprint(w, html)
}
