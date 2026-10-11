# Build stage
FROM rust:1-alpine AS builder

WORKDIR /app

# musl-dev: C toolchain support for ring and mimalloc
RUN apk add --no-cache musl-dev

# Build dependencies first so they are cached across source changes
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release --locked && \
    rm -rf src

# Copy source code and build a static release binary
COPY favicon.png ./
COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

# Runtime stage
FROM alpine:3.20

WORKDIR /app

# Install ca-certificates for HTTPS requests
RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN adduser -D -g '' appuser

# Copy binary from builder
COPY --from=builder /app/target/release/bangs .

# Copy example config (user should mount their own config)
COPY bangs.example.yaml ./bangs.example.yaml

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
