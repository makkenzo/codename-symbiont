FROM rust:1.86.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml ./Cargo.toml

COPY ./services/perception_service/Cargo.toml ./services/perception_service/Cargo.toml

COPY ./libs/shared_models/Cargo.toml ./libs/shared_models/Cargo.toml

COPY ./services/preprocessing_service/Cargo.toml ./services/preprocessing_service/Cargo.toml
COPY ./services/knowledge_graph_service/Cargo.toml ./services/knowledge_graph_service/Cargo.toml
COPY ./services/text_generator_service/Cargo.toml ./services/text_generator_service/Cargo.toml
COPY ./services/api_service/Cargo.toml ./services/api_service/Cargo.toml
COPY ./services/vector_memory_service/Cargo.toml ./services/vector_memory_service/Cargo.toml

RUN mkdir -p ./services/preprocessing_service/src && echo "fn main() { /* preprocessing_service stub */ }" > ./services/preprocessing_service/src/main.rs
RUN mkdir -p ./services/perception_service/src && echo "fn main() { /* perception_service stub */ }" > ./services/perception_service/src/main.rs
RUN mkdir -p ./services/text_generator_service/src && echo "fn main() { /* text_generator_service stub */ }" > ./services/text_generator_service/src/main.rs
RUN mkdir -p ./services/api_service/src && echo "fn main() { /* api_service stub */ }" > ./services/api_service/src/main.rs
RUN mkdir -p ./services/vector_memory_service/src && echo "fn main() { /* vector_memory_service stub */ }" > ./services/vector_memory_service/src/main.rs

COPY ./libs/shared_models/src ./libs/shared_models/src
COPY ./services/knowledge_graph_service/src ./services/knowledge_graph_service/src

RUN cargo build --release --package knowledge_graph_service

FROM debian:bookworm-20250520-slim

COPY --from=builder /usr/src/app/target/release/knowledge_graph_service /usr/local/bin/knowledge_graph_service

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/knowledge_graph_service"]