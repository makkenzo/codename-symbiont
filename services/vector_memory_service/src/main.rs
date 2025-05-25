use anyhow::{Context, Result};
use async_nats::Message;
use futures::StreamExt;
use log::{error, info, warn};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, Distance, PointId as QdrantPointId, PointStruct, SearchPoints, UpsertPoints,
    Value, VectorParams, VectorsConfig, WithPayloadSelector, WithVectorsSelector,
};
use shared_models::{
    QdrantPointPayload, SemanticSearchNatsResult, SemanticSearchNatsTask, SemanticSearchResultItem,
    TextWithEmbeddingsMessage,
};
use std::collections::HashMap;
use std::time::Duration;
use std::{env, sync::Arc};
use uuid::Uuid;

const TEXT_WITH_EMBEDDINGS_SUBJECT: &str = "data.text.with_embeddings";
const QDRANT_COLLECTION_NAME: &str = "symbiont_document_embeddings";
const SEMANTIC_SEARCH_TASK_SUBJECT: &str = "tasks.search.semantic.request";
const QDRANT_VECTOR_DIM: u64 = 768;

async fn create_new_qdrant_collection(
    client: Arc<Qdrant>,
    collection_name: &str,
    vector_dim: u64,
) -> Result<()> {
    info!(
        "[QDRANT_CREATE] Attempting to create new collection '{}' with vector size {}...",
        collection_name, vector_dim
    );

    let vectors_config = Some(VectorsConfig::from(VectorParams {
        size: vector_dim,
        distance: Distance::Cosine.into(),
        hnsw_config: None,
        quantization_config: None,
        on_disk: Some(true),
        multivector_config: None,
        datatype: None,
    }));

    let create_collection_request = CreateCollection {
        collection_name: collection_name.to_string(),
        vectors_config,

        hnsw_config: None,
        wal_config: None,
        optimizers_config: None,
        shard_number: None,
        on_disk_payload: Some(true),
        replication_factor: None,
        write_consistency_factor: None,
        init_from_collection: None,
        quantization_config: None,
        sharding_method: None,
        sparse_vectors_config: None,

        strict_mode_config: None,
        timeout: None,
    };

    client
        .create_collection(create_collection_request)
        .await
        .map(|response| {
            info!(
                "[QDRANT_CREATE] Collection '{}' creation reported: {:?}",
                collection_name, response
            );
        })
        .with_context(|| format!("Failed to create Qdrant collection '{}'", collection_name))?;

    info!(
        "[QDRANT_CREATE] Collection '{}' created successfully or request processed.",
        collection_name
    );
    Ok(())
}

async fn ensure_qdrant_collection(
    client: Arc<Qdrant>,
    collection_name: &str,
    vector_dim: u64,
) -> Result<()> {
    info!(
        "[QDRANT_SETUP] Checking if collection '{}' exists...",
        collection_name
    );

    let collections = client
        .list_collections()
        .await
        .with_context(|| "Failed to list Qdrant collections")?;

    let collection_exists = collections
        .collections
        .iter()
        .any(|collection| collection.name == collection_name);

    if collection_exists {
        info!(
            "[QDRANT_SETUP] Collection '{}' already exists, skipping creation.",
            collection_name
        );
    } else {
        info!(
            "[QDRANT_SETUP] Collection '{}' does not exist, creating...",
            collection_name
        );

        create_new_qdrant_collection(client, collection_name, vector_dim)
            .await
            .with_context(|| format!("Failed to create collection '{}'", collection_name))?;
    }

    Ok(())
}

