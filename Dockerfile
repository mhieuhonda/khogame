# ============================================================
# Louis Space — Multi-stage Dockerfile (Rust 1.98)
# Stage 1: builder với BuildKit cache mounts
# Stage 2: runtime debian-slim tối giản, non-root
# ============================================================

# OCI image metadata — hữu ích cho GHCR + Docker Hub + image scanners.
# docker/metadata-action (deploy.yml) tự động thêm source/revision/date
# nhưng set default ở đây để image tự mô tả được khi scan bằng
# `docker inspect` hoặc trivy/snyk.
ARG REPO="https://github.com/mhieuhonda/khogame"
ARG DESC="Louis Space — nền tảng chia sẻ game & tin tức độc lập Việt Nam (Rust/Axum)"
ARG LICENSE="MIT"

# ============================================================
# Stage 1: builder
# ============================================================
FROM rust:1.98-slim AS builder
WORKDIR /app

# Build-time deps: pkg-config cần cho ring (rustls backend) build script.
# Không cần libssl-dev/libpq-dev vì dùng rustls + sqlx pure-Rust protocol.
# Ca-certificates cho https cargo registry fetch.
# brotli (v3.6.0): sinh file .br cho static assets (precompressed serving
# — xem routes.rs ServeDir::precompressed_br). Chỉ tồn tại ở builder stage,
# không vào image runtime.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config ca-certificates brotli \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest + tài nguyên compile-time (askama templates + sqlx migrations)
# TRƯỚC để tận dụng cache dependencies — chỉ khi Cargo.toml/Cargo.lock đổi
# thì layer này mới invalidate, dependencies sẽ re-build.
COPY Cargo.toml Cargo.lock ./
COPY templates/ templates/
COPY migrations/ migrations/
# Dummy src/ để cargo build dependencies mà không cần source thật.
# `echo 'fn main() {}' > src/main.rs` hợp lệ (empty crate).
# lib.rs chỉ có comment — Rust chấp nhận file chỉ chứa comment (empty crate).
RUN mkdir -p src && \
    echo '// placeholder for dependency caching' > src/lib.rs && \
    echo 'fn main() {}' > src/main.rs

# Build dependencies (cache mounts giữ registry + target giữa các lần build).
# `|| true` cho phép dummy build fail mà vẫn cache dependencies (registry
# fetch + crate compile xong trước khi build crate khogame).
# Lưu ý: dummy build cần templates + migrations vì askama/migrate! macro
# đọc file lúc compile — thiếu sẽ fail ngay khi compile dependencies nếu
# có crate nào dùng askama macro (không phải khogame thì axum/tokio không
# dùng, nhưng để an toàn).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked || true

# Copy source thật và build lại.
# ⚠️ `cargo clean -p khogame --release` là BẮT BUỘC — nếu không, cargo sẽ
# coi fingerprint của bản dummy là còn hợp lệ (mtime ngang nhau do cache
# mount) và copy nhầm binary rỗng (303KB thay vì ~12MB).
# Sanity check cuối: binary > 2MB (catch trường hợp dummy leak qua).
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo clean -p khogame --release && \
    cargo build --release --locked && \
    cp target/release/khogame /usr/local/bin/khogame && \
    stat -c "khogame binary: %s bytes" /usr/local/bin/khogame && \
    test $(stat -c%s /usr/local/bin/khogame) -gt 2000000 || \
        (echo "!! Binary quá nhỏ — nghi là dummy build"; exit 1)

# v3.6.0 PERF — Precompress static assets (gzip -9 + brotli -q 11) cho
# ServeDir::precompressed_gzip/.precompressed_br() (routes.rs):
#   - style.css 252KB → ~38KB brotli / ~49KB gzip — client tải nhanh hơn
#     rõ, server KHÔNG tốn CPU nén runtime mỗi request.
#   - Chỉ nén css/js/svg/json (woff2 đã nén sẵn bằng brotli nội bộ — nén
#     lại vô nghĩa; PNG/JPG cũng đã nén).
#   - File gốc GIỮ NGUYÊN (-k) — client không gửi Accept-Encoding vẫn nhận
#     bản thường. File .gz/.br sinh NGAY SAU khi COPY source → luôn khớp
#     version asset trong image (không có nguy cơ stale như commit file
#     nén vào repo).
RUN cd static && \
    find . -type f \( -name "*.css" -o -name "*.js" -o -name "*.svg" -o -name "*.json" \) \
        -exec sh -c 'gzip -9 -k -c "$1" > "$1.gz"; brotli -q 11 -c "$1" > "$1.br"' _ {} \; && \
    find . -name "*.gz" -o -name "*.br" | head -20 && \
    echo "Precompressed $(find . -name '*.br' | wc -l) files brotli + $(find . -name '*.gz' | wc -l) gzip"

