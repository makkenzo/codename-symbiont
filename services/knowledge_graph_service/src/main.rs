use futures::StreamExt;
use std::{env, sync::Arc};

use log::{debug, error, info, warn};
use neo4rs::{ConfigBuilder, Error as Neo4jError, Graph, query};
use shared_models::TokenizedTextMessage;

const PROCESSED_TEXT_TOKENIZED_SUBJECT: &str = "data.processed_text.tokenized";

async fn save_to_neo4j(msg: &TokenizedTextMessage, graph: Arc<Graph>) -> Result<(), Neo4jError> {
    info!(
        "[NEO4J_SAVE] Attempting to save data for original_id: {}",
        msg.original_id
    );

    let mut tx = graph.start_txn().await?;

    let doc_query = query(
        "MERGE (d:Document {original_id: $original_id}) \
         ON CREATE SET d.source_url = $source_url, d.processed_at_ms = $processed_at \
         ON MATCH SET d.source_url = $source_url, d.processed_at_ms = $processed_at \
         RETURN id(d) AS doc_node_id",
    )
    .param("original_id", msg.original_id.clone())
    .param("source_url", msg.source_url.clone())
    .param("processed_at", msg.timestamp_ms as i64);

    tx.run(doc_query).await?;
    info!(
        "[NEO4J_SAVE] Document node processed for original_id: {}",
        msg.original_id
    );

    for (sentence_order, sentence_text) in msg.sentences.iter().enumerate() {
        if sentence_text.trim().is_empty() {
            continue;
        }

        let sentence_query_str = format!(
            "MATCH (d:Document {{original_id: $original_id}}) \
             MERGE (d)-[r:HAS_SENTENCE {{order: $order}}]->(s:Sentence) \
             ON CREATE SET s.text = $text, s.order = $order \
             ON MATCH SET s.text = $text \
             RETURN id(s) AS sentence_node_id"
        );

        let sentence_query = query(&sentence_query_str)
            .param("original_id", msg.original_id.clone())
            .param("text", sentence_text.clone())
            .param("order", sentence_order as i64);

        tx.run(sentence_query).await?;
    }

    for token_text in msg.tokens.iter() {
        if token_text.trim().is_empty() {
            continue;
        }
        let token_query_str = format!(
            "MATCH (d:Document {{original_id: $original_id}}) \
             MERGE (t:Token {{text: $token_text}}) \
             MERGE (d)-[:CONTAINS_TOKEN]->(t)"
        );
        let token_query = query(&token_query_str)
            .param("original_id", msg.original_id.clone())
            .param("token_text", token_text.to_lowercase());

        tx.run(token_query).await?;
    }

    info!(
        "[NEO4J_SAVE] Token nodes processed for document_id: {}",
        msg.original_id
    );

    tx.commit().await?;
    info!(
        "[NEO4J_SAVE] Successfully saved data for original_id: {}",
        msg.original_id
    );

    Ok(())
}

async fn handle_tokenized_text_message(msg: TokenizedTextMessage, graph: Arc<Graph>) {
    info!(
        "[KG_HANDLER] Received TokenizedTextMessage (original_id: {}), {} tokens, {} sentences.",
        msg.original_id,
        msg.tokens.len(),
        msg.sentences.len()
    );

    if let Err(e) = save_to_neo4j(&msg, graph).await {
        error!(
            "[KG_HANDLER_ERROR] Failed to save data to Neo4j for original_id {}: {}",
            msg.original_id, e
        );
    }
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

    let neo4j_uri = env::var("NEO4J_URI").unwrap_or_else(|_| {
        warn!("[NEO4J_CONFIG] NEO4J_URI not set, defaulting to bolt://localhost:7687");
        "bolt://localhost:7687".to_string()
    });
    let neo4j_user = env::var("NEO4J_USER").unwrap_or_else(|_| {
        warn!("[NEO4J_CONFIG] NEO4J_USER not set, defaulting to 'neo4j'");
        "neo4j".to_string()
    });

    let neo4j_pass = env::var("NEO4J_PASSWORD").unwrap_or_else(|_| {
        warn!("[NEO4J_CONFIG] NEO4J_PASSWORD not set. Ensure Neo4j auth is 'none' or provide password.");
        "s3cr3tPassword".to_string()
    });

    info!(
        "[NEO4J_CONNECT] Attempting to connect to Neo4j at URI: {}, User: {}",
        neo4j_uri, neo4j_user
    );

    let config = ConfigBuilder::default()
        .uri(&neo4j_uri)
        .user(&neo4j_user)
        .password(&neo4j_pass)
        .db("neo4j")
        .fetch_size(500)
        .max_connections(10)
        .build()?;

    let graph = Arc::new(Graph::connect(config).await.map_err(|e| {
        error!("[NEO4J_CONNECT_FAIL] Failed to connect to Neo4j: {:?}", e);
        e
    })?);
    info!("[NEO4J_CONNECT_SUCCESS] Successfully connected to Neo4j!");

    async fn ensure_schema(graph_client: Arc<Graph>) -> Result<(), Neo4jError> {
        info!("[NEO4J_SCHEMA] Ensuring database schema (constraints/indexes)...");
        graph_client
            .run(query(
                "CREATE CONSTRAINT IF NOT EXISTS FOR (d:Document) REQUIRE d.original_id IS UNIQUE",
            ))
            .await?;
        graph_client
            .run(query(
                "CREATE INDEX token_text_index IF NOT EXISTS FOR (t:Token) ON (t.text)",
            ))
            .await?;
        info!("[NEO4J_SCHEMA] Database schema ensured.");
        Ok(())
    }
    if let Err(e) = ensure_schema(Arc::clone(&graph)).await {
        error!("[NEO4J_SCHEMA_FAIL] Failed to ensure Neo4j schema: {:?}", e);
    }

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

                let graph_clone = Arc::clone(&graph);

                tokio::spawn(async move {
                    handle_tokenized_text_message(tokenized_msg, graph_clone).await;
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
