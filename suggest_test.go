package main

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/rs/zerolog"
)

func TestSuggestHandler_BangMatch_ProxiesAndPrefixes(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.URL.Query().Get("q"); got != "hello" {
			t.Errorf("expected upstream query 'hello', got '%s'", got)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`["hello",["hello world","hello kitty"]]`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); ct != "application/x-suggestions+json" {
		t.Errorf("expected Content-Type 'application/x-suggestions+json', got '%s'", ct)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{"g hello world", "g hello kitty"})
}

func TestSuggestHandler_NoSuggestTemplate_ReturnsEmpty(t *testing.T) {
	repo := NewRepository()
	repo.Add(NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", ""))
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{})
}

func TestSuggestHandler_DefaultBang_NoPrefix(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if got := r.URL.Query().Get("q"); got != "hello world" {
			t.Errorf("expected upstream query 'hello world', got '%s'", got)
		}
		_, _ = w.Write([]byte(`["hello world",["hello world lyrics"]]`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	repo.SetDefault("g")
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=hello+world", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	assertSuggestResponse(t, rec.Body.Bytes(), "hello world", []string{"hello world lyrics"})
}

func TestSuggestHandler_MissingQuery_ReturnsEmpty(t *testing.T) {
	repo := NewRepository()
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "", []string{})
}

func TestSuggestHandler_NilRepo_ReturnsEmpty(t *testing.T) {
	handler := NewSuggestHandler(nil, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{})
}

func TestSuggestHandler_UpstreamError_ReturnsEmpty(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{})
}

func TestSuggestHandler_UpstreamInvalidJSON_ReturnsEmpty(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`not json`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{})
}

func TestSuggestHandler_TriggerOnly_NoUpstreamCall(t *testing.T) {
	called := false
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		called = true
		_, _ = w.Write([]byte(`["",["something"]]`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if called {
		t.Error("expected no upstream call for trigger-only query")
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g", []string{})
}

func TestSuggestHandler_SendsFirefoxUserAgent(t *testing.T) {
	// Wikimedia 등은 Go 기본 UA(Go-http-client)를 로봇 정책 위반으로 거부한다
	var gotUA string
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		gotUA = r.Header.Get("User-Agent")
		_, _ = w.Write([]byte(`["hello",["hello world"]]`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if !strings.HasPrefix(gotUA, "Mozilla/5.0") || !strings.Contains(gotUA, "Firefox/") {
		t.Errorf("expected Firefox User-Agent, got '%s'", gotUA)
	}
}

func TestSuggestHandler_KoreanTrigger_PrefixesAsTyped(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`["hello",["hello world"]]`))
	}))
	t.Cleanup(upstream.Close)

	repo := NewRepository()
	bang := NewBang("gh", "GitHub", "https://github.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = upstream.URL + "/complete?q={{{s}}}"
	repo.Add(bang)
	handler := NewSuggestHandler(repo, NewParser())

	// "호" is "gh" typed with the Korean 2-set layout
	req := httptest.NewRequest(http.MethodGet, "/suggest?q=%ED%98%B8+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	assertSuggestResponse(t, rec.Body.Bytes(), "호 hello", []string{"호 hello world"})
}

func TestServer_SuggestRoute(t *testing.T) {
	upstream := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write([]byte(`["hello",["hello world"]]`))
	}))
	t.Cleanup(upstream.Close)

	configPath := filepath.Join(t.TempDir(), "bangs.yaml")
	config := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
    suggest_url_template: "` + upstream.URL + `/complete?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(config), 0o644); err != nil {
		t.Fatalf("failed to write config: %v", err)
	}
	server := NewServer(configPath, zerolog.Nop())

	req := httptest.NewRequest(http.MethodGet, "/suggest?q=g+hello", nil)
	rec := httptest.NewRecorder()

	server.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	assertSuggestResponse(t, rec.Body.Bytes(), "g hello", []string{"g hello world"})
}

func assertSuggestResponse(t *testing.T, body []byte, wantQuery string, wantSuggestions []string) {
	t.Helper()
	var query string
	var suggestions []string
	raw := []any{&query, &suggestions}
	if err := json.Unmarshal(body, &raw); err != nil {
		t.Fatalf("failed to parse response body '%s': %v", body, err)
	}
	if query != wantQuery {
		t.Errorf("expected query '%s', got '%s'", wantQuery, query)
	}
	if len(suggestions) != len(wantSuggestions) {
		t.Fatalf("expected %d suggestions %v, got %d: %v", len(wantSuggestions), wantSuggestions, len(suggestions), suggestions)
	}
	for i, want := range wantSuggestions {
		if suggestions[i] != want {
			t.Errorf("suggestion[%d]: expected '%s', got '%s'", i, want, suggestions[i])
		}
	}
}