# ============================================================
# Stage 2: runtime (debian-slim, non-root)
# ============================================================
FROM debian:bookworm-slim AS runtime

# Build arg truyền từ outer scope (không tự động kế thừa từ stage 1).
ARG REPO
ARG DESC
ARG LICENSE

# OCI labels — `docker/metadata-action` có thể override/extend ở CI,
# nhưng set default ở đây để image tự mô tả khi scan `docker inspect`.
LABEL org.opencontainers.image.title="Louis Space (khogame)" \
      org.opencontainers.image.description="${DESC}" \
      org.opencontainers.image.source="${REPO}" \
      org.opencontainers.image.url="${REPO}" \
      org.opencontainers.image.documentation="${REPO}/blob/main/docs/DEPLOY.md" \
      org.opencontainers.image.licenses="${LICENSE}" \
      org.opencontainers.image.authors="Louis Space Team" \
      org.opencontainers.image.issues="${REPO}/issues"

# Runtime deps:
# - ca-certificates: HTTPS call (Google OAuth, GitHub API) cần root CA.
# - tzdata: Asia/Ho_Chi_Minh timezone cho TIMESTAMPTZ display + log.
# - curl: HEALTHCHECK probe /health (curl -fsS).
# --no-install-recommends: bỏ các package phụ trợ (man pages, doc) giảm 30MB.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates tzdata curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r khogame \
    && useradd -r -g khogame -s /sbin/nologin khogame

# Thư mục storage cho user upload + persistent volume (Coolify mount).
# chown trước USER directive để khogame sở hữu /app (không cần chown lại sau COPY).
RUN mkdir -p /app/static /app/templates /app/migrations /app/storage && \
    chown -R khogame:khogame /app

WORKDIR /app

# Copy binary + static assets từ builder stage.
# --chown để tránh chown layer phụ (slower image build + larger layer).
COPY --from=builder --chown=khogame:khogame \
    /usr/local/bin/khogame /app/khogame
COPY --from=builder --chown=khogame:khogame \
    /app/static /app/static
COPY --from=builder --chown=khogame:khogame \
    /app/templates /app/templates
COPY --from=builder --chown=khogame:khogame \
    /app/migrations /app/migrations

# Drop privileges — chạy với user khogame (UID tự sinh), không phải root.
# Layer này phải đứng SAU chown để file thuộc sở hữu khogame.
USER khogame

# Env defaults — có thể override qua docker run -e hoặc compose.
ENV TZ=Asia/Ho_Chi_Minh \
    RUST_LOG=khogame=info,tower_http=warn \
    STORAGE_DIR=/app/storage

# EXPOSE không publish port — chỉ là metadata cho operator biết app
# listen trên 3000. Cần `docker run -p 3000:3000` hoặc compose `ports:`.
EXPOSE 3000

# Healthcheck: dùng /health (lightweight, không chạm DB) thay vì
# /api/v1/health (detail có DB probe) để tránh:
#  - Tiêu tốn pool connection mỗi 30s (retries=3 × interval=30s).
#  - Container marked unhealthy khi DB slow → docker restart app
#    không giải quyết được vì app vẫn start được, chỉ DB chậm.
# Compose override (deploy/compose.prod.yml) có thể chọn /api/v1/health
# nếu muốn healthcheck gắn với DB.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -fsS http://127.0.0.1:3000/health || exit 1

# Exec form (json array) — tránh shell injection + signal propagation đúng
# (SIGTERM tới process khogame thay vì tới shell wrapper).
ENTRYPOINT ["/app/khogame"]
