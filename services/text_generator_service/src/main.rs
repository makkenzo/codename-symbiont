use futures::StreamExt;
use log::{debug, error, info, warn};
use shared_models::{GenerateTextTask, GeneratedTextMessage, current_timestamp_ms};
use std::env;
use std::sync::Arc;

const GENERATE_TEXT_TASK_SUBJECT: &str = "tasks.generation.text";
const TEXT_GENERATED_EVENT_SUBJECT: &str = "events.text.generated";

async fn handle_generate_text_task(task: GenerateTextTask, nats_client: Arc<async_nats::Client>) {
    info!(
        "[TEXT_GEN_HANDLER] Received GenerateTextTask (id: {}), max_length: {}",
        task.task_id, task.max_length
    );
    if let Some(prompt) = &task.prompt {
        info!("[TEXT_GEN_HANDLER] Prompt: {}", prompt);
    }

    let generated_output = format!(
        "Stub: Generated text for task {} (max_len: {}) - Lorem ipsum...",
        task.task_id, task.max_length
    );

    let result_message = GeneratedTextMessage {
        original_task_id: task.task_id.clone(),
        generated_text: generated_output,
        timestamp_ms: current_timestamp_ms(),
    };
    debug!(
        "[TEXT_GEN_HANDLER] Stub: Would publish result: {:?}",
        result_message
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

    let mut subscriber = match nats_client.subscribe(GENERATE_TEXT_TASK_SUBJECT).await {
        Ok(sub) => {
            info!(
                "[NATS_SUB_SUCCESS] Subscribed to subject: {}",
                GENERATE_TEXT_TASK_SUBJECT
            );
            sub
        }
        Err(err) => {
            error!(
                "[NATS_SUB_FAIL] Failed to subscribe to {}: {}",
                GENERATE_TEXT_TASK_SUBJECT, err
            );
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };
    info!("[NATS_LOOP] Waiting for text generation tasks...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_MSG_RECV] Received message on subject: {}",
            message.subject
        );
        debug!("[NATS_MSG_PAYLOAD] Payload (raw): {:?}", message.payload);

        match serde_json::from_slice::<GenerateTextTask>(&message.payload) {
            Ok(task) => {
                info!(
                    "[TASK_DESERIALIZED] Deserialized GenerateTextTask (id: {})",
                    task.task_id
                );

                let client_clone = Arc::clone(&nats_client);

                tokio::spawn(async move {
                    handle_generate_text_task(task, client_clone).await;
                });
            }
            Err(e) => {
                warn!(
                    "[TASK_DESERIALIZE_FAIL] Failed to deserialize GenerateTextTask: {}. Payload: {}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                );
            }
        }
    }

    info!("[NATS_LOOP_END] Subscription ended or NATS connection lost.");
    Ok(())
}
