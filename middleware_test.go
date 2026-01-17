package main

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/rs/zerolog"
)

func TestRecoverMiddleware_PanicRecovery(t *testing.T) {
	panicHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		panic("test panic")
	})

	handler := RecoverMiddleware(panicHandler)

	req := httptest.NewRequest(http.MethodGet, "/", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusInternalServerError {
		t.Errorf("expected status %d, got %d", http.StatusInternalServerError, rec.Code)
	}
}

func TestLoggingMiddleware_LogsRequest(t *testing.T) {
	var buf bytes.Buffer
	logger := zerolog.New(&buf)

	testHandler := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})

	handler := LoggingMiddleware(logger)(testHandler)

	req := httptest.NewRequest(http.MethodGet, "/search?q=hello", nil)
	rec := httptest.NewRecorder()

	handler.ServeHTTP(rec, req)

	logOutput := buf.String()
	if !strings.Contains(logOutput, "GET") {
		t.Errorf("expected log to contain 'GET', got: %s", logOutput)
	}
	if !strings.Contains(logOutput, "/search") {
		t.Errorf("expected log to contain '/search', got: %s", logOutput)
	}
}
