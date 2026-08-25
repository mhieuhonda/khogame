# ============================================================
# Kho Game - Multi-stage Dockerfile (Rust 1.98)
# Stage 1: builder với BuildKit cache mounts
# Stage 2: runtime debian-slim tối giản, non-root
# ============================================================

FROM rust:1.98-slim AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends pkg-config && rm -rf /var/lib/apt/lists/*

# Copy manifest + tài nguyên compile-time (askama templates + sqlx migrations)
# trước để tận dụng cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY templates/ templates/
COPY migrations/ migrations/
RUN mkdir -p src && echo '// placeholder for dependency caching' > src/lib.rs && echo 'fn main() {}' > src/main.rs

# Build dependencies (cache mounts giữ registry + target giữa các lần build)
# Lưu ý: dummy build cần templates + migrations vì askama/migrate! macro đọc lúc compile
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked || true

# Copy source thật và build lại
# ⚠️ cargo clean -p khogame: bắt buộc rebuild crate chính — nếu không cargo sẽ
# coi fingerprint của bản dummy là còn hợp lệ (mtime ngang nhau) và copy nhầm binary rỗng
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo clean -p khogame --release && \
    cargo build --release --locked && \
    cp target/release/khogame /usr/local/bin/khogame && \
    stat -c "khogame binary: %s bytes" /usr/local/bin/khogame && \
    test $(stat -c%s /usr/local/bin/khogame) -gt 2000000 || (echo "!! Binary quá nhỏ — nghi là dummy build"; exit 1)

# ============ Runtime ============
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tzdata curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r khogame && useradd -r -g khogame khogame

# Thư mục storage trên VPS (gắn persistent volume qua Coolify)
RUN mkdir -p /app/static /app/templates /app/migrations /app/storage && chown -R khogame:khogame /app

WORKDIR /app
COPY --from=builder /usr/local/bin/khogame /app/khogame
COPY --from=builder /app/static /app/static
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/migrations /app/migrations

USER khogame
ENV TZ=Asia/Ho_Chi_Minh \
    RUST_LOG=khogame=info,tower_http=warn \
    STORAGE_DIR=/app/storage

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:3000/api/v1/health || exit 1

ENTRYPOINT ["/app/khogame"]
