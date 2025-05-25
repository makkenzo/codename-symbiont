use std::{env, error, sync::Arc};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use shared_models::TextWithEmbeddingsMessage;

const TEXT_WITH_EMBEDDINGS_SUBJECT: &str = "data.text.with_embeddings";
const QDRANT_COLLECTION_NAME: &str = "symbiont_document_embeddings";
const QDRANT_VECTOR_DIM: u64 = 768;
use futures::StreamExt;

async fn handle_text_with_embeddings_message(msg: TextWithEmbeddingsMessage) -> Result<()> {
    info!(
        "[QDRANT_HANDLER] Received TextWithEmbeddingsMessage (original_id: {}), {} embeddings from model '{}'.",
        msg.original_id,
        msg.embeddings_data.len(),
        msg.model_name
    );
    // TODO: Сформировать точки и сохранить в Qdrant
    debug!(
        "[QDRANT_HANDLER] Stub: Data for original_id {} would be processed for Qdrant.",
        msg.original_id
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("info,vector_memory_service=debug,qdrant_client=info"),
    )
    .init();
    info!("[vector_memory_service] Starting...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        warn!("[NATS_CONFIG] NATS_URL not set, defaulting to nats://localhost:4222");
        "nats://localhost:4222".to_string()
    });
    info!(
        "[NATS_CONNECT] Attempting to connect to NATS server at {}...",
        nats_url
    );
    let nats_client = Arc::new(
        async_nats::connect(&nats_url)
            .await
            .with_context(|| format!("Failed to connect to NATS at {}", nats_url))?,
    );
    info!("[NATS_CONNECT_SUCCESS] Successfully connected to NATS!");

    let mut subscriber = nats_client
        .subscribe(TEXT_WITH_EMBEDDINGS_SUBJECT)
        .await
        .with_context(|| {
            format!(
                "Failed to subscribe to NATS subject {}",
                TEXT_WITH_EMBEDDINGS_SUBJECT
            )
        })?;
    info!(
        "[NATS_SUB_SUCCESS] Subscribed to subject: {}",
        TEXT_WITH_EMBEDDINGS_SUBJECT
    );

    info!("[NATS_LOOP] Waiting for messages with text embeddings...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_MSG_RECV] Received message on subject: {}",
            message.subject
        );
        debug!(
            "[NATS_MSG_PAYLOAD] Payload (raw) length: {}",
            message.payload.len()
        );

        match serde_json::from_slice::<TextWithEmbeddingsMessage>(&message.payload) {
            Ok(embeddings_msg) => {
                info!(
                    "[TASK_DESERIALIZED] Deserialized TextWithEmbeddingsMessage (original_id: {})",
                    embeddings_msg.original_id
                );

                tokio::spawn(async move {
                    if let Err(e) = handle_text_with_embeddings_message(embeddings_msg).await {
                        error!("[HANDLER_ERROR] Error processing message: {:?}", e);
                    }
                });
            }
            Err(e) => {
                warn!(
                    "[TASK_DESERIALIZE_FAIL] Failed to deserialize TextWithEmbeddingsMessage: {}. Payload (first 100 bytes): {:?}",
                    e,
                    message.payload.get(..100)
                );
            }
        }
    }

    info!("[NATS_LOOP_END] Subscription ended or NATS connection lost.");
    Ok(())
}
