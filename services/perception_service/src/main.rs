use futures::StreamExt;
use serde_json;
use std::env;

use shared_models::PerceiveUrlTask;

const PERCEPTION_URL_TASK_SUBJECT: &str = "tasks.perceive.url";
// const RAW_TEXT_DISCOVERED_SUBJECT: &str = "data.raw_text.discovered";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[perception_service] Starting...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        log_warn("NATS_URL not set, defaulting to nats://localhost:4222");
        "nats://localhost:4222".to_string()
    });

    log_info(&format!(
        "Attempting to connect to NATS server at {}...",
        nats_url
    ));

    let client = match async_nats::connect(&nats_url).await {
        Ok(client) => {
            log_info("Successfully connected to NATS!");
            client
        }
        Err(err) => {
            log_error(&format!("Failed to connect to NATS: {}", err));
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    let mut subscriber = match client.subscribe(PERCEPTION_URL_TASK_SUBJECT).await {
        Ok(sub) => {
            log_info(&format!(
                "Subscribed to subject: {}",
                PERCEPTION_URL_TASK_SUBJECT
            ));
            sub
        }
        Err(err) => {
            log_error(&format!(
                "Failed to subscribe to {}: {}",
                PERCEPTION_URL_TASK_SUBJECT, err
            ));
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    log_info("[perception_service] Waiting for URL tasks...");

    while let Some(message) = subscriber.next().await {
        log_info(&format!("Received message on subject: {}", message.subject));

        match serde_json::from_slice::<PerceiveUrlTask>(&message.payload) {
            Ok(task) => {
                log_info(&format!("Deserialized task for URL: {}", task.url));

                // TODO: Шаг 4 - Реализовать скрапинг task.url
                // TODO: Шаг 5 - Сформировать RawTextMessage и опубликовать
                // Временная заглушка для проверки:
                println!("[perception_service] Received task to scrape: {}", task.url);

                if let Some(reply_subject) = message.reply {
                    let response_payload =
                        format!("Task for {} received by perception_service", task.url);
                    if let Err(e) = client.publish(reply_subject, response_payload.into()).await {
                        log_warn(&format!("Failed to send reply: {}", e));
                    } else {
                        log_info("Sent reply confirmation.");
                    }
                }
            }
            Err(e) => {
                log_warn(&format!(
                    "Failed to deserialize PerceiveUrlTask: {}. Payload: {:?}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                ));
            }
        }
    }

    log_info("[perception_service] Subscription ended or NATS connection lost.");
    Ok(())
}

fn log_info(message: &str) {
    println!("[INFO] {}", message);
}

fn log_warn(message: &str) {
    println!("[WARN] {}", message);
}

fn log_error(message: &str) {
    eprintln!("[ERROR] {}", message);
}
