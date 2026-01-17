package main

import (
	"net/http"
	"os"

	"github.com/rs/zerolog"
)

func main() {
	logger := zerolog.New(os.Stdout).With().Timestamp().Logger()

	configPath := GetEnv("BANGS_CONFIG_PATH", "./bangs.yaml")
	port := GetEnv("BANGS_PORT", "8080")

	server := NewServer(configPath, logger)

	if err := server.WatchConfig(); err != nil {
		logger.Warn().Err(err).Msg("failed to watch config file")
	}

	logger.Info().Str("port", port).Msg("starting server")
	if err := http.ListenAndServe(":"+port, server.Handler()); err != nil {
		logger.Fatal().Err(err).Msg("server failed")
	}
}
