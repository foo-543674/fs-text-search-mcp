FROM rust:slim-bullseye AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
  echo "fn main() {}" > src/main.rs && \
  cargo build --release && \
  rm -rf src

COPY src ./src
RUN cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    gosu \
    && rm -rf /var/lib/apt/lists/* \
    && gosu nobody true

ENV PUID=1000 \
    PGID=1000

COPY entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/entrypoint.sh

COPY --from=builder /usr/src/app/target/release/fs-text-search-mcp /usr/local/bin

RUN mkdir -p /home

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]