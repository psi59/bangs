package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestNewSearchHandler(t *testing.T) {
	repo := NewRepository()
	parser := NewParser()

	handler := NewSearchHandler(repo, parser)

	if handler == nil {
		t.Error("expected handler to be non-nil")
	}
}

func TestSearchHandler_BangSearch_Redirect(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=!g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=hello"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestSearchHandler_NoBang_UsesDefault(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello+world", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=hello+world"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestSearchHandler_NonexistentBang_UsesDefault(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=!nonexistent+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=%21nonexistent+hello"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestSearchHandler_EmptyQuery_BadRequest(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, rec.Code)
	}
}

func TestSearchHandler_MissingQuery_BadRequest(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, rec.Code)
	}
}

func TestSearchHandler_NilRepository_ServiceUnavailable(t *testing.T) {
	parser := NewParser()
	handler := NewSearchHandler(nil, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Errorf("expected status %d, got %d", http.StatusServiceUnavailable, rec.Code)
	}
	contentType := rec.Header().Get("Content-Type")
	if !strings.Contains(contentType, "text/html") {
		t.Errorf("expected Content-Type to contain 'text/html', got '%s'", contentType)
	}
	body := rec.Body.String()
	if !strings.Contains(body, "<html") {
		t.Error("expected HTML response body")
	}
	if !strings.Contains(body, "503") {
		t.Error("expected error code in response body")
	}
}

func TestSearchHandler_NoDefaultBang_ServiceUnavailable(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	// default bang is NOT set
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Errorf("expected status %d, got %d", http.StatusServiceUnavailable, rec.Code)
	}
	contentType := rec.Header().Get("Content-Type")
	if !strings.Contains(contentType, "text/html") {
		t.Errorf("expected Content-Type to contain 'text/html', got '%s'", contentType)
	}
}

func TestSearchHandler_BangWithoutExclamation(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	// "g hello" without "!" should work if "g" is a registered bang
	req := httptest.NewRequest(http.MethodGet, "/search?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=hello"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestSearchHandler_BangWithoutExclamation_NotRegistered(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	// "foo hello" - "foo" is not a registered bang, so search "foo hello"
	req := httptest.NewRequest(http.MethodGet, "/search?q=foo+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=foo+hello"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestSearchHandler_BangWithoutExclamation_OnlyTrigger(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("yt", "YouTube", "https://www.youtube.com/results?search_query={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("yt")
	parser := NewParser()
	handler := NewSearchHandler(repo, parser)

	// "yt" alone (without "!") should redirect to YouTube home
	req := httptest.NewRequest(http.MethodGet, "/search?q=yt", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusPermanentRedirect {
		t.Errorf("expected status %d, got %d", http.StatusPermanentRedirect, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.youtube.com"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}
