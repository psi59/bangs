package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/rs/zerolog"
)

func TestOpenSearchHandler_ReturnsDescriptor(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "http://localhost:8080/opensearch.xml", nil)
	rec := httptest.NewRecorder()

	openSearchHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); ct != "application/opensearchdescription+xml" {
		t.Errorf("expected Content-Type 'application/opensearchdescription+xml', got '%s'", ct)
	}
	body := rec.Body.String()
	searchURL := `template="http://localhost:8080/search?q={searchTerms}"`
	if !strings.Contains(body, searchURL) {
		t.Errorf("expected body to contain '%s', got: %s", searchURL, body)
	}
	suggestURL := `template="http://localhost:8080/suggest?q={searchTerms}"`
	if !strings.Contains(body, suggestURL) {
		t.Errorf("expected body to contain '%s', got: %s", suggestURL, body)
	}
	if !strings.Contains(body, `type="application/x-suggestions+json"`) {
		t.Errorf("expected body to declare suggestions Url type, got: %s", body)
	}
}

func TestOpenSearchHandler_ForwardedProto_UsesHTTPS(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "http://bangs.example.com/opensearch.xml", nil)
	req.Header.Set("X-Forwarded-Proto", "https")
	rec := httptest.NewRecorder()

	openSearchHandler(rec, req)

	body := rec.Body.String()
	searchURL := `template="https://bangs.example.com/search?q={searchTerms}"`
	if !strings.Contains(body, searchURL) {
		t.Errorf("expected body to contain '%s', got: %s", searchURL, body)
	}
}

func TestRootHandler_LinksOpenSearchDescriptor(t *testing.T) {
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()

	rootHandler(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); ct != "text/html; charset=utf-8" {
		t.Errorf("expected Content-Type 'text/html; charset=utf-8', got '%s'", ct)
	}
	link := `<link rel="search" type="application/opensearchdescription+xml" title="Bangs" href="/opensearch.xml">`
	if !strings.Contains(rec.Body.String(), link) {
		t.Errorf("expected body to contain OpenSearch link tag, got: %s", rec.Body.String())
	}
}

func TestServer_RootRoute_UnknownPath_Returns404(t *testing.T) {
	server := NewServer("nonexistent.yaml", zerolog.Nop())

	req := httptest.NewRequest(http.MethodGet, "/nonexistent", nil)
	rec := httptest.NewRecorder()

	server.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusNotFound {
		t.Errorf("expected status %d, got %d", http.StatusNotFound, rec.Code)
	}
}

func TestServer_RootRoute_ServesInstallPage(t *testing.T) {
	server := NewServer("nonexistent.yaml", zerolog.Nop())

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()

	server.Handler().ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if !strings.Contains(rec.Body.String(), "/opensearch.xml") {
		t.Errorf("expected install page to reference /opensearch.xml")
	}
}
