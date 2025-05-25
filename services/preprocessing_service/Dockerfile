FROM nvidia/cuda:12.8.0-devel-ubuntu24.04 AS builder

RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    pkg-config \
    cmake \
    libssl-dev \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain 1.86.0 \
    && rm -rf /var/lib/apt/lists/*

ENV PATH="/root/.cargo/bin:${PATH}"
ENV CUDA_COMPUTE_CAP=86

WORKDIR /usr/src/app

COPY Cargo.toml ./Cargo.toml

COPY ./services/preprocessing_service/Cargo.toml ./services/preprocessing_service/Cargo.toml

COPY ./libs/shared_models/Cargo.toml ./libs/shared_models/Cargo.toml

COPY ./services/knowledge_graph_service/Cargo.toml ./services/knowledge_graph_service/Cargo.toml
COPY ./services/text_generator_service/Cargo.toml ./services/text_generator_service/Cargo.toml
COPY ./services/api_service/Cargo.toml ./services/api_service/Cargo.toml
COPY ./services/vector_memory_service/Cargo.toml ./services/vector_memory_service/Cargo.toml
COPY ./services/perception_service/Cargo.toml ./services/perception_service/Cargo.toml

RUN mkdir -p ./services/perception_service/src && echo "fn main() { /* perception_service stub */ }" > ./services/perception_service/src/main.rs
RUN mkdir -p ./services/knowledge_graph_service/src && echo "fn main() { /* knowledge_graph_service stub */ }" > ./services/knowledge_graph_service/src/main.rs
RUN mkdir -p ./services/text_generator_service/src && echo "fn main() { /* text_generator_service stub */ }" > ./services/text_generator_service/src/main.rs
RUN mkdir -p ./services/api_service/src && echo "fn main() { /* api_service stub */ }" > ./services/api_service/src/main.rs
RUN mkdir -p ./services/vector_memory_service/src && echo "fn main() { /* vector_memory_service stub */ }" > ./services/vector_memory_service/src/main.rs

COPY ./libs/shared_models/src ./libs/shared_models/src
COPY ./services/preprocessing_service/src ./services/preprocessing_service/src

RUN cargo build --release --package preprocessing_service

FROM nvidia/cuda:12.8.0-runtime-ubuntu24.04 AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

ENV HF_HOME=/opt/hf_home
ENV HUGGINGFACE_HUB_CACHE=/opt/hf_home/hub

RUN mkdir -p /opt/hf_home/hub && chmod -R 777 /opt/hf_home

COPY --from=builder /usr/src/app/target/release/preprocessing_service /usr/local/bin/preprocessing_service

WORKDIR /app

ENTRYPOINT ["/usr/local/bin/preprocessing_service"]