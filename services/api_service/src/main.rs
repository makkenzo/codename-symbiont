use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use async_nats::Client as NatsClient;
use futures::StreamExt;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use shared_models::{GenerateTextTask, GeneratedTextMessage};
use std::env;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

const TEXT_GENERATED_EVENT_SUBJECT: &str = "events.text.generated";

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

async fn submit_url_handler(payload: web::Json<SubmitUrlApiPayload>) -> impl Responder {
    let task_id = Uuid::new_v4().to_string();
    info!(
        "[API] /api/submit-url called for URL: {}, generated task_id: {}",
        payload.url, task_id
    );
    // TODO: Сформировать PerceiveUrlTask и опубликовать в NATS
    HttpResponse::Ok().json(ApiResponse {
        message: format!("Task to scrape URL {} submitted (stub)", payload.url),
        task_id: Some(task_id),
    })
}

async fn generate_text_handler(task_payload: web::Json<GenerateTextTask>) -> impl Responder {
    let task = task_payload.into_inner();
    info!(
        "[API] /api/generate-text called with task_id: {}",
        task.task_id
    );
    // TODO: Опубликовать GenerateTextTask в NATS
    HttpResponse::Ok().json(ApiResponse {
        message: format!("Text generation task {} submitted (stub)", task.task_id),
        task_id: Some(task.task_id),
    })
}

async fn sse_events_handler() -> impl Responder {
    info!("[API] SSE client connected to /api/events");
    // TODO: Реализовать подписку на broadcast канал и стриминг SSE
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .append_header(("Cache-Control", "no-cache"))
        .append_header(("Connection", "keep-alive"))
        .body(": sse connected (stub)\n\n")
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
        App::new()
            .app_data(web::Data::new(AppState {
                nats_client: Arc::clone(&nats_client),
                sse_tx: sse_tx.clone(),
            }))
            .service(
                web::scope("/api")
                    .route("/submit-url", web::post().to(submit_url_handler))
                    .route("/generate-text", web::post().to(generate_text_handler))
                    .route("/events", web::get().to(sse_events_handler)),
            )
    })
    .bind((server_host, server_port))?
    .run()
    .await
}
