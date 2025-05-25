# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 25-05-2025

### Added

-   **MVP 3: Advanced NLP (Embedding Generation) & Semantic Search Integration**
    -   **`preprocessing_service` (Enhancement):**
        -   Integrated the `candle` ML framework for Rust.
        -   Implemented `embedding_generator.rs` for generating sentence embeddings.
        -   Utilizes the `sentence-transformers/paraphrase-multilingual-mpnet-base-v2` model (from Hugging Face Hub) with CUDA support (if available) and CPU fallback.
        -   The service now subscribes to `data.raw_text.discovered`, segments text into sentences, and generates embeddings for them.
        -   Publishes `TextWithEmbeddingsMessage` (containing original ID, URL, embedding data, model name) to the `data.text.with_embeddings` NATS subject.
    -   **`vector_memory_service` (New Service):**
        -   Created a new Rust service for managing vector memory.
        -   Connects to Qdrant.
        -   On startup, checks for the existence of the `symbiont_document_embeddings` collection (vector dimension 768, Cosine metric) and creates it if absent.
        -   Subscribes to the `data.text.with_embeddings` NATS subject.
        -   Stores received sentence embeddings and their metadata (document ID, URL, sentence text, order, model name, processing timestamp) in Qdrant.
        -   Implemented a NATS handler (`tasks.search.semantic.request`) that accepts a query vector and `top_k`, performs a semantic search in Qdrant, and returns results via NATS request-reply.
        -   Added a Dockerfile for the service.
    -   **`api_service` (Enhancement):**
        -   Added a new HTTP endpoint: `POST /api/search/semantic`.
        -   Implemented semantic search orchestration logic:
            1.  Receives a query text and `top_k` from the client.
            2.  Sends the query text to `preprocessing_service` (via NATS request-reply on `tasks.embedding.for_query`) to obtain an embedding.
            3.  Sends the received embedding and `top_k` to `vector_memory_service` (via NATS request-reply on `tasks.search.semantic.request`) to perform the search.
            4.  Returns the search results (list of `SemanticSearchResultItem`) to the client.
        -   Implemented timeout and error handling for NATS interactions with other services.
    -   **`frontend` (Enhancement):**
        -   Added a new section to the UI for semantic search.
        -   Users can input a text query and specify the desired number of results (`top_k`).
        -   Implemented request submission to the `/api/search/semantic` endpoint.
        -   Semantic search results (found sentence text, source URL, similarity score, metadata) are displayed in the UI.
        -   Added new TypeScript interfaces for typing semantic search data.
    -   **`shared_models` (Enhancement):**
        -   Added new Rust structs to support semantic search functionality:
            -   `SemanticSearchApiRequest` (HTTP request to `api_service`)
            -   `QueryForEmbeddingTask` (NATS: `api_service` -> `preprocessing_service`)
            -   `QueryEmbeddingResult` (NATS: `preprocessing_service` -> `api_service`)
            -   `QdrantPointPayload` (payload structure for Qdrant points)
            -   `SemanticSearchNatsTask` (NATS: `api_service` -> `vector_memory_service`)
            -   `SemanticSearchResultItem` (search result item)
            -   `SemanticSearchNatsResult` (NATS: `vector_memory_service` -> `api_service`)
            -   `SemanticSearchApiResponse` (HTTP response from `api_service`)
    -   **Docker & Orchestration:**
        -   Updated `docker-compose.yml` to include the `vector_memory_service`.
        -   Adjusted Dockerfiles for `preprocessing_service` (e.g., CUDA dependencies if GPU is used, `HF_HOME` volume) and other services as needed.

### Changed

-   **`preprocessing_service`:** The primary function has shifted from simple tokenization to embedding generation. The old logic for publishing `TokenizedTextMessage` (and its consumption by `knowledge_graph_service`) remains for now, but the main new output is `TextWithEmbeddingsMessage`.
    _(Note: Future work will need to address how `knowledge_graph_service` integrates with these new embeddings or if the services' responsibilities need further separation regarding text processing stages.)_

### Fixed

-   Improved error and timeout handling for NATS request-reply patterns in `api_service`.

## [0.2.0] - 24-05-2025

### Added

-   **MVP 2: Basic Text Generation & UI**
    -   `text_generator_service`:
        -   Created service skeleton and NATS subscription for `GenerateTextTask`.
        -   Implemented a basic Markov chain model for text generation (trained on fixed text).
        -   Implemented publishing of `GeneratedTextMessage` to NATS.
        -   Added Dockerfile.
    -   `api_service` (evolved from `ui_service` backend concept):
        -   Created Rust HTTP API service using Actix Web.
        -   Endpoint `/api/submit-url` to receive URLs and publish `PerceiveUrlTask` to NATS.
        -   Endpoint `/api/generate-text` to receive `GenerateTextTask` and publish to NATS.
        -   SSE endpoint `/api/events` to stream `GeneratedTextMessage` events from NATS to UI clients (using `tokio::sync::broadcast`).
        -   Added Dockerfile.
        -   Configured CORS to allow requests from the frontend.
    -   `frontend` (Next.js UI):
        -   Initialized Next.js (TypeScript, Tailwind CSS, App Router) project in `frontend/` directory.
        -   Implemented UI components for submitting URLs and requesting text generation.
        -   Integrated with `api_service` HTTP endpoints (`/api/submit-url`, `/api/generate-text`).
        -   Implemented real-time display of generated text using Server-Sent Events from `/api/events`.
        -   Added Dockerfile for Next.js application (with `output: 'standalone'`).
        -   Configured `NEXT_PUBLIC_API_URL` via build arguments and environment variables for Docker.
    -   `shared_models`: Added `GenerateTextTask` and `GeneratedTextMessage` structures.
    -   Updated `docker-compose.yml` to include `text_generator_service`, `api_service`, and `frontend` services with appropriate configurations, dependencies, and port mappings.
    -   Enhanced `docker-compose.yml` with healthchecks for NATS and Neo4j, and conditional dependencies (`service_healthy`).

### Changed

-   Refactored `ui_service` concept into a dedicated `api_service` (Rust backend) and a separate `frontend` (Next.js) application.
-   Improved Docker build process for Rust workspace services to correctly handle manifests and stubs for all workspace members.
-   Standardized NATS URL and API server port configurations via `.env` file and Docker Compose environment variables.

### Fixed

-   Resolved CORS issues preventing Next.js frontend from communicating with the Rust `api_service`.
-   Corrected Docker build errors related to `NEXT_PUBLIC_API_URL` in Next.js frontend by using build arguments.
-   Addressed various NATS and `tokenizers` API usage issues during service development.
-   Fixed Neo4j connection timing issuesbeliefs by `knowledge_graph_service` by ensuring Neo4j is healthy (though a retry for schema creation is still a good TODO).

## [0.1.0] - 23-05-2025

### Added

-   **MVP 1: Data Ingestion Pipeline**
    -   `perception_service`:
        -   Created service skeleton and NATS subscription for URL tasks.
        -   Implemented web scraping functionality using `reqwest` and `scraper`.
        -   Implemented publishing of `RawTextMessage` to NATS.
        -   Added Dockerfile.
    -   `preprocessing_service`:
        -   Created service skeleton and NATS subscription for `RawTextMessage`.
        -   Implemented basic text cleaning, sentence segmentation (simple).
        -   Implemented tokenization using `tokenizers` crate (whitespace pre-tokenizer).
        -   Implemented publishing of `TokenizedTextMessage` to NATS.
        -   Added Dockerfile.
    -   `knowledge_graph_service`:
        -   Created service skeleton and NATS subscription for `TokenizedTextMessage`.
        -   Implemented Neo4j connection setup and basic schema (constraints/indexes).
        -   Implemented saving of `Document`, `Sentence`, and `Token` nodes to Neo4j (simplified token-document linkage for MVP1).
        -   Added Dockerfile.
    -   `shared_models` library crate for common NATS message structures (`PerceiveUrlTask`, `RawTextMessage`, `TokenizedTextMessage`).
    -   Docker Compose setup for all three Rust services, NATS, Neo4j, and Qdrant.
    -   Implemented structured logging (`log` + `env_logger`) for Rust services.
    -   Initial project structure, Git repository, `README.md`, and `CHANGELOG.md`.
    -   `.env` and `.env.example` mechanism for managing secrets (e.g., Neo4j password).

### Changed

-   Improved Dockerfile structure for Rust workspace members to correctly handle dependencies and stubs during builds.
-   Refined NATS message handling in services to use `tokio::spawn` for non-blocking processing.
-   Switched Neo4j parameter handling in `knowledge_graph_service` to use `HashMap<String, BoltType>` for `neo4rs v0.7.x`.

### Fixed

-   Resolved Docker build issues related to workspace member manifest loading and target specification.
-   Addressed NATS API usage inconsistencies and error handling in services.
-   Corrected `scraper` API usage for text extraction.
