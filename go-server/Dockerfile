# Multi-stage build for Go BlueSky MCP Server
FROM golang:1.21-alpine AS builder

# Set up build environment
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download

# Copy source code and build
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o bluesky-mcp ./cmd/bluesky-mcp

# Final stage - minimal image
FROM alpine:latest

# Install ca-certificates for HTTPS requests
RUN apk --no-cache add ca-certificates

# Create non-root user
RUN adduser -D -s /bin/sh appuser

# Set up application directory
WORKDIR /app
COPY --from=builder /app/bluesky-mcp .
RUN chown appuser:appuser bluesky-mcp

# Switch to non-root user
USER appuser

# Expose default port (though this server primarily uses stdio)
EXPOSE 8080

# Default command
CMD ["./bluesky-mcp"]