# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

-   **MVP 3 (Planned): Advanced NLP & Vector Search Integration**
    -   Integration with `candle` for ML model inference (e.g., sentence embeddings).
    -   `vector_memory_service` to store and query text embeddings using Qdrant.
    -   API endpoint for semantic search.
    -   UI integration for semantic search.

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