async fn handle_text_with_embeddings_message(
    msg: TextWithEmbeddingsMessage,
    qdrant_client: Arc<Qdrant>,
) -> Result<()> {
    info!(
        "[QDRANT_HANDLER] Received TextWithEmbeddingsMessage (original_id: {}), {} embeddings from model '{}'.",
        msg.original_id,
        msg.embeddings_data.len(),
        msg.model_name
    );

    if msg.embeddings_data.is_empty() {
        warn!(
            "[QDRANT_HANDLER] No embeddings data found in message for original_id: {}. Skipping.",
            msg.original_id
        );
        return Ok(());
    }

    let mut points_to_upsert: Vec<PointStruct> = Vec::with_capacity(msg.embeddings_data.len());

    for (index, sentence_embedding) in msg.embeddings_data.iter().enumerate() {
        let mut payload: HashMap<String, Value> = HashMap::new();
        payload.insert(
            "original_document_id".to_string(),
            Value::from(msg.original_id.clone()),
        );
        payload.insert(
            "source_url".to_string(),
            Value::from(msg.source_url.clone()),
        );
        payload.insert(
            "sentence_text".to_string(),
            Value::from(sentence_embedding.sentence_text.clone()),
        );
        payload.insert("sentence_order".to_string(), Value::from(index as i64));
        payload.insert(
            "model_name".to_string(),
            Value::from(msg.model_name.clone()),
        );
        payload.insert(
            "processed_at_ms".to_string(),
            Value::from(msg.timestamp_ms as i64),
        );

        let point_id = qdrant_client::qdrant::PointId::from(Uuid::new_v4().to_string());

        let point = PointStruct {
            id: Some(point_id),
            payload,
            vectors: Some(qdrant_client::qdrant::Vectors::from(
                sentence_embedding.embedding.clone(),
            )),
        };

        points_to_upsert.push(point);
    }

    if points_to_upsert.is_empty() {
        warn!(
            "[QDRANT_HANDLER] No points to upsert for original_id: {}. This shouldn't happen if embeddings_data was not empty.",
            msg.original_id
        );
        return Ok(());
    }

    info!(
        "[QDRANT_HANDLER] Upserting {} points to Qdrant collection '{}' for original_id: {}...",
        points_to_upsert.len(),
        QDRANT_COLLECTION_NAME,
        msg.original_id
    );

    let upsert_request = UpsertPoints {
        collection_name: QDRANT_COLLECTION_NAME.to_string(),
        wait: Some(true),
        points: points_to_upsert,
        ordering: None,
        shard_key_selector: None,
    };

    match qdrant_client.upsert_points(upsert_request).await {
        Ok(response) => {
            if response.result.map_or(false, |op_info| {
                op_info.status == qdrant_client::qdrant::UpdateStatus::Completed as i32
            }) {
                info!(
                    "[QDRANT_HANDLER] Successfully upserted points for original_id: {}. Qdrant op time: {}s",
                    msg.original_id, response.time
                );
            } else {
                warn!(
                    "[QDRANT_HANDLER] Qdrant upsert operation for original_id: {} completed but status was not 'Completed'. Response: {:?}",
                    msg.original_id, response
                );
            }
        }
        Err(e) => {
            error!(
                "[QDRANT_HANDLER_ERROR] Failed to upsert points to Qdrant for original_id {}: {}",
                msg.original_id, e
            );
            return Err(e.into());
        }
    }

    Ok(())
}

