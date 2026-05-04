# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.79-slim AS builder

WORKDIR /app

# Install system dependencies for chromiumoxide (headless Chrome)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests first for layer caching
COPY backend/Cargo.toml backend/Cargo.lock* ./backend/

# Create a dummy main.rs so cargo can fetch and compile dependencies
RUN mkdir -p backend/src && echo "fn main() {}" > backend/src/main.rs
WORKDIR /app/backend
RUN cargo build --release 2>/dev/null || true

# Now copy the real source
COPY backend/src ./src
COPY backend/templates ./templates

# Touch main.rs so cargo knows to recompile
RUN touch src/main.rs && cargo build --release

# ── Runtime stage ──────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install Chromium and runtime deps
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-liberation \
    libgbm1 \
    libnss3 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libcups2 \
    libdrm2 \
    libxkbcommon0 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libglib2.0-0 \
    libnspr4 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Point chromiumoxide to system Chromium
ENV CHROME_PATH=/usr/bin/chromium

WORKDIR /app

COPY --from=builder /app/backend/target/release/clrdbt ./clrdbt
COPY frontend ./frontend

EXPOSE 8080

CMD ["./clrdbt"]
