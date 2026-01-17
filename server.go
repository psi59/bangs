package main

import (
	"fmt"
	"net/http"
	"os"
	"sync/atomic"

	"github.com/fsnotify/fsnotify"
	"github.com/rs/zerolog"
)

type Server struct {
	repo       atomic.Pointer[Repository]
	parser     *Parser
	logger     zerolog.Logger
	configPath string
}

func NewServer(configPath string, logger zerolog.Logger) *Server {
	s := &Server{
		parser:     NewParser(),
		logger:     logger,
		configPath: configPath,
	}
	s.loadConfig()
	return s
}

func (s *Server) loadConfig() {
	config, err := LoadConfigFromFile(s.configPath)
	if err != nil {
		s.logger.Error().Err(err).Msg("failed to load config")
		s.repo.Store(nil)
		return
	}
	repo := config.ToRepository()
	s.repo.Store(repo)
	s.logger.Info().Str("path", s.configPath).Msg("config loaded")
}

func (s *Server) WatchConfig() error {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return fmt.Errorf("failed to create watcher: %w", err)
	}

	go func() {
		for {
			select {
			case event, ok := <-watcher.Events:
				if !ok {
					return
				}
				if event.Has(fsnotify.Write) {
					s.logger.Info().Str("path", s.configPath).Msg("config file changed, reloading")
					s.loadConfig()
				}
			case err, ok := <-watcher.Errors:
				if !ok {
					return
				}
				s.logger.Error().Err(err).Msg("watcher error")
			}
		}
	}()

	if err := watcher.Add(s.configPath); err != nil {
		return fmt.Errorf("failed to watch config file: %w", err)
	}
	return nil
}

func (s *Server) Handler() http.Handler {
	searchHandler := &dynamicSearchHandler{server: s}
	handler := http.NewServeMux()
	handler.Handle("/search", searchHandler)
	handler.HandleFunc("/health", s.healthHandler)

	return RecoverMiddleware(LoggingMiddleware(s.logger)(handler))
}

func (s *Server) healthHandler(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	status := "ok"
	if s.repo.Load() == nil {
		status = "degraded"
		w.WriteHeader(http.StatusServiceUnavailable)
	}
	fmt.Fprintf(w, `{"status":"%s"}`, status)
}

type dynamicSearchHandler struct {
	server *Server
}

func (h *dynamicSearchHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	repo := h.server.repo.Load()
	searchHandler := NewSearchHandler(repo, h.server.parser)
	searchHandler.ServeHTTP(w, r)
}

func GetEnv(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}
