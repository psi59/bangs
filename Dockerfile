# Build stage
FROM golang:alpine AS builder

WORKDIR /app

# Install ca-certificates for HTTPS requests
RUN apk add --no-cache ca-certificates

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build with optimizations
# -trimpath: remove file system paths from binary
# -ldflags="-s -w": strip debug info and symbol table
# CGO_ENABLED=0: static binary
RUN CGO_ENABLED=0 GOOS=linux go build \
    -trimpath \
    -ldflags="-s -w" \
    -o bangs .

# Runtime stage
FROM alpine:3.20

WORKDIR /app

# Install ca-certificates for HTTPS requests
RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN adduser -D -g '' appuser

# Copy binary from builder
COPY --from=builder /app/bangs .

# Copy example config (user should mount their own config)
COPY --from=builder /app/bangs.example.yaml ./bangs.example.yaml

# Change ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 8080

# Environment variables
ENV BANGS_CONFIG_PATH=/app/bangs.yaml
ENV BANGS_PORT=8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:${BANGS_PORT}/health || exit 1

# Run
ENTRYPOINT ["./bangs"]
