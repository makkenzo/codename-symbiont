FROM rust:1.86.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml ./Cargo.toml

COPY ./services/perception_service/Cargo.toml ./services/perception_service/Cargo.toml
COPY ./libs/shared_models/Cargo.toml ./libs/shared_models/Cargo.toml
COPY ./services/preprocessing_service/Cargo.toml ./services/preprocessing_service/Cargo.toml
COPY ./services/knowledge_graph_service/Cargo.toml ./services/knowledge_graph_service/Cargo.toml

RUN mkdir -p ./services/preprocessing_service/src && echo "fn main() { /* preprocessing_service stub */ }" > ./services/preprocessing_service/src/main.rs
RUN mkdir -p ./services/knowledge_graph_service/src && echo "fn main() { /* knowledge_graph_service stub */ }" > ./services/knowledge_graph_service/src/main.rs

COPY ./libs/shared_models/src ./libs/shared_models/src
COPY ./services/perception_service/src ./services/perception_service/src

RUN cargo build --release --package perception_service

FROM debian:bookworm-20250520-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/perception_service /usr/local/bin/perception_service

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/perception_service"]