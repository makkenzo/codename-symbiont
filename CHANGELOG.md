# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-05-23 <!-- Укажите текущую дату -->

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
