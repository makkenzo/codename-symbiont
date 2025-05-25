use anyhow::{Context, Result};
use futures::StreamExt;
use log::{error, info, warn};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollection, Distance, PointStruct, UpsertPoints, Value, VectorParams, VectorsConfig,
};
use shared_models::TextWithEmbeddingsMessage;
use std::collections::HashMap;
use std::time::Duration;
use std::{env, sync::Arc};
use uuid::Uuid;

const TEXT_WITH_EMBEDDINGS_SUBJECT: &str = "data.text.with_embeddings";
const QDRANT_COLLECTION_NAME: &str = "symbiont_document_embeddings";
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

    info!("[NATS_LOOP] Waiting for messages with text embeddings...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_MSG_RECV] Received message on subject: {}",
            message.subject
        );

        match serde_json::from_slice::<TextWithEmbeddingsMessage>(&message.payload) {
            Ok(embeddings_msg) => {
                info!(
                    "[TASK_DESERIALIZED] Deserialized TextWithEmbeddingsMessage (original_id: {})",
                    embeddings_msg.original_id
                );

                let qdrant_client_clone = Arc::clone(&qdrant_client_arc);

                tokio::spawn(async move {
                    if let Err(e) =
                        handle_text_with_embeddings_message(embeddings_msg, qdrant_client_clone)
                            .await
                    {
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
