use actix_cors::Cors;
use actix_web::{App, Error as ActixError, HttpResponse, HttpServer, Responder, http::header, web};
use actix_web_lab::sse::{Data as SseData, Event as SseEvent, Sse};
use async_nats::Client as NatsClient;
use futures::StreamExt;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use shared_models::{
    GenerateTextTask, GeneratedTextMessage, PerceiveUrlTask, QueryEmbeddingResult,
    QueryForEmbeddingTask, SemanticSearchApiRequest, SemanticSearchApiResponse,
    SemanticSearchNatsResult, SemanticSearchNatsTask,
};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use uuid::Uuid;

const PERCEPTION_URL_TASK_SUBJECT: &str = "tasks.perceive.url";
const GENERATE_TEXT_TASK_SUBJECT: &str = "tasks.generation.text";
const TEXT_GENERATED_EVENT_SUBJECT: &str = "events.text.generated";
const EMBEDDING_FOR_QUERY_NATS_SUBJECT: &str = "tasks.embedding.for_query";
const SEMANTIC_SEARCH_NATS_SUBJECT: &str = "tasks.search.semantic.request";

#[derive(Serialize, Clone)]
struct ApiResponse {
    message: String,
    task_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SubmitUrlApiPayload {
    url: String,
}

struct AppState {
    nats_client: Arc<NatsClient>,
    sse_tx: broadcast::Sender<String>,
}

async fn submit_url_handler(
    payload: web::Json<SubmitUrlApiPayload>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let url_to_scrape = payload.url.trim();

    if url_to_scrape.is_empty() {
        warn!("[API_SUBMIT_URL] Received empty URL");
        return HttpResponse::BadRequest().json(ApiResponse {
            message: "URL cannot be empty".to_string(),
            task_id: None,
        });
    }

    // TODO: Валидация URL

    info!(
        "[API_SUBMIT_URL] Received request to scrape URL: {}",
        url_to_scrape
    );

    let perceiver_task = PerceiveUrlTask {
        url: url_to_scrape.to_string(),
    };

    match serde_json::to_vec(&perceiver_task) {
        Ok(task_payload_json) => {
            info!(
                "[API_SUBMIT_URL] Publishing PerceiveUrlTask to NATS subject: {}",
                PERCEPTION_URL_TASK_SUBJECT
            );
            if let Err(e) = app_state
                .nats_client
                .publish(PERCEPTION_URL_TASK_SUBJECT, task_payload_json.into())
                .await
            {
                error!(
                    "[API_SUBMIT_URL] Failed to publish PerceiveUrlTask to NATS: {}",
                    e
                );
                HttpResponse::InternalServerError().json(ApiResponse {
                    message: "Failed to publish task to processing queue".to_string(),
                    task_id: None,
                })
            } else {
                info!(
                    "[API_SUBMIT_URL] Successfully published PerceiveUrlTask for URL: {}",
                    url_to_scrape
                );
                HttpResponse::Ok().json(ApiResponse {
                    message: format!(
                        "Task to scrape URL '{}' submitted successfully.",
                        url_to_scrape
                    ),
                    task_id: None,
                })
            }
        }
        Err(e) => {
            error!(
                "[API_SUBMIT_URL] Failed to serialize PerceiveUrlTask: {}",
                e
            );
            HttpResponse::InternalServerError().json(ApiResponse {
                message: "Internal error: Failed to prepare task".to_string(),
                task_id: None,
            })
        }
    }
}

async fn generate_text_handler(
    task_payload_from_http: web::Json<GenerateTextTask>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let task = task_payload_from_http.into_inner();

    info!(
        "[API] /api/generate-text called with task_id: {}",
        task.task_id
    );
    debug!("[API_GENERATE_TEXT] Task details: {:?}", task);

    if task.task_id.trim().is_empty() {
        warn!("[API_GENERATE_TEXT] Received task with empty task_id");
        return HttpResponse::BadRequest().json(ApiResponse {
            message: "task_id cannot be empty".to_string(),
            task_id: None,
        });
    }

    if task.max_length == 0 || task.max_length > 1000 {
        warn!(
            "[API_GENERATE_TEXT] Received task with invalid max_length: {}",
            task.max_length
        );
        return HttpResponse::BadRequest().json(ApiResponse {
            message: "max_length must be between 1 and 1000".to_string(),
            task_id: Some(task.task_id),
        });
    }

    match serde_json::to_vec(&task) {
        Ok(nats_payload_json) => {
            info!(
                "[API_GENERATE_TEXT] Publishing GenerateTextTask (id: {}) to NATS subject: {}",
                task.task_id, GENERATE_TEXT_TASK_SUBJECT
            );
            if let Err(e) = app_state
                .nats_client
                .publish(GENERATE_TEXT_TASK_SUBJECT, nats_payload_json.into())
                .await
            {
                error!(
                    "[API_GENERATE_TEXT] Failed to publish GenerateTextTask (id: {}) to NATS: {}",
                    task.task_id, e
                );
                HttpResponse::InternalServerError().json(ApiResponse {
                    message: "Failed to publish generation task to queue".to_string(),
                    task_id: Some(task.task_id.clone()),
                })
            } else {
                info!(
                    "[API_GENERATE_TEXT] Successfully published GenerateTextTask (id: {})",
                    task.task_id
                );
                HttpResponse::Ok().json(ApiResponse {
                    message: format!(
                        "Text generation task (id: {}) submitted successfully.",
                        task.task_id
                    ),
                    task_id: Some(task.task_id.clone()),
                })
            }
        }
        Err(e) => {
            error!(
                "[API_GENERATE_TEXT] Failed to serialize GenerateTextTask (id: {}): {}",
                task.task_id, e
            );
            HttpResponse::InternalServerError().json(ApiResponse {
                message: "Internal error: Failed to prepare generation task".to_string(),
                task_id: Some(task.task_id.clone()),
            })
        }
    }
}

async fn sse_events_handler(
    app_state: web::Data<AppState>,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, ActixError>>> {
    info!("[API_SSE] New SSE client connected to /api/events");

    let rx = app_state.sse_tx.subscribe();

    let event_stream = BroadcastStream::new(rx).filter_map(
        |result: Result<String, BroadcastStreamRecvError>| async move {
            match result {
                Ok(json_payload) => Some(Ok(SseEvent::Data(SseData::new(json_payload)))),
                Err(BroadcastStreamRecvError::Lagged(num_skipped)) => {
                    warn!(
                        "[SSE_STREAM] SSE receiver lagged, skipped {} messages.",
                        num_skipped
                    );
                    None
                }
            }
        },
    );

    Sse::from_stream(event_stream).with_keep_alive(Duration::from_secs(15))
}

async fn nats_to_sse_listener(nats_client: Arc<NatsClient>, sse_tx: broadcast::Sender<String>) {
    info!(
        "[NATS_SSE_Bridge] Subscribing to NATS subject: {}",
        TEXT_GENERATED_EVENT_SUBJECT
    );
    match nats_client.subscribe(TEXT_GENERATED_EVENT_SUBJECT).await {
        Ok(mut subscriber) => {
            info!(
                "[NATS_SSE_Bridge] Successfully subscribed to {}",
                TEXT_GENERATED_EVENT_SUBJECT
            );
            while let Some(message) = subscriber.next().await {
                debug!(
                    "[NATS_SSE_Bridge] Received NATS message for SSE: {:?}",
                    message.payload
                );
                match serde_json::from_slice::<GeneratedTextMessage>(&message.payload) {
                    Ok(gen_text_msg) => match serde_json::to_string(&gen_text_msg) {
                        Ok(json_payload_for_sse) => {
                            if let Err(e) = sse_tx.send(json_payload_for_sse) {
                                warn!(
                                    "[NATS_SSE_Bridge] Failed to send message to broadcast channel (no active SSE receivers?): {}",
                                    e
                                );
                            } else {
                                info!(
                                    "[NATS_SSE_Bridge] Forwarded GeneratedTextMessage (task_id: {}) to SSE broadcast channel.",
                                    gen_text_msg.original_task_id
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                "[NATS_SSE_Bridge] Failed to re-serialize GeneratedTextMessage for SSE: {}",
                                e
                            );
                        }
                    },
                    Err(e) => {
                        error!(
                            "[NATS_SSE_Bridge] Failed to deserialize GeneratedTextMessage from NATS: {}",
                            e
                        );
                    }
                }
            }
            info!("[NATS_SSE_Bridge] NATS subscription for SSE ended.");
        }
        Err(e) => {
            error!(
                "[NATS_SSE_Bridge] Failed to subscribe to {} for SSE: {}",
                TEXT_GENERATED_EVENT_SUBJECT, e
            );
        }
    }
}

async fn semantic_search_handler(
    http_payload: web::Json<SemanticSearchApiRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let search_api_req = http_payload.into_inner();
    let client_request_id = Uuid::new_v4().to_string();

    info!(
        "[API_SEARCH_HANDLER] Received semantic search request (client_req_id: {}): query='{}', top_k={}",
        client_request_id, search_api_req.query_text, search_api_req.top_k
    );

    let embedding_task = QueryForEmbeddingTask {
        request_id: client_request_id.clone(),
        text_to_embed: search_api_req.query_text.clone(),
    };

    let embedding_task_payload_json = match serde_json::to_vec(&embedding_task) {
        Ok(json) => json,
        Err(e) => {
            error!(
                "[API_SEARCH_HANDLER] Failed to serialize QueryForEmbeddingTask (client_req_id: {}): {}",
                client_request_id, e
            );
            return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some("Internal error: Failed to prepare embedding task".to_string()),
            });
        }
    };

    info!(
        "[API_SEARCH_HANDLER] Requesting embedding for query (client_req_id: {}) from preprocessing_service on subject '{}'",
        client_request_id, EMBEDDING_FOR_QUERY_NATS_SUBJECT
    );

    let embedding_response_msg = match tokio::time::timeout(
        Duration::from_secs(15),
        app_state.nats_client.request(
            EMBEDDING_FOR_QUERY_NATS_SUBJECT.to_string(),
            embedding_task_payload_json.into(),
        ),
    )
    .await
    {
        Ok(result) => match result {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "[API_SEARCH_HANDLER] NATS request for embedding failed (client_req_id: {}): {}",
                    client_request_id, e
                );
                return HttpResponse::ServiceUnavailable().json(SemanticSearchApiResponse {
                    search_request_id: client_request_id,
                    results: vec![],
                    error_message: Some(format!(
                        "Failed to get embedding from preprocessing service: {}",
                        e
                    )),
                });
            }
        },
        Err(_) => {
            error!(
                "[API_SEARCH_HANDLER] NATS request for embedding timed out after 15 seconds (client_req_id: {})",
                client_request_id
            );
            return HttpResponse::ServiceUnavailable().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some(
                    "Timeout: Failed to get embedding from preprocessing service within 15 seconds"
                        .to_string(),
                ),
            });
        }
    };

    let embedding_result: QueryEmbeddingResult = match serde_json::from_slice(
        &embedding_response_msg.payload,
    ) {
        Ok(res) => res,
        Err(e) => {
            error!(
                "[API_SEARCH_HANDLER] Failed to deserialize QueryEmbeddingResult (client_req_id: {}): {}",
                client_request_id, e
            );
            return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some(
                    "Internal error: Failed to parse embedding service response".to_string(),
                ),
            });
        }
    };

    if let Some(err_msg) = embedding_result.error_message {
        error!(
            "[API_SEARCH_HANDLER] Preprocessing service returned error for embedding (client_req_id: {}): {}",
            client_request_id, err_msg
        );
        return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
            search_request_id: client_request_id,
            results: vec![],
            error_message: Some(format!("Error from preprocessing service: {}", err_msg)),
        });
    }

    let query_embedding = match embedding_result.embedding {
        Some(emb) => emb,
        None => {
            error!(
                "[API_SEARCH_HANDLER] Preprocessing service returned no embedding and no error (client_req_id: {})",
                client_request_id
            );
            return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some(
                    "Preprocessing service did not return an embedding.".to_string(),
                ),
            });
        }
    };
    info!(
        "[API_SEARCH_HANDLER] Successfully received embedding for query (client_req_id: {}). Model: {:?}",
        client_request_id, embedding_result.model_name
    );

    let search_nats_task = SemanticSearchNatsTask {
        request_id: client_request_id.clone(),
        query_embedding,
        top_k: search_api_req.top_k,
    };

    let search_nats_task_payload_json = match serde_json::to_vec(&search_nats_task) {
        Ok(json) => json,
        Err(e) => {
            error!(
                "[API_SEARCH_HANDLER] Failed to serialize SemanticSearchNatsTask (client_req_id: {}): {}",
                client_request_id, e
            );
            return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some("Internal error: Failed to prepare search task".to_string()),
            });
        }
    };

    info!(
        "[API_SEARCH_HANDLER] Requesting semantic search (client_req_id: {}) from vector_memory_service on subject '{}'",
        client_request_id, SEMANTIC_SEARCH_NATS_SUBJECT
    );

    let search_response_msg = match tokio::time::timeout(
        Duration::from_secs(20),
        app_state.nats_client.request(
            SEMANTIC_SEARCH_NATS_SUBJECT.to_string(),
            search_nats_task_payload_json.into(),
        ),
    )
    .await
    {
        Ok(result) => match result {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "[API_SEARCH_HANDLER] NATS request for search failed (client_req_id: {}): {}",
                    client_request_id, e
                );
                return HttpResponse::ServiceUnavailable().json(SemanticSearchApiResponse {
                    search_request_id: client_request_id,
                    results: vec![],
                    error_message: Some(format!(
                        "Failed to get search results from vector memory service: {}",
                        e
                    )),
                });
            }
        },
        Err(_) => {
            error!(
                "[API_SEARCH_HANDLER] NATS request for search timed out after 20 seconds (client_req_id: {})",
                client_request_id
            );
            return HttpResponse::ServiceUnavailable().json(SemanticSearchApiResponse {
            search_request_id: client_request_id,
            results: vec![],
            error_message: Some(
                "Timeout: Failed to get search results from vector memory service within 20 seconds".to_string()
            ),
        });
        }
    };

    let search_nats_result: SemanticSearchNatsResult = match serde_json::from_slice(
        &search_response_msg.payload,
    ) {
        Ok(res) => res,
        Err(e) => {
            error!(
                "[API_SEARCH_HANDLER] Failed to deserialize SemanticSearchNatsResult (client_req_id: {}): {}",
                client_request_id, e
            );
            return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
                search_request_id: client_request_id,
                results: vec![],
                error_message: Some(
                    "Internal error: Failed to parse search service response".to_string(),
                ),
            });
        }
    };

    if let Some(err_msg) = search_nats_result.error_message {
        error!(
            "[API_SEARCH_HANDLER] Vector memory service returned error for search (client_req_id: {}): {}",
            client_request_id, err_msg
        );
        return HttpResponse::InternalServerError().json(SemanticSearchApiResponse {
            search_request_id: client_request_id,
            results: vec![],
            error_message: Some(format!("Error from vector memory service: {}", err_msg)),
        });
    }

    info!(
        "[API_SEARCH_HANDLER] Successfully received {} search results for client_req_id: {}",
        search_nats_result.results.len(),
        client_request_id
    );

    HttpResponse::Ok().json(SemanticSearchApiResponse {
        search_request_id: client_request_id,
        results: search_nats_result.results,
        error_message: None,
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("[api_service] Starting Actix Web server...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        warn!(
            "[NATS_CONFIG] NATS_URL (for API service) not set, defaulting to nats://cs-nats:4222"
        );
        "nats://cs-nats:4222".to_string()
    });
    let nats_client = Arc::new(async_nats::connect(&nats_url).await.map_err(|e| {
        error!(
            "[NATS_CONNECT_FAIL] Failed to connect to NATS for API service: {}",
            e
        );
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("NATS connect error: {}", e),
        )
    })?);
    info!("[NATS_CONNECT_SUCCESS] API Service connected to NATS.");

    let (sse_tx, _) = broadcast::channel::<String>(32);

    let nats_client_for_listener = Arc::clone(&nats_client);
    let sse_tx_for_listener = sse_tx.clone();
    tokio::spawn(async move {
        nats_to_sse_listener(nats_client_for_listener, sse_tx_for_listener).await;
    });

    let server_host = env::var("API_SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let server_port_str = env::var("API_SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    let server_port = server_port_str.parse::<u16>().unwrap_or(8080);

    info!(
        "[HTTP_SERVER] Starting API HTTP server at http://{}:{}",
        server_host, server_port
    );

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin_fn(|origin, _req_head| {
                origin.as_bytes().starts_with(b"http://localhost")
                    || origin.as_bytes().starts_with(b"http://marchenzo")
                    || origin.as_bytes().starts_with(b"http://127.0.0.1")
            })
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::ACCEPT,
                header::CONTENT_TYPE,
            ])
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(AppState {
                nats_client: Arc::clone(&nats_client),
                sse_tx: sse_tx.clone(),
            }))
            .service(
                web::scope("/api")
                    .route("/submit-url", web::post().to(submit_url_handler))
                    .route("/generate-text", web::post().to(generate_text_handler))
                    .route("/events", web::get().to(sse_events_handler))
                    .route("/search/semantic", web::post().to(semantic_search_handler)),
            )
    })
    .bind((server_host, server_port))?
    .run()
    .await
}
