use futures::StreamExt;
use serde_json;
use shared_models::{RawTextMessage, TokenizedTextMessage, current_timestamp_ms};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use tokenizers::decoders::sequence::Sequence;
use tokenizers::models::bpe::{BPE, BpeBuilder, Vocab};
use tokenizers::normalizers::BertNormalizer;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::processors::bert::BertProcessing;
use tokenizers::{TokenizerBuilder, TokenizerImpl};

const RAW_TEXT_DISCOVERED_SUBJECT: &str = "data.raw_text.discovered";
const PROCESSED_TEXT_TOKENIZED_SUBJECT: &str = "data.processed_text.tokenized";

fn process_text_message(raw_msg: &RawTextMessage) -> Result<TokenizedTextMessage, String> {
    log_info(&format!(
        "[text_processor] Processing text for id: {}, url: {}",
        raw_msg.id, raw_msg.source_url
    ));

    let cleaned_text = raw_msg
        .raw_text
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    if cleaned_text.is_empty() {
        log_warn(&format!(
            "[text_processor] Cleaned text is empty for id: {}",
            raw_msg.id
        ));
        return Err(format!("Cleaned text is empty for id: {}", raw_msg.id));
    }

    let mut sentences = Vec::new();
    let mut current_sentence_start = 0;
    for (i, character) in cleaned_text.char_indices() {
        if character == '.' || character == '?' || character == '!' {
            if i >= current_sentence_start {
                let sentence_slice = &cleaned_text[current_sentence_start..=i];
                sentences.push(sentence_slice.trim().to_string());
                current_sentence_start = i + 1;
            }
        }
    }

    if current_sentence_start < cleaned_text.len() {
        let remainder = cleaned_text[current_sentence_start..].trim();
        if !remainder.is_empty() {
            sentences.push(remainder.to_string());
        }
    }

    if sentences.is_empty() && !cleaned_text.is_empty() {
        sentences.push(cleaned_text.clone());
    }

    let mut vocab: Vocab = HashMap::new();
    vocab.insert("<unk>".to_string(), 0);

    let merges: Vec<(String, String)> = Vec::new();

    let bpe_model = match BpeBuilder::new()
        .vocab_and_merges(vocab, merges)
        .unk_token("<unk>".to_string())
        .build()
    {
        Ok(model) => model,
        Err(e) => {
            log_error(&format!(
                "[text_processor] Failed to build BPE model: {:?}",
                e
            ));
            return Err(format!("Failed to build BPE model: {:?}", e));
        }
    };

    let tokenizer_result: Result<
        TokenizerImpl<BPE, BertNormalizer, Whitespace, BertProcessing, Sequence>,
        Box<dyn std::error::Error + Send + Sync>,
    > = TokenizerBuilder::new()
        .with_model(bpe_model)
        .with_pre_tokenizer(Some(Whitespace::default()))
        .build();

    let tokenizer = match tokenizer_result {
        Ok(tk) => tk,
        Err(e) => {
            log_error(&format!(
                "[text_processor] Failed to build tokenizer: {:?}",
                e
            ));
            return Err(format!("Failed to build tokenizer: {:?}", e));
        }
    };

    let encoding_result = tokenizer.encode(cleaned_text.clone(), false);

    let tokens: Vec<String> = match encoding_result {
        Ok(encoding) => encoding.get_tokens().to_vec(),
        Err(e) => {
            log_error(&format!(
                "[text_processor] Tokenization failed for id {}: {:?}",
                raw_msg.id, e
            ));
            return Err(format!(
                "Tokenization failed for id {}: {:?}",
                raw_msg.id, e
            ));
        }
    };

    if tokens.is_empty() && !cleaned_text.is_empty() {
        log_warn(&format!(
            "[text_processor] Tokenization yielded no tokens for id: {}, but cleaned text was not empty.",
            raw_msg.id
        ));
        return Err(format!(
            "Tokenization yielded no tokens for id: {}",
            raw_msg.id
        ));
    }

    log_info(&format!(
        "[text_processor] Extracted {} sentences and {} tokens for id: {}",
        sentences.len(),
        tokens.len(),
        raw_msg.id
    ));

    Ok(TokenizedTextMessage {
        original_id: raw_msg.id.clone(),
        source_url: raw_msg.source_url.clone(),
        tokens,
        sentences,
        timestamp_ms: current_timestamp_ms(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[preprocessing_service] Starting...");

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
            Arc::new(client)
        }
        Err(err) => {
            log_error(&format!("Failed to connect to NATS: {}", err));
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    let mut subscriber = match client.subscribe(RAW_TEXT_DISCOVERED_SUBJECT).await {
        Ok(sub) => {
            log_info(&format!(
                "Subscribed to subject: {}",
                RAW_TEXT_DISCOVERED_SUBJECT
            ));
            sub
        }
        Err(err) => {
            log_error(&format!(
                "Failed to subscribe to {}: {}",
                RAW_TEXT_DISCOVERED_SUBJECT, err
            ));
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    log_info("[preprocessing_service] Waiting for raw text messages...");

    while let Some(message) = subscriber.next().await {
        log_info(&format!("Received message on subject: {}", message.subject));

        match serde_json::from_slice::<RawTextMessage>(&message.payload) {
            Ok(raw_text_msg) => {
                log_info(&format!(
                    "Deserialized RawTextMessage (id: {}, url: {})",
                    raw_text_msg.id, raw_text_msg.source_url
                ));

                match process_text_message(&raw_text_msg) {
                    Ok(tokenized_msg) => {
                        // TODO: Опубликовать tokenized_msg в NATS
                        log_info(&format!(
                            "[Stub] Would publish TokenizedTextMessage (original_id: {}) with {} tokens and {} sentences.",
                            tokenized_msg.original_id,
                            tokenized_msg.tokens.len(),
                            tokenized_msg.sentences.len()
                        ));
                    }
                    Err(e) => {
                        log_error(&format!(
                            "Failed to process text for id {}: {}",
                            raw_text_msg.id, e
                        ));
                    }
                }
            }
            Err(e) => {
                log_warn(&format!(
                    "Failed to deserialize RawTextMessage: {}. Payload: {:?}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                ));
            }
        }
    }

    log_info("[preprocessing_service] Subscription ended or NATS connection lost.");
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
