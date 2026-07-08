FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p oxiderp-core

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 oxiderp
WORKDIR /app
COPY --from=builder /app/target/release/oxiderp-core /usr/local/bin/oxiderp-core
USER oxiderp
ENV RUST_LOG=info
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 CMD ["/usr/local/bin/oxiderp-core", "--healthcheck"]
CMD ["/usr/local/bin/oxiderp-core"]
