mod embedding_generator;
use anyhow::{Context, Result};
use async_nats::Message;
use embedding_generator::EmbeddingGenerator;
use futures::StreamExt;
use log::{debug, error, info, warn};
use serde_json;
use shared_models::{
    QueryEmbeddingResult, QueryForEmbeddingTask, RawTextMessage, SentenceEmbedding,
    TextWithEmbeddingsMessage, current_timestamp_ms,
};
use std::env;
use std::sync::Arc;

const RAW_TEXT_DISCOVERED_SUBJECT: &str = "data.raw_text.discovered";
const TEXT_WITH_EMBEDDINGS_SUBJECT: &str = "data.text.with_embeddings";
const EMBEDDING_FOR_QUERY_TASK_SUBJECT: &str = "tasks.embedding.for_query";

fn process_text_and_embed(
    raw_msg: &RawTextMessage,
    embed_generator: &EmbeddingGenerator,
) -> Result<TextWithEmbeddingsMessage, String> {
    info!(
        "[text_processor] Processing text for id: {}, url: {}",
        raw_msg.id, raw_msg.source_url
    );

    let cleaned_text = raw_msg
        .raw_text
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    if cleaned_text.is_empty() {
        warn!(
            "[TEXT_PROCESSOR_EMBED] Cleaned text is empty for id: {}",
            raw_msg.id
        );
        return Err(format!("Cleaned text is empty for id: {}", raw_msg.id));
    }

    let mut sentences_str = Vec::new();
    let mut current_sentence_start = 0;
    for (i, character) in cleaned_text.char_indices() {
        if character == '.' || character == '?' || character == '!' {
            if i >= current_sentence_start {
                let sentence_slice = &cleaned_text[current_sentence_start..=i];
                sentences_str.push(sentence_slice.trim().to_string());
                current_sentence_start = i + 1;
            }
        }
    }

    if current_sentence_start < cleaned_text.len() {
        let remainder = cleaned_text[current_sentence_start..].trim();
        if !remainder.is_empty() {
            sentences_str.push(remainder.to_string());
        }
    }

    if sentences_str.is_empty() && !cleaned_text.is_empty() {
        sentences_str.push(cleaned_text.clone());
    }

    if sentences_str.is_empty() {
        warn!(
            "[TEXT_PROCESSOR_EMBED] No sentences extracted for id: {}",
            raw_msg.id
        );
        return Err(format!("No sentences extracted for id: {}", raw_msg.id));
    }

    info!(
        "[TEXT_PROCESSOR_EMBED] Extracted {} sentences for id: {}",
        sentences_str.len(),
        raw_msg.id
    );

    debug!(
        "[TEXT_PROCESSOR_EMBED] Generating embeddings for {} sentences...",
        sentences_str.len()
    );

    let embeddings = match embed_generator.generate_sentence_embeddings(&sentences_str) {
        Ok(embs) => embs,
        Err(e) => {
            let err_msg = format!("Failed to generate embeddings for id {}: {}", raw_msg.id, e);
            error!("[TEXT_PROCESSOR_EMBED] {}", err_msg);
            return Err(err_msg);
        }
    };

    if embeddings.len() != sentences_str.len() {
        let err_msg = format!(
            "Mismatch between number of sentences ({}) and embeddings ({}) for id: {}",
            sentences_str.len(),
            embeddings.len(),
            raw_msg.id
        );
        error!("[TEXT_PROCESSOR_EMBED] {}", err_msg);
        return Err(err_msg);
    }
    info!(
        "[TEXT_PROCESSOR_EMBED] Successfully generated {} embeddings for id: {}",
        embeddings.len(),
        raw_msg.id
    );

    let embeddings_data: Vec<SentenceEmbedding> = sentences_str
        .into_iter()
        .zip(embeddings.into_iter())
        .map(|(sentence, embedding)| SentenceEmbedding {
            sentence_text: sentence,
            embedding,
        })
        .collect();

    Ok(TextWithEmbeddingsMessage {
        original_id: raw_msg.id.clone(),
        source_url: raw_msg.source_url.clone(),
        embeddings_data,
        model_name: "sentence-transformers/paraphrase-multilingual-mpnet-base-v2".to_string(),
        timestamp_ms: current_timestamp_ms(),
    })
}

async fn handle_raw_text_message_and_publish_embeddings(
    raw_text_msg: RawTextMessage,
    nats_client: Arc<async_nats::Client>,
    embed_generator: Arc<EmbeddingGenerator>,
) {
    match process_text_and_embed(&raw_text_msg, &embed_generator) {
        Ok(msg_with_embeddings) => {
            info!(
                "[NATS_PUB_PREP] Text processed with embeddings for original_id: {}. Publishing...",
                msg_with_embeddings.original_id
            );

            match serde_json::to_vec(&msg_with_embeddings) {
                Ok(payload_json) => {
                    if let Err(e) = nats_client
                        .publish(TEXT_WITH_EMBEDDINGS_SUBJECT, payload_json.into())
                        .await
                    {
                        error!(
                            "[NATS_PUB_FAIL] Failed to publish TextWithEmbeddingsMessage (original_id: {}): {}",
                            msg_with_embeddings.original_id, e
                        );
                    } else {
                        info!(
                            "[NATS_PUB_SUCCESS] Successfully published TextWithEmbeddingsMessage (original_id: {}) with {} embeddings.",
                            msg_with_embeddings.original_id,
                            msg_with_embeddings.embeddings_data.len()
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "[SERIALIZE_FAIL] Failed to serialize TextWithEmbeddingsMessage (original_id: {}): {}",
                        msg_with_embeddings.original_id, e
                    );
                }
            }
        }
        Err(e) => {
            error!(
                "[PROCESS_TEXT_FAIL] Failed to process text with embeddings for id {}: {}",
                raw_text_msg.id, e
            );
        }
    }
}

async fn handle_query_for_embedding_task(
    nats_msg: Message,
    embed_generator: Arc<EmbeddingGenerator>,
    nats_client_for_reply: Arc<async_nats::Client>,
) -> Result<()> {
    let task: QueryForEmbeddingTask = match serde_json::from_slice(&nats_msg.payload) {
        Ok(t) => t,
        Err(e) => {
            let err_msg = format!("Failed to deserialize QueryForEmbeddingTask: {}", e);
            error!("[QUERY_EMBED_HANDLER_DESERIALIZE_FAIL] {}", err_msg);
            if let Some(reply_to) = &nats_msg.reply {
                let error_result = QueryEmbeddingResult {
                    request_id: "unknown".to_string(),
                    embedding: None,
                    model_name: None,
                    error_message: Some(err_msg.clone()),
                };
                if let Ok(payload_json) = serde_json::to_vec(&error_result) {
                    let _ = nats_client_for_reply
                        .publish(reply_to.clone(), payload_json.into())
                        .await;
                }
            }
            return Err(anyhow::anyhow!(err_msg));
        }
    };

    info!(
        "[QUERY_EMBED_HANDLER] Processing QueryForEmbeddingTask (request_id: {}), text: '{}'",
        task.request_id, task.text_to_embed
    );

    let sentences_to_embed = vec![task.text_to_embed.clone()];
    let mut result_embedding: Option<Vec<f32>> = None;
    let mut error_msg_opt: Option<String> = None;
    let model_name_used =
        Some("sentence-transformers/paraphrase-multilingual-mpnet-base-v2".to_string());

    match embed_generator.generate_sentence_embeddings(&sentences_to_embed) {
        Ok(mut embeddings_vec) => {
            if embeddings_vec.len() == 1 {
                result_embedding = embeddings_vec.pop();
                if result_embedding.is_none() {
                    let err_str = format!(
                        "Embedding generation returned an empty vector for request_id {}",
                        task.request_id
                    );
                    error!("[QUERY_EMBED_HANDLER_FAIL] {}", err_str);
                    error_msg_opt = Some(err_str);
                } else {
                    info!(
                        "[QUERY_EMBED_HANDLER] Successfully generated embedding for request_id {}",
                        task.request_id
                    );
                }
            } else {
                let err_str = format!(
                    "Embedding generation for a single sentence returned {} embeddings for request_id {}",
                    embeddings_vec.len(),
                    task.request_id
                );
                error!("[QUERY_EMBED_HANDLER_UNEXPECTED_COUNT] {}", err_str);
                error_msg_opt = Some(err_str);
            }
        }
        Err(e) => {
            let err_str = format!(
                "Failed to generate embedding for request_id {}: {}",
                task.request_id, e
            );
            error!("[QUERY_EMBED_HANDLER_GENERATION_FAIL] {}", err_str);
            error_msg_opt = Some(err_str);
        }
    }

    let final_result = QueryEmbeddingResult {
        request_id: task.request_id.clone(),
        embedding: result_embedding,
        model_name: model_name_used,
        error_message: error_msg_opt,
    };

    if let Some(reply_to) = nats_msg.reply {
        match serde_json::to_vec(&final_result) {
            Ok(payload_json) => {
                info!(
                    "[QUERY_EMBED_HANDLER] Sending embedding result for request_id {} to NATS reply subject: {}",
                    task.request_id, reply_to
                );
                if let Err(e) = nats_client_for_reply
                    .publish(reply_to, payload_json.into())
                    .await
                {
                    error!(
                        "[QUERY_EMBED_HANDLER_NATS_REPLY_FAIL] Failed to publish embedding result for request_id {}: {}",
                        task.request_id, e
                    );
                }
            }
            Err(e) => {
                error!(
                    "[QUERY_EMBED_HANDLER_SERIALIZE_FAIL] Failed to serialize QueryEmbeddingResult for request_id {}: {}",
                    task.request_id, e
                );
                let error_result_on_serialize_fail = QueryEmbeddingResult {
                    request_id: task.request_id.clone(),
                    embedding: None,
                    model_name: None,
                    error_message: Some(format!("Failed to serialize result: {}", e)),
                };
                if let Ok(err_payload_json) = serde_json::to_vec(&error_result_on_serialize_fail) {
                    let _ = nats_client_for_reply
                        .publish(reply_to, err_payload_json.into())
                        .await;
                }
            }
        }
    } else {
        warn!(
            "[QUERY_EMBED_HANDLER] No reply subject provided for query embedding task_id {}. Result not sent.",
            task.request_id
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,preprocessing_service=debug,candle_core=warn,candle_nn=warn,candle_transformers=warn,tokenizers=warn,hf_hub=warn")).init();
    println!("Starting with embedding generation capabilities...");

    let model_id = "sentence-transformers/paraphrase-multilingual-mpnet-base-v2";
    let revision = "main".to_string();
    let force_cpu = env::var("FORCE_CPU").map_or(false, |v| v == "1" || v.to_lowercase() == "true");

    info!(
        "[EMBED_INIT] Initializing EmbeddingGenerator with model: {}, revision: {}, force_cpu: {}",
        model_id, revision, force_cpu
    );

    let embedding_generator = Arc::new(
        EmbeddingGenerator::new(model_id, Some(revision), force_cpu)
            .context("Failed to create EmbeddingGenerator during service startup")?,
    );

    info!("[EMBED_INIT_SUCCESS] EmbeddingGenerator initialized successfully.");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| {
        warn!("[NATS_CONFIG] NATS_URL not set, defaulting to nats://localhost:4222");
        "nats://localhost:4222".to_string()
    });
    info!(
        "[NATS_CONNECT] Attempting to connect to NATS server at {}...",
        nats_url
    );

    let client = match async_nats::connect(&nats_url).await {
        Ok(client) => {
            info!("Successfully connected to NATS!");
            Arc::new(client)
        }
        Err(err) => {
            error!("Failed to connect to NATS: {}", err);
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    let mut raw_text_subscriber = match client.subscribe(RAW_TEXT_DISCOVERED_SUBJECT).await {
        Ok(sub) => {
            info!("Subscribed to subject: {}", RAW_TEXT_DISCOVERED_SUBJECT);
            sub
        }
        Err(err) => {
            error!(
                "Failed to subscribe to {}: {}",
                RAW_TEXT_DISCOVERED_SUBJECT, err
            );
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };

    let nats_client_for_raw_text_task = Arc::clone(&client);
    let embedding_generator_for_raw_text_task = Arc::clone(&embedding_generator);

    tokio::spawn(async move {
        info!("[NATS_LOOP_RAW_TEXT] Waiting for raw text messages to process and embed...");
        while let Some(message) = raw_text_subscriber.next().await {
            info!(
                "[NATS_MSG_RECV_RAW_TEXT] Received message on subject: {}",
                message.subject
            );

            match serde_json::from_slice::<RawTextMessage>(&message.payload) {
                Ok(raw_text_msg) => {
                    info!(
                        "[TASK_DESERIALIZED_RAW_TEXT] Deserialized RawTextMessage (id: {}, url: {})",
                        raw_text_msg.id, raw_text_msg.source_url,
                    );

                    let nats_client_clone = Arc::clone(&nats_client_for_raw_text_task);
                    let embed_generator_clone = Arc::clone(&embedding_generator_for_raw_text_task);

                    tokio::spawn(async move {
                        handle_raw_text_message_and_publish_embeddings(
                            raw_text_msg,
                            nats_client_clone,
                            embed_generator_clone,
                        )
                        .await;
                    });
                }
                Err(e) => {
                    warn!(
                        "[TASK_DESERIALIZE_FAIL_RAW_TEXT] Failed to deserialize RawTextMessage: {}. Payload: {:?}",
                        e,
                        String::from_utf8_lossy(&message.payload),
                    );
                }
            }
        }

        info!("[NATS_LOOP_RAW_TEXT_END] Raw text processing subscription ended.");
    });

    let mut query_embedding_subscriber = client
        .subscribe(EMBEDDING_FOR_QUERY_TASK_SUBJECT)
        .await
        .with_context(|| {
        format!(
            "Failed to subscribe to NATS subject {}",
            EMBEDDING_FOR_QUERY_TASK_SUBJECT
        )
    })?;
    info!(
        "[NATS_SUB_SUCCESS] Subscribed to subject: {} for query embedding tasks",
        EMBEDDING_FOR_QUERY_TASK_SUBJECT
    );

    let nats_client_for_query_reply = Arc::clone(&client);
    let embedding_generator_for_query_task = Arc::clone(&embedding_generator);

    info!("[NATS_LOOP_QUERY_EMBED] Waiting for query embedding tasks...");

    while let Some(message) = query_embedding_subscriber.next().await {
        info!(
            "[NATS_MSG_RECV_QUERY_EMBED] Received query embedding task on subject: {}",
            message.subject
        );
        let n_client_clone = Arc::clone(&nats_client_for_query_reply);
        let embed_gen_clone = Arc::clone(&embedding_generator_for_query_task);

        tokio::spawn(async move {
            if let Err(e) =
                handle_query_for_embedding_task(message, embed_gen_clone, n_client_clone).await
            {
                error!(
                    "[HANDLER_ERROR_QUERY_EMBED] Error processing query embedding task: {:?}",
                    e
                );
            }
        });
    }

    info!("[NATS_LOOP_QUERY_EMBED_END] Query embedding subscription ended.");

    Ok(())
}
