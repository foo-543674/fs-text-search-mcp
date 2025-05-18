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

RUN groupadd -r appuser && useradd -r -g appuser appuser

COPY --from=builder /usr/src/app/target/release/fs-text-search-mcp /usr/local/bin

USER appuser
CMD ["fs-text-search-mcp"]