# Codename: Symbiont

An evolving AI organism pet project.

## Concept

**Creating a "digital organism" (Symbiote) that**:

-   Lives in the system (microservice architecture on one VPS, locally via Docker Compose).
-   Analyzes unstructured data (text from the web, local files).
-   Builds an internal model of the "world" (knowledge graph in Neo4j, vector representations in Qdrant).
-   Interacts/expresses itself through the generation of new content (text).
-   Evolves over time based on data and, possibly, feedback.
-   Goal: The most unusual pet project with the most unusual and sophisticated stack.

## Project Status

**MVP 1: Data Ingestion Pipeline - COMPLETE (v0.1.0)**

The first milestone of "Codename: Symbiont" is complete. The system can currently:

1.  Accept a URL via a NATS message.
2.  Scrape the webpage for its main text content.
3.  Perform basic text preprocessing including sentence segmentation and tokenization.
4.  Store the document metadata, sentences, and extracted tokens into a Neo4j graph database.
    All components are containerized using Docker and orchestrated with Docker Compose.

## Core Tech Stack

-   **Language & Core Logic:** Rust
-   **Microservices Communication:** NATS.io
-   **Knowledge Graph:** Neo4j
-   **Vector Memory:** Qdrant
-   **UI:** Rust + WASM (Yew/Dioxus)
-   **Containerization:** Docker, Docker Compose

## Getting Started

To run the MVP 1 pipeline:

1.  **Prerequisites:**

    -   Git
    -   Docker
    -   Docker Compose
    -   `nats-cli` (optional, for sending NATS messages manually) - installation instructions [here](https://github.com/nats-io/natscli).

2.  **Clone the repository:**

    ```bash
    git clone https://github.com/makkenzo/codename-symbiont.git
    cd codename-symbiont
    ```

3.  **Environment Configuration:**

    -   Copy the example environment file: `cp .env.example .env`
    -   Edit `.env` and set your desired `NEO4J_PASSWORD`. The default `NEO4J_USER` is `neo4j`.

4.  **Build and run the services:**

    ```bash
    docker-compose up --build
    ```

    This will build the Rust services and start all containers (NATS, Neo4j, Qdrant, and the Symbiont services). Wait for all services to initialize. You should see logs indicating they are ready.

5.  **Send a task to scrape a URL:**
    Open a new terminal and use `nats-cli` to publish a message:
    ```bash
    nats pub tasks.perceive.url '{"url":"https://www.example.com"}'
    ```

## Roadmap

[Trello](https://trello.com/b/0rCkQEeu/codename-symbiont)

## License

This project is licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-APACHE.md))

-   MIT license ([LICENSE-MIT](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-MIT.md))

at your option.