async fn handle_semantic_search_task(
    nats_msg: Message,
    qdrant_client: Arc<Qdrant>,
    nats_client_for_reply: Arc<async_nats::Client>,
) -> Result<()> {
    let task: SemanticSearchNatsTask = match serde_json::from_slice(&nats_msg.payload) {
        Ok(t) => t,
        Err(e) => {
            let err_msg = format!("Failed to deserialize SemanticSearchNatsTask: {}", e);
            error!("[SEARCH_HANDLER_DESERIALIZE_FAIL] {}", err_msg);
            if let Some(reply_to) = &nats_msg.reply {
                let error_result = SemanticSearchNatsResult {
                    request_id: "unknown".to_string(),
                    results: vec![],
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
        "[SEARCH_HANDLER] Processing SemanticSearchNatsTask (request_id: {}, top_k: {})",
        task.request_id, task.top_k
    );

    let search_request = SearchPoints {
        collection_name: QDRANT_COLLECTION_NAME.to_string(),
        vector: task.query_embedding,
        limit: task.top_k as u64,
        with_payload: Some(WithPayloadSelector {
            selector_options: Some(
                qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
            ),
        }),
        with_vectors: Some(WithVectorsSelector {
            selector_options: Some(
                qdrant_client::qdrant::with_vectors_selector::SelectorOptions::Enable(false),
            ),
        }),
        offset: Some(0),
        vector_name: None,
        read_consistency: None,
        timeout: None,
        shard_key_selector: None,
        filter: None,
        score_threshold: None,
        params: None,
        sparse_indices: None,
    };

    let search_result_qdrant = match qdrant_client.search_points(search_request).await {
        Ok(res) => res,
        Err(e) => {
            let err_msg = format!(
                "Qdrant search failed for request_id {}: {}",
                task.request_id, e
            );
            error!("[SEARCH_HANDLER_QDRANT_FAIL] {}", err_msg);
            if let Some(reply_to) = &nats_msg.reply {
                let error_result = SemanticSearchNatsResult {
                    request_id: task.request_id.clone(),
                    results: vec![],
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
        "[SEARCH_HANDLER] Qdrant search completed for request_id {}. Found {} points. Took: {}s",
        task.request_id,
        search_result_qdrant.result.len(),
        search_result_qdrant.time
    );

    let mut results_for_nats: Vec<SemanticSearchResultItem> = Vec::new();

    for scored_point in search_result_qdrant.result {
        let qdrant_point_id_str = match scored_point.id {
            Some(QdrantPointId {
                point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s)),
            }) => s,
            Some(QdrantPointId {
                point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)),
            }) => n.to_string(),
            _ => {
                warn!(
                    "[SEARCH_HANDLER] Found point with missing or unexpected ID format. Skipping."
                );
                continue;
            }
        };

        let payload_map = scored_point.payload;

        let original_document_id = payload_map
            .get("original_document_id")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        let source_url = payload_map
            .get("source_url")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        let sentence_text = payload_map
            .get("sentence_text")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        let sentence_order = payload_map
            .get("sentence_order")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(*i as u32),
                    _ => None,
                })
            })
            .unwrap_or(0);
        let model_name = payload_map
            .get("model_name")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default();
        let processed_at_ms = payload_map
            .get("processed_at_ms")
            .and_then(|v| {
                v.kind.as_ref().and_then(|k| match k {
                    qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(*i as u64),
                    _ => None,
                })
            })
            .unwrap_or(0);

        let qdrant_payload = QdrantPointPayload {
            original_document_id,
            source_url,
            sentence_text,
            sentence_order,
            model_name,
            processed_at_ms,
        };

        results_for_nats.push(SemanticSearchResultItem {
            qdrant_point_id: qdrant_point_id_str,
            score: scored_point.score,
            payload: qdrant_payload,
        });
    }

    let final_result = SemanticSearchNatsResult {
        request_id: task.request_id.clone(),
        results: results_for_nats,
        error_message: None,
    };

    if let Some(reply_to) = nats_msg.reply {
        match serde_json::to_vec(&final_result) {
            Ok(payload_json) => {
                info!(
                    "[SEARCH_HANDLER] Sending search results for request_id {} to NATS reply subject: {}",
                    task.request_id, reply_to
                );
                if let Err(e) = nats_client_for_reply
                    .publish(reply_to, payload_json.into())
                    .await
                {
                    error!(
                        "[SEARCH_HANDLER_NATS_REPLY_FAIL] Failed to publish search result for request_id {}: {}",
                        task.request_id, e
                    );
                }
            }
            Err(e) => {
                error!(
                    "[SEARCH_HANDLER_SERIALIZE_FAIL] Failed to serialize SemanticSearchNatsResult for request_id {}: {}",
                    task.request_id, e
                );
                let error_result_on_serialize_fail = SemanticSearchNatsResult {
                    request_id: task.request_id.clone(),
                    results: vec![],
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
            "[SEARCH_HANDLER] No reply subject provided for search task_id {}. Results not sent.",
            task.request_id
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("info,vector_memory_service=debug,qdrant_client=info"),
    )
    .init();

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

    let mut embeddings_subscriber = nats_client
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

    let qdrant_uri = env::var("QDRANT_URI").unwrap_or_else(|_| {
        warn!("[QDRANT_CONFIG] QDRANT_URI not set, defaulting to http://localhost:6334");
        "http://localhost:6334".to_string()
    });

    info!(
        "[QDRANT_CONNECT] Attempting to connect to Qdrant at URI: {}",
        qdrant_uri
    );

    let qdrant_client_arc: Arc<Qdrant>;
    let max_retries = 5;
    let retry_delay = Duration::from_secs(5);
    let mut attempt = 0;

    loop {
        attempt += 1;
        match Qdrant::from_url(&qdrant_uri).build() {
            Ok(client_instance) => {
                qdrant_client_arc = Arc::new(client_instance);
                info!("[QDRANT_CONNECT_SUCCESS] Successfully created Qdrant client.");
                break;
            }
            Err(e) => {
                error!(
                    "[QDRANT_CONNECT_FAIL] Attempt {} to connect to Qdrant failed: {}. Retrying in {:?}...",
                    attempt, e, retry_delay
                );
                if attempt >= max_retries {
                    error!(
                        "[QDRANT_CONNECT_FATAL] Max retries reached. Failed to connect to Qdrant."
                    );
                    return Err(e.into());
                }
                tokio::time::sleep(retry_delay).await;
            }
        }
    }

    if let Err(e) = ensure_qdrant_collection(
        Arc::clone(&qdrant_client_arc),
        QDRANT_COLLECTION_NAME,
        QDRANT_VECTOR_DIM,
    )
    .await
    {
        error!(
            "[QDRANT_SETUP_FATAL] Failed to ensure Qdrant collection: {}. Service will not be able to store vectors.",
            e
        );
    }

    let qdrant_client_for_storage_task = Arc::clone(&qdrant_client_arc);
    tokio::spawn(async move {
        info!("[NATS_LOOP_STORAGE] Waiting for messages with text embeddings...");

        while let Some(message) = embeddings_subscriber.next().await {
            info!(
                "[NATS_MSG_RECV_STORAGE] Received message on subject: {}",
                message.subject
            );

            match serde_json::from_slice::<TextWithEmbeddingsMessage>(&message.payload) {
                Ok(embeddings_msg) => {
                    info!(
                        "[TASK_DESERIALIZED_STORAGE] Deserialized TextWithEmbeddingsMessage (original_id: {})",
                        embeddings_msg.original_id
                    );
                    let qdrant_client_clone = Arc::clone(&qdrant_client_for_storage_task);
                    tokio::spawn(async move {
                        if let Err(e) =
                            handle_text_with_embeddings_message(embeddings_msg, qdrant_client_clone)
                                .await
                        {
                            error!(
                                "[HANDLER_ERROR_STORAGE] Error processing storage message: {:?}",
                                e
                            );
                        }
                    });
                }
                Err(e) => {
                    warn!(
                        "[TASK_DESERIALIZE_FAIL_STORAGE] Failed to deserialize TextWithEmbeddingsMessage: {}. Payload (first 100 bytes): {:?}",
                        e,
                        message.payload.get(..100)
                    );
                }
            }
        }

        info!("[NATS_LOOP_STORAGE_END] Embeddings storage subscription ended.");
    });

    let mut search_task_subscriber = nats_client
        .subscribe(SEMANTIC_SEARCH_TASK_SUBJECT)
        .await
        .with_context(|| {
            format!(
                "Failed to subscribe to NATS subject {}",
                SEMANTIC_SEARCH_TASK_SUBJECT
            )
        })?;
    info!(
        "[NATS_SUB_SUCCESS] Subscribed to subject: {} for semantic search tasks",
        SEMANTIC_SEARCH_TASK_SUBJECT
    );

    let qdrant_client_for_search_task = Arc::clone(&qdrant_client_arc);
    let nats_client_for_search_reply = Arc::clone(&nats_client);

    info!("[NATS_LOOP_SEARCH] Waiting for semantic search tasks...");
    while let Some(message) = search_task_subscriber.next().await {
        info!(
            "[NATS_MSG_RECV_SEARCH] Received search task on subject: {}",
            message.subject
        );
        let q_client_clone = Arc::clone(&qdrant_client_for_search_task);
        let n_client_clone = Arc::clone(&nats_client_for_search_reply);

        tokio::spawn(async move {
            if let Err(e) =
                handle_semantic_search_task(message, q_client_clone, n_client_clone).await
            {
                error!(
                    "[HANDLER_ERROR_SEARCH] Error processing search task: {:?}",
                    e
                );
            }
        });
    }
    info!("[NATS_LOOP_SEARCH_END] Semantic search subscription ended.");

    Ok(())
}
