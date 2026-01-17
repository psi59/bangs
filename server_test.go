package main

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/rs/zerolog"
)

func TestNewServer(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "bangs.yaml")
	content := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test config: %v", err)
	}

	logger := zerolog.Nop()
	server := NewServer(configPath, logger)

	if server == nil {
		t.Fatal("expected server to be non-nil")
	}
}

func TestServer_Handler_BangSearch(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "bangs.yaml")
	content := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test config: %v", err)
	}

	logger := zerolog.Nop()
	server := NewServer(configPath, logger)
	handler := server.Handler()

	req := httptest.NewRequest(http.MethodGet, "/search?q=g+hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusFound {
		t.Errorf("expected status %d, got %d", http.StatusFound, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=hello"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestServer_Handler_DefaultSearch(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "bangs.yaml")
	content := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test config: %v", err)
	}

	logger := zerolog.Nop()
	server := NewServer(configPath, logger)
	handler := server.Handler()

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello+world", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusFound {
		t.Errorf("expected status %d, got %d", http.StatusFound, rec.Code)
	}
	location := rec.Header().Get("Location")
	expected := "https://www.google.com/search?q=hello+world"
	if location != expected {
		t.Errorf("expected Location '%s', got '%s'", expected, location)
	}
}

func TestServer_Handler_ConfigNotFound(t *testing.T) {
	logger := zerolog.Nop()
	server := NewServer("/nonexistent/bangs.yaml", logger)
	handler := server.Handler()

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Errorf("expected status %d, got %d", http.StatusServiceUnavailable, rec.Code)
	}
}

func TestGetEnv(t *testing.T) {
	t.Run("returns environment variable when set", func(t *testing.T) {
		os.Setenv("TEST_VAR", "test_value")
		defer os.Unsetenv("TEST_VAR")

		result := GetEnv("TEST_VAR", "default")
		if result != "test_value" {
			t.Errorf("expected 'test_value', got '%s'", result)
		}
	})

	t.Run("returns default when not set", func(t *testing.T) {
		result := GetEnv("NONEXISTENT_VAR", "default_value")
		if result != "default_value" {
			t.Errorf("expected 'default_value', got '%s'", result)
		}
	})
}

func TestServer_Health_OK(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "bangs.yaml")
	content := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test config: %v", err)
	}

	logger := zerolog.Nop()
	server := NewServer(configPath, logger)
	handler := server.Handler()

	req := httptest.NewRequest(http.MethodGet, "/health", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, rec.Code)
	}
	if !strings.Contains(rec.Body.String(), `"status":"ok"`) {
		t.Errorf("expected status ok, got %s", rec.Body.String())
	}
}

func TestServer_Health_Degraded(t *testing.T) {
	logger := zerolog.Nop()
	server := NewServer("/nonexistent/bangs.yaml", logger)
	handler := server.Handler()

	req := httptest.NewRequest(http.MethodGet, "/health", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusServiceUnavailable {
		t.Errorf("expected status %d, got %d", http.StatusServiceUnavailable, rec.Code)
	}
	if !strings.Contains(rec.Body.String(), `"status":"degraded"`) {
		t.Errorf("expected status degraded, got %s", rec.Body.String())
	}
}
