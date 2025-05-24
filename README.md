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

The project has achieved its initial milestone and is now expanding its capabilities:

**MVP 1: Data Ingestion Pipeline - COMPLETE (v0.1.0)**

The first _initial_ milestone of "Codename: Symbiont" is complete. The system can currently:

1.  Accept a URL via a NATS message.
2.  Scrape the webpage for its main text content.
3.  Perform basic text preprocessing including sentence segmentation and tokenization.
4.  Store the document metadata, sentences, and extracted tokens into a Neo4j graph database.
    All components are containerized using Docker and orchestrated with Docker Compose.

**Text Generation Capabilities (Ongoing Development - v0.2.0 Unreleased)**

The Symbiont is beginning to express itself:

-   **Text Generation:** The system can now generate novel text.
-   **`text_generator_service`:** A new dedicated service, `text_generator_service`, is responsible for this capability.
-   **Markov Chain Model:** Currently, generation is based on a simple fixed Markov chain model built from pre-processed text data.
-   **API Integration:**
    -   An `api_service` provides an HTTP POST endpoint at `/api/generate-text` to trigger text generation.
    -   Generated text segments are streamed back to the client via Server-Sent Events (SSE) on the `/api/events` endpoint.

## Core Tech Stack

-   **Language & Core Logic:** Rust
-   **Microservices Communication:** NATS.io
-   **Knowledge Graph:** Neo4j
-   **Vector Memory:** Qdrant
-   **UI:** Next.js (React/TypeScript) with Tailwind CSS
-   **Containerization:** Docker, Docker Compose

## Getting Started

Running the Symbiont System:

1.  **Prerequisites:**

    -   Git
    -   Docker
    -   Docker Compose
    -   `curl` (for interacting with the API service)
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

5.  **Interacting with the System:**

    Once the system is running, you can interact with it in several ways:

    -   **Submitting URLs for Processing:**
        This is part of the data ingestion pipeline. You can submit URLs via:

        -   **NATS (CLI):** Open a new terminal and use `nats-cli`:
            ```bash
            nats pub tasks.perceive.url '{"url":"https://www.example.com"}'
            ```
        -   **HTTP API:** The `api_service` also exposes an endpoint for this at `POST /api/submit-url`.

    -   **Generating Text:**
        Send a POST request to the `api_service` to trigger text generation. By default, the `api_service` listens on port 8080.

        **Endpoint:** `POST http://localhost:8080/api/generate-text`

        **Payload Structure (`GenerateTextTask`):**

        ```json
        {
            "task_id": "your-unique-task-id", // Should be a UUID
            "prompt": "An optional prompt for the text generator", // Optional
            "max_length": 50 // Max length of generated text
        }
        ```

        **`curl` Example:**
        The `uuidgen` command can be used to generate a unique task ID. If `uuidgen` is not available on your system, replace `$(uuidgen)` with any unique string.

        ```bash
        curl -X POST -H "Content-Type: application/json" \
             -d '{"task_id":"$(uuidgen)","prompt":"Hello world","max_length":30}' \
             http://localhost:8080/api/generate-text
        ```

        The response to this POST request will confirm that the task has been submitted.

    -   **Receiving Generated Text via SSE:**
        Generated text segments are streamed back to clients via Server-Sent Events (SSE).

        **Endpoint:** `GET http://localhost:8080/api/events`

        **`curl` Example to connect to the SSE stream:**

        ```bash
        curl -N http://localhost:8080/api/events
        ```

        You will see a stream of events. Each event is a JSON object representing a `GeneratedTextMessage`:

        ```
        data: {"original_task_id":"your-unique-task-id","generated_text":"Generated sample text...","timestamp_ms":1678886400000}
        ```

        (Expect multiple such `data:` lines as text is generated.)

## Roadmap

[Trello](https://trello.com/b/0rCkQEeu/codename-symbiont)

The project continues to evolve. Recent developments include the introduction of text generation capabilities (see 'Project Status' above). Future work will focus on enhancing these generative models and further expanding the Symbiont's understanding of ingested data.

## License

This project is licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-APACHE.md))

-   MIT license ([LICENSE-MIT](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-MIT.md))

at your option.
