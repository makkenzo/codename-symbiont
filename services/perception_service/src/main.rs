use async_nats::Client as NatsClient;
use futures::StreamExt;
use scraper::{Html, Selector};
use serde_json;
use std::sync::Arc;
use std::{env, time::Duration};
use uuid::Uuid;

use shared_models::{PerceiveUrlTask, RawTextMessage, current_timestamp_ms};

const PERCEPTION_URL_TASK_SUBJECT: &str = "tasks.perceive.url";
const RAW_TEXT_DISCOVERED_SUBJECT: &str = "data.raw_text.discovered";

async fn scrape_and_publish(
    task: PerceiveUrlTask,
    nats_client: Arc<NatsClient>,
) -> Result<(), Box<dyn std::error::Error>> {
    log_info(&format!("Processing task for URL: {}", task.url));

    let scraped_text = match scrape_url_content(&task.url).await {
        Ok(text) => text,
        Err(e) => {
            log_error(&format!("Failed to scrape URL {}: {}", task.url, e));
            return Err(e);
        }
    };

    if scraped_text.is_empty() {
        log_warn(&format!(
            "Scraping URL {} yielded no text. Not publishing.",
            task.url
        ));
        return Ok(());
    }

    log_info(&format!(
        "Successfully scraped URL: {}. Text length: {}",
        task.url,
        scraped_text.len()
    ));

    let raw_msg = RawTextMessage {
        id: Uuid::new_v4().to_string(),
        source_url: task.url.clone(),
        raw_text: scraped_text,
        timestamp_ms: current_timestamp_ms(),
    };

    let Ok(payload_json) = serde_json::to_vec(&raw_msg) else {
        log_error("Failed to serialize RawTextMessage to JSON");
        return Err("Failed to serialize RawTextMessage".into());
    };

    log_info(&format!(
        "Publishing RawTextMessage (id: {}) to subject: {}",
        raw_msg.id, RAW_TEXT_DISCOVERED_SUBJECT
    ));

    if let Err(e) = nats_client
        .publish(RAW_TEXT_DISCOVERED_SUBJECT, payload_json.into())
        .await
    {
        log_error(&format!(
            "Failed to publish RawTextMessage (id: {}) to NATS: {}",
            raw_msg.id, e
        ));
        return Err(Box::new(e) as Box<dyn std::error::Error>);
    } else {
        log_info(&format!(
            "Successfully published RawTextMessage (id: {})",
            raw_msg.id
        ));
    }

    Ok(())
}

async fn scrape_url_content(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    log_info(&format!("Scraping URL: {}", url));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("CodenameSymbiontBot/0.1 (+https://makkenzo.com)")
        .build()?;

    let response_text = client.get(url).send().await?.text().await?;

    let document = Html::parse_document(&response_text);

    let mut content_parts = Vec::new();

    let selectors_to_try = vec![
        "article",
        "main",
        "div[role='main']",
        "div.content",
        "div.post-content",
        "div.entry-content",
        "body",
    ];

    let mut main_content_html = None;

    for selector_str in selectors_to_try {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                main_content_html = Some(element.html());
                log_info(&format!(
                    "Found content block with selector: {}",
                    selector_str
                ));
                break;
            }
        }
    }

    let html_to_parse = main_content_html.as_ref().unwrap_or(&response_text);
    let fragment_to_parse = Html::parse_fragment(html_to_parse);

    let text_selectors_str = vec!["h1", "h2", "h3", "h4", "h5", "h6", "p", "li", "span"];

    for selector_str in text_selectors_str {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element_ref in fragment_to_parse.select(&selector) {
                let mut element_text = String::new();
                for text_node in element_ref.text() {
                    let trimmed_text_node = text_node.trim();
                    if !trimmed_text_node.is_empty() {
                        element_text.push_str(trimmed_text_node);
                        element_text.push(' ');
                    }
                }
                let cleaned_text_for_element = element_text.trim();
                if !cleaned_text_for_element.is_empty() {
                    content_parts.push(cleaned_text_for_element.to_string());
                }
            }
        }
    }

    let extracted_text = content_parts
        .join("\n")
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join("\n");

    if extracted_text.is_empty() {
        log_warn(&format!(
            "No meaningful text content extracted from {}",
            url
        ));
    } else {
        log_info(&format!(
            "Extracted text (first 200 chars): {:.200}",
            extracted_text
        ));
    }

    Ok(extracted_text)
}

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

    let client = Arc::new(match async_nats::connect(&nats_url).await {
        Ok(client) => {
            log_info("Successfully connected to NATS!");
            client
        }
        Err(err) => {
            log_error(&format!("Failed to connect to NATS: {}", err));
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    });

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

                let nats_client_clone = Arc::clone(&client);

                tokio::spawn(async move {
                    if let Err(e) = scrape_and_publish(task, nats_client_clone).await {
                        log_error(&format!("Error during scrape_and_publish: {}", e));
                    }
                });
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
