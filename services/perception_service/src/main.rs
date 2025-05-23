use async_nats::Client as NatsClient;
use futures::StreamExt;
use log::{debug, error, info, trace, warn};
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
    info!("[TASK] Processing task for URL: {}", task.url);

    let scraped_text = match scrape_url_content(&task.url).await {
        Ok(text) => text,
        Err(e) => {
            error!("[SCRAPE_FAIL] Failed to scrape URL {}: {}", task.url, e);
            return Err(e);
        }
    };

    if scraped_text.is_empty() {
        warn!(
            "[SCRAPE_EMPTY] Scraping URL {} yielded no text. Not publishing.",
            task.url
        );
        return Ok(());
    }

    info!(
        "[SCRAPE_SUCCESS] Successfully scraped URL: {}. Text length: {}",
        task.url,
        scraped_text.len()
    );
    trace!(
        "[SCRAPE_CONTENT] Scraped text (first 200): {:.200}",
        scraped_text
    );

    let raw_msg = RawTextMessage {
        id: Uuid::new_v4().to_string(),
        source_url: task.url.clone(),
        raw_text: scraped_text,
        timestamp_ms: current_timestamp_ms(),
    };

    let Ok(payload_json) = serde_json::to_vec(&raw_msg) else {
        error!(
            "[SERIALIZE_FAIL] Failed to serialize RawTextMessage to JSON for id: {}",
            raw_msg.id
        );
        return Err("Failed to serialize RawTextMessage".into());
    };

    debug!(
        "[NATS_PUB] Publishing RawTextMessage (id: {}) to subject: {}",
        raw_msg.id, RAW_TEXT_DISCOVERED_SUBJECT
    );

    if let Err(e) = nats_client
        .publish(RAW_TEXT_DISCOVERED_SUBJECT, payload_json.into())
        .await
    {
        error!(
            "[NATS_PUB_FAIL] Failed to publish RawTextMessage (id: {}) to NATS: {}",
            raw_msg.id, e
        );
        return Err(Box::new(e) as Box<dyn std::error::Error>);
    } else {
        info!(
            "[NATS_PUB_SUCCESS] Successfully published RawTextMessage (id: {})",
            raw_msg.id
        );
    }

    Ok(())
}

async fn scrape_url_content(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    info!("[SCRAPE_URL_CONTENT] Scraping URL: {}", url);

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
                info!(
                    "[SCRAPE_URL_CONTENT] Found content block with selector: {}",
                    selector_str
                );
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
        warn!(
            "[SCRAPE_URL_CONTENT] No meaningful text content extracted from {}",
            url
        );
    } else {
        info!(
            "[SCRAPE_URL_CONTENT] Extracted text (first 200 chars): {:.200}",
            &extracted_text[..200]
        );
    }

    Ok(extracted_text)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting ...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        warn!("[NATS_URL] NATS_URL not set, defaulting to nats://localhost:4222");
        "nats://localhost:4222".to_string()
    });

    info!(
        "[NATS_URL] Attempting to connect to NATS server at {}...",
        nats_url
    );

    let client = Arc::new(match async_nats::connect(&nats_url).await {
        Ok(client) => {
            info!("[NATS_URL] Successfully connected to NATS!");
            client
        }
        Err(err) => {
            error!("[NATS_URL] Failed to connect to NATS: {}", err);
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    });

    let mut subscriber = match client.subscribe(PERCEPTION_URL_TASK_SUBJECT).await {
        Ok(sub) => {
            info!(
                "[NATS_URL] Subscribed to subject: {}",
                PERCEPTION_URL_TASK_SUBJECT
            );
            sub
        }
        Err(err) => {
            error!(
                "[NATS_URL] Failed to subscribe to {}: {}",
                PERCEPTION_URL_TASK_SUBJECT, err
            );
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    info!("[NATS_URL] Waiting for URL tasks...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_URL] Received message on subject: {}",
            message.subject
        );

        match serde_json::from_slice::<PerceiveUrlTask>(&message.payload) {
            Ok(task) => {
                info!("[NATS_URL] Deserialized task for URL: {}", task.url);

                let nats_client_clone = Arc::clone(&client);

                tokio::spawn(async move {
                    if let Err(e) = scrape_and_publish(task, nats_client_clone).await {
                        error!("[NATS_URL] Error during scrape_and_publish: {}", e);
                    }
                });
            }
            Err(e) => {
                warn!(
                    "[NATS_URL] Failed to deserialize PerceiveUrlTask: {}. Payload: {:?}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                );
            }
        }
    }

    info!("[NATS_URL] Subscription ended or NATS connection lost.");
    Ok(())
}
