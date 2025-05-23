use futures::StreamExt;
use std::{collections::HashMap, env, sync::Arc};

use log::{debug, error, info, warn};

use neo4rs::{BoltType, ConfigBuilder, Error as Neo4jError, Graph, Query};
use shared_models::TokenizedTextMessage;

const PROCESSED_TEXT_TOKENIZED_SUBJECT: &str = "data.processed_text.tokenized";

fn new_boxed_error(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    #[derive(Debug)]
    struct StringError(String);
    impl std::fmt::Display for StringError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    impl std::error::Error for StringError {}
    Box::new(StringError(message.to_string()))
}

async fn save_to_neo4j(
    msg: &TokenizedTextMessage,
    graph: Arc<Graph>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "[NEO4J_SAVE] Attempting to save data for original_id: {}",
        msg.original_id
    );

    let mut tx = graph
        .start_txn()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let doc_query_str = "MERGE (d:Document {original_id: $original_id}) \
                         ON CREATE SET d.source_url = $source_url, d.processed_at_ms = $processed_at, d.created_at_ms = timestamp() \
                         ON MATCH SET d.source_url = $source_url, d.processed_at_ms = $processed_at \
                         RETURN id(d) AS doc_node_id";

    let mut doc_params: HashMap<String, BoltType> = HashMap::new();
    doc_params.insert("original_id".to_string(), msg.original_id.clone().into());
    doc_params.insert("source_url".to_string(), msg.source_url.clone().into());
    doc_params.insert(
        "processed_at".to_string(),
        msg.timestamp_ms.to_string().into(),
    );

    let mut doc_stream = tx
        .execute(Query::new(doc_query_str.to_string()).params(doc_params))
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let doc_row = doc_stream
        .next(&mut tx)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        .ok_or_else(|| new_boxed_error("Document node not created/found after MERGE"))?;

    let doc_node_id: i64 = doc_row
        .get("doc_node_id")
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    info!(
        "[NEO4J_SAVE] Document node (Neo4j ID: {}) processed for original_id: {}",
        doc_node_id, msg.original_id
    );

    for (sentence_order, sentence_text) in msg.sentences.iter().enumerate() {
        if sentence_text.trim().is_empty() {
            warn!(
                "[NEO4J_SAVE] Skipping empty sentence for original_id: {}, order: {}",
                msg.original_id, sentence_order
            );
            continue;
        }

        let sentence_query_str = "MATCH (d:Document) WHERE id(d) = $doc_node_id \
                                  MERGE (s:Sentence {text: $text}) \
                                  ON CREATE SET s.created_at_ms = timestamp() \
                                  MERGE (d)-[r:HAS_SENTENCE {order: $order}]->(s) \
                                  RETURN id(s) AS sentence_node_id";

        let mut sentence_params: HashMap<String, BoltType> = HashMap::new();
        sentence_params.insert("doc_node_id".to_string(), doc_node_id.into());
        sentence_params.insert("text".to_string(), sentence_text.as_str().into());
        sentence_params.insert("order".to_string(), (sentence_order as i64).into());

        tx.run(Query::new(sentence_query_str.to_string()).params(sentence_params))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    }
    info!(
        "[NEO4J_SAVE] All {} sentences processed for document original_id: {}",
        msg.sentences.len(),
        msg.original_id
    );

    for token_text_original in msg.tokens.iter() {
        let token_text = token_text_original.trim();
        if token_text.is_empty() {
            warn!(
                "[NEO4J_SAVE] Skipping empty token for original_id: {}",
                msg.original_id
            );
            continue;
        }
        let token_text_lc = token_text.to_lowercase();

        let token_query_str = "MATCH (d:Document) WHERE id(d) = $doc_node_id \
                               MERGE (t:Token {text_lc: $token_text_lc}) \
                               ON CREATE SET t.text_original_case = $token_text_original, t.created_at_ms = timestamp() \
                               ON MATCH SET t.text_original_case = $token_text_original \
                               MERGE (d)-[r_ct:CONTAINS_TOKEN]->(t)";

        let mut token_params: HashMap<String, BoltType> = HashMap::new();
        token_params.insert("doc_node_id".to_string(), doc_node_id.into());
        token_params.insert("token_text_lc".to_string(), token_text_lc.as_str().into());
        token_params.insert("token_text_original".to_string(), token_text.into());

        tx.run(Query::new(token_query_str.to_string()).params(token_params))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    }
    info!(
        "[NEO4J_SAVE] All {} tokens processed for document original_id: {}",
        msg.tokens.len(),
        msg.original_id
    );

    tx.commit()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    info!(
        "[NEO4J_SAVE] Successfully committed transaction for original_id: {}",
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
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting knowledge graph service...");

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
            return Err(Box::new(err) as Box<dyn std::error::Error + Send + Sync>);
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
            return Err(Box::new(err) as Box<dyn std::error::Error + Send + Sync>);
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
        "".to_string()
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
        .build()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let graph = Arc::new(Graph::connect(config).await.map_err(|e| {
        error!("[NEO4J_CONNECT_FAIL] Failed to connect to Neo4j: {:?}", e);
        Box::new(e) as Box<dyn std::error::Error + Send + Sync>
    })?);
    info!("[NEO4J_CONNECT_SUCCESS] Successfully connected to Neo4j!");

    async fn ensure_schema(graph_client: Arc<Graph>) -> Result<(), Neo4jError> {
        info!("[NEO4J_SCHEMA] Ensuring database schema (constraints/indexes)...");
        graph_client
            .run(Query::new(
                "CREATE CONSTRAINT IF NOT EXISTS FOR (d:Document) REQUIRE d.original_id IS UNIQUE"
                    .to_string(),
            ))
            .await?;
        graph_client
            .run(Query::new(
                "CREATE INDEX token_text_lc_index IF NOT EXISTS FOR (t:Token) ON (t.text_lc)"
                    .to_string(),
            ))
            .await?;
        info!("[NEO4J_SCHEMA] Database schema ensured.");
        Ok(())
    }
    if let Err(e) = ensure_schema(Arc::clone(&graph)).await {
        error!(
            "[NEO4J_SCHEMA_FAIL] Failed to ensure Neo4j schema: {:?}. Proceeding without schema guarantees, but this might cause issues.",
            e
        );
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
                error!(
                    "[TASK_DESERIALIZE_FAIL] Failed to deserialize TokenizedTextMessage: {}. Payload: {}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                );
            }
        }
    }

    info!("[NATS_LOOP_END] Subscription ended or NATS connection lost. Shutting down.");
    Ok(())
}
