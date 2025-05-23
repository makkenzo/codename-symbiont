FROM rust:1.86.0 AS builder

WORKDIR /usr/src/app

COPY ./services/nats_tester/Cargo.toml ./services/nats_tester/Cargo.toml
# COPY ./services/nats_tester/Cargo.lock ./services/nats_tester/Cargo.lock
COPY Cargo.toml ./Cargo.toml

COPY ./services/nats_tester/src ./services/nats_tester/src

RUN cargo build --release --package nats_tester

FROM debian:bookworm-20250520-slim

COPY --from=builder /usr/src/app/target/release/nats_tester /usr/local/bin/nats_tester

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/nats_tester"]