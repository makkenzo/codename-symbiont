FROM rust:1.86.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml ./Cargo.toml

COPY ./services/preprocessing_service/Cargo.toml ./services/preprocessing_service/Cargo.toml
COPY ./libs/shared_models/Cargo.toml ./libs/shared_models/Cargo.toml
COPY ./services/perception_service/Cargo.toml ./services/perception_service/Cargo.toml

RUN mkdir -p ./services/perception_service/src && echo "fn main() {println!(\"perception_service stub\");}" > ./services/perception_service/src/main.rs

COPY ./libs/shared_models/src ./libs/shared_models/src
COPY ./services/preprocessing_service/src ./services/preprocessing_service/src

RUN cargo build --release --package preprocessing_service

FROM debian:bookworm-20250520-slim

COPY --from=builder /usr/src/app/target/release/preprocessing_service /usr/local/bin/preprocessing_service

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/preprocessing_service"]