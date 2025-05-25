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

The project is actively under development and has achieved key milestones:

**MVP 3: Advanced NLP & Vector Search Integration - COMPLETE (v0.3.0)**

This milestone significantly enhances the Symbiont's understanding and retrieval capabilities by:

1.  **Embedding Generation (`preprocessing_service`):** Text is now processed into sentence embeddings using the `candle` ML framework (with a `sentence-transformers` model). This allows for understanding semantic similarity.
2.  **Vector Storage (`vector_memory_service`):** A new service stores these embeddings and associated metadata in a Qdrant vector database.
3.  **Semantic Search (API & UI):**
    -   The `api_service` now orchestrates a semantic search pipeline, converting user queries into embeddings and querying the `vector_memory_service`.
    -   The **Web UI (`frontend`)** features a new section allowing users to perform semantic searches over the ingested and processed data, viewing relevant text snippets, their sources, and similarity scores.

**MVP 2: Basic Text Generation & UI - COMPLETE (v0.2.0)**

This milestone introduces the Symbiont's ability to generate text and interact with users via a web interface. Key capabilities include:

1.  **Web UI (Next.js):** Users can submit URLs for processing, request text generation (with optional prompt and max length), and view status messages and generated text in real-time via Server-Sent Events (SSE).
2.  **API Service (Rust/Actix Web):** Provides HTTP endpoints for the UI to submit tasks and an SSE endpoint to stream generated text. It acts as a gateway to the NATS messaging system.
3.  **Text Generation Service (Rust):** Generates text based on a simple fixed Markov chain model. Receives tasks and publishes results via NATS.
4.  **Integration:** All new services (`api_service`, `text_generator_service`, `frontend`) are containerized and orchestrated with Docker Compose.

**MVP 1: Data Ingestion Pipeline - COMPLETE (v0.1.0)**

The first _initial_ milestone of "Codename: Symbiont" is complete. The system can currently:

1.  Accept a URL via a NATS message.
2.  Scrape the webpage for its main text content.
3.  Perform basic text preprocessing including sentence segmentation and tokenization.
4.  Store the document metadata, sentences, and extracted tokens into a Neo4j graph database.

All components are containerized using Docker and orchestrated with Docker Compose.

## Core Tech Stack

-   **Backend & Core Logic:** Rust
    -   Microservices: Actix Web (for API), Tokio, Async-NATS
    -   Data Processing: `tokenizers` crate, `candle` (for ML inference/embeddings)
    -   Text Generation (MVP2): Basic Markov Chains
-   **Microservices Communication:** NATS.io
-   **Databases:**
    -   Knowledge Graph: Neo4j
    -   Vector Memory: Qdrant (using `qdrant-client`)
-   **UI:** Next.js (React/TypeScript) with Tailwind CSS
-   **Containerization:** Docker, Docker Compose
-   **Logging:** `log` + `env_logger` for Rust services.

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
    -   Edit `.env` and set your desired `NEO4J_PASSWORD` (default user is `neo4j`).
    -   You can also configure `API_SERVER_PORT` (defaults to 8080, for the API service) and `FRONTEND_PORT` (defaults to 3000, for the Web UI).
    -   The `.env` file also defines:
        -   `NEXT_PUBLIC_API_URL_FOR_FRONTEND_BUILD` (e.g., `http://localhost:${API_SERVER_PORT}/api`): This URL is embedded into the frontend during its build process to allow it to communicate with the API service.
        -   `NEXT_PUBLIC_API_URL_FOR_FRONTEND_RUNTIME` (e.g., `http://cs-api-service:${API_SERVER_INTERNAL_PORT}/api`): This URL is used by the running frontend container to communicate with the API service container. Users typically do not need to change this, as it's for internal Docker network communication and relies on `API_SERVER_INTERNAL_PORT`.

4.  **Build and run the services:**

    ```bash
    docker-compose up --build
    ```

    This will build the Rust services and start all containers (NATS, Neo4j, Qdrant, the Symbiont services, and the frontend service). Wait for all services to initialize. You should see logs indicating they are ready. A web interface will be available.

5.  **Interacting with the System:**

    Once the system is running, you can interact with it in several ways:

    -   **Web UI:**
        The primary way to interact with the Symbiont system is now through its web interface.
        It is typically accessible at `http://localhost:3000` (or the `FRONTEND_PORT` you've configured in the `.env` file).

        The Web UI provides the following functionalities:

        -   **Submit URLs:** Input URLs for the system to scrape, process, and add to its knowledge graph.
        -   **Generate Text:** Initiate text generation. You can provide an optional prompt and specify the maximum length of the desired text.
        -   **Real-time Updates:** View status messages and the generated text output, which updates in real-time via Server-Sent Events (SSE) from the `api_service`.
        -   **Semantic Search:** Perform searches based on semantic similarity. Input a query, and the system will find the most relevant text snippets from the data it has processed. Results include the text, source URL, and similarity score.

    -   **Submitting URLs for Processing:**
        (Note: This action can also be performed via the Web UI. The methods below detail API/CLI interactions, suitable for advanced users or scripting.)

        This is part of the data ingestion pipeline. You can submit URLs via:

        -   **NATS (CLI):** Open a new terminal and use `nats-cli`:
            ```bash
            nats pub tasks.perceive.url '{"url":"https://www.example.com"}'
            ```
        -   **HTTP API:** The `api_service` also exposes an endpoint for this at `POST /api/submit-url`.

    -   **Generating Text:**
        (Note: This action, including receiving generated text via SSE, can also be performed via the Web UI. The methods below detail API/CLI interactions, suitable for advanced users or scripting.)

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

Track my progress and upcoming features on [Trello](https://trello.com/b/0rCkQEeu/codename-symbiont).

-   ✅ **MVP 1: Data Ingestion & Storage Pipeline (v0.1.0)**
-   ✅ **MVP 2: Basic Text Generation & UI (v0.2.0)**
-   ✅ **MVP 3: Advanced NLP & Vector Search Integration (v0.3.0)**
    -   Integrated `candle` for ML model inference (sentence embeddings).
    -   Utilize Qdrant for storing and querying text embeddings (`vector_memory_service`).
    -   Implemented semantic search API and UI features.
-   ➡️ **MVP 4 (Planned/In Progress): Enhanced Generation & Knowledge Integration**
    -   Improve Markov chain model by training on data from Neo4j/Qdrant context.
    -   **OR** Explore advanced text generation with `candle` (e.g., smaller GPT-2 or other generative models).
    -   Integrate knowledge graph data more deeply with search and generation (e.g., RAG-like patterns).
    -   Refine text processing pipeline (e.g., better sentence segmentation, NER).
-   ... (Further ideas for evolution)

The project continues to evolve. Recent developments include the introduction of text generation capabilities (see 'Project Status' above). Future work will focus on enhancing these generative models and further expanding the Symbiont's understanding of ingested data.

## License

This project is licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-APACHE.md))

-   MIT license ([LICENSE-MIT](https://github.com/makkenzo/codename-symbiont/blob/master/LICENSE-MIT.md))

at your option.
