package main

import (
	"encoding/json"
	"io"
	"net/http"
	"time"
)

const suggestMaxBodySize = 1 << 20 // 1MB

// Wikimedia 등은 기본 Go-http-client UA를 로봇 정책 위반으로 거부한다.
// 실사용자의 Firefox 입력을 대리하는 요청이므로 최신 Firefox UA를 사용
const suggestUserAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:152.0) Gecko/20100101 Firefox/152.0"

type SuggestHandler struct {
	repo   *Repository
	parser *Parser
	client *http.Client
}

func NewSuggestHandler(repo *Repository, parser *Parser) *SuggestHandler {
	return &SuggestHandler{
		repo:   repo,
		parser: parser,
		client: &http.Client{Timeout: 2 * time.Second},
	}
}

func (h *SuggestHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	query := r.URL.Query().Get("q")
	writeSuggestions(w, query, h.fetchSuggestions(r, query))
}

func (h *SuggestHandler) fetchSuggestions(r *http.Request, query string) []string {
	if h.repo == nil || query == "" {
		return nil
	}
	parsed := h.parser.Parse(query)
	bang, searchTerm, viaTrigger := resolveBang(h.repo, query, parsed)
	if bang == nil || bang.SuggestURLTemplate == "" || searchTerm == "" {
		return nil
	}

	upstream := h.fetchUpstream(r, bang.BuildSuggestURL(searchTerm))
	if !viaTrigger {
		return upstream
	}
	prefixed := make([]string, len(upstream))
	for i, s := range upstream {
		prefixed[i] = parsed.FirstWord + " " + s
	}
	return prefixed
}

func (h *SuggestHandler) fetchUpstream(r *http.Request, suggestURL string) []string {
	req, err := http.NewRequestWithContext(r.Context(), http.MethodGet, suggestURL, nil)
	if err != nil {
		return nil
	}
	req.Header.Set("User-Agent", suggestUserAgent)
	resp, err := h.client.Do(req)
	if err != nil {
		return nil
	}
	defer func() { _ = resp.Body.Close() }()
	if resp.StatusCode != http.StatusOK {
		return nil
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, suggestMaxBodySize))
	if err != nil {
		return nil
	}
	return parseSuggestions(body)
}

// parseSuggestions extracts the completion list from an OpenSearch
// suggestions response: [query, [completions], ...trailing elements].
func parseSuggestions(body []byte) []string {
	var elements []json.RawMessage
	if err := json.Unmarshal(body, &elements); err != nil || len(elements) < 2 {
		return nil
	}
	var suggestions []string
	if err := json.Unmarshal(elements[1], &suggestions); err != nil {
		return nil
	}
	return suggestions
}

func writeSuggestions(w http.ResponseWriter, query string, suggestions []string) {
	if suggestions == nil {
		suggestions = []string{}
	}
	w.Header().Set("Content-Type", "application/x-suggestions+json")
	_ = json.NewEncoder(w).Encode([]any{query, suggestions})
}
