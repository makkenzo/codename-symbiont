use futures::StreamExt;
use std::{env, sync::Arc};

use log::{debug, error, info, warn};
use neo4rs::{ConfigBuilder, Error as Neo4jError, Graph};
use shared_models::TokenizedTextMessage;

const PROCESSED_TEXT_TOKENIZED_SUBJECT: &str = "data.processed_text.tokenized";

async fn handle_tokenized_text_message(msg: TokenizedTextMessage) {
    info!(
        "[KG_HANDLER] Received TokenizedTextMessage (original_id: {}), {} tokens, {} sentences.",
        msg.original_id,
        msg.tokens.len(),
        msg.sentences.len()
    );
    // TODO: Сформировать и выполнить Cypher-запросы для сохранения в Neo4j
    debug!(
        "[KG_HANDLER] Stub: Data for original_id {} would be processed for Neo4j.",
        msg.original_id
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        warn!("[NATS_CONFIG] NATS_URL not set, defaulting to nats://localhost:4222");
        "nats://localhost:4222".to_string()
    });
    info!(
        "[NATS_CONNECT] Attempting to connect to NATS server at {}...",
        nats_url
    );

    let nats_client = Arc::new(match async_nats::connect(&nats_url).await {
        Ok(client) => {
            info!("[NATS_CONNECT_SUCCESS] Successfully connected to NATS!");
            client
        }
        Err(err) => {
            error!("[NATS_CONNECT_FAIL] Failed to connect to NATS: {}", err);
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    });

    let mut subscriber = match nats_client
        .subscribe(PROCESSED_TEXT_TOKENIZED_SUBJECT)
        .await
    {
        Ok(sub) => {
            info!(
                "[NATS_SUB_SUCCESS] Subscribed to subject: {}",
                PROCESSED_TEXT_TOKENIZED_SUBJECT
            );
            sub
        }
        Err(err) => {
            error!(
                "[NATS_SUB_FAIL] Failed to subscribe to {}: {}",
                PROCESSED_TEXT_TOKENIZED_SUBJECT, err
            );
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };
    info!("[NATS_LOOP] Waiting for tokenized text messages...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_MSG_RECV] Received message on subject: {}",
            message.subject
        );
        debug!("[NATS_MSG_PAYLOAD] Payload (raw): {:?}", message.payload);

        match serde_json::from_slice::<TokenizedTextMessage>(&message.payload) {
            Ok(tokenized_msg) => {
                info!(
                    "[TASK_DESERIALIZED] Deserialized TokenizedTextMessage (original_id: {})",
                    tokenized_msg.original_id
                );

                tokio::spawn(async move {
                    handle_tokenized_text_message(tokenized_msg).await;
                });
            }
            Err(e) => {
                warn!(
                    "[TASK_DESERIALIZE_FAIL] Failed to deserialize TokenizedTextMessage: {}. Payload: {}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                );
            }
        }
    }

    info!("[NATS_LOOP_END] Subscription ended or NATS connection lost.");
    Ok(())
}
