use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PerceiveUrlTask {
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RawTextMessage {
    pub id: String,
    pub source_url: String,
    pub raw_text: String,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenizedTextMessage {
    pub original_id: String,
    pub source_url: String,
    pub tokens: Vec<String>,
    pub sentences: Vec<String>,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenerateTextTask {
    pub task_id: String,
    pub prompt: Option<String>,
    pub max_length: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneratedTextMessage {
    pub original_task_id: String,
    pub generated_text: String,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SentenceEmbedding {
    pub sentence_text: String,
    pub embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TextWithEmbeddingsMessage {
    pub original_id: String,
    pub source_url: String,
    pub embeddings_data: Vec<SentenceEmbedding>,
    pub model_name: String,
    pub timestamp_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticSearchApiRequest {
    pub query_text: String,
    pub top_k: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryForEmbeddingTask {
    pub request_id: String,
    pub text_to_embed: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueryEmbeddingResult {
    pub request_id: String,
    pub embedding: Option<Vec<f32>>,
    pub model_name: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QdrantPointPayload {
    pub original_document_id: String,
    pub source_url: String,
    pub sentence_text: String,
    pub sentence_order: u32,
    pub model_name: String,
    pub processed_at_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticSearchNatsTask {
    pub request_id: String,
    pub query_embedding: Vec<f32>,
    pub top_k: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticSearchResultItem {
    pub qdrant_point_id: String,
    pub score: f32,
    pub payload: QdrantPointPayload,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticSearchNatsResult {
    pub request_id: String,
    pub results: Vec<SemanticSearchResultItem>,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticSearchApiResponse {
    pub search_request_id: String,
    pub results: Vec<SemanticSearchResultItem>,
    pub error_message: Option<String>,
}

pub fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perceive_url_task_serialization() {
        let task = PerceiveUrlTask {
            url: "http://example.com".to_string(),
        };
        let serialized = serde_json::to_string(&task).unwrap();
        let deserialized: PerceiveUrlTask = serde_json::from_str(&serialized).unwrap();
        assert_eq!(task.url, deserialized.url);
    }

    #[test]
    fn test_raw_text_message_serialization() {
        let msg = RawTextMessage {
            id: "test-id".to_string(),
            source_url: "http://example.com".to_string(),
            raw_text: "Hello world".to_string(),
            timestamp_ms: current_timestamp_ms(),
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: RawTextMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(msg.id, deserialized.id);
        assert_eq!(msg.raw_text, deserialized.raw_text);
    }

    #[test]
    fn test_tokenized_text_message_serialization() {
        let msg = TokenizedTextMessage {
            original_id: "test-id".to_string(),
            source_url: "http://example.com".to_string(),
            tokens: vec!["Hello".to_string(), "world".to_string()],
            sentences: vec!["Hello world.".to_string()],
            timestamp_ms: current_timestamp_ms(),
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: TokenizedTextMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(msg.original_id, deserialized.original_id);
        assert_eq!(msg.tokens.len(), 2);
    }

    #[test]
    fn test_generate_text_task_serialization() {
        let task = GenerateTextTask {
            task_id: generate_uuid(),
            prompt: Some("Hello".to_string()),
            max_length: 50,
        };
        let serialized = serde_json::to_string(&task).unwrap();
        let deserialized: GenerateTextTask = serde_json::from_str(&serialized).unwrap();
        assert_eq!(task.task_id, deserialized.task_id);
        assert_eq!(task.prompt, deserialized.prompt);
    }

    #[test]
    fn test_generated_text_message_serialization() {
        let msg = GeneratedTextMessage {
            original_task_id: "test-id".to_string(),
            generated_text: "Hello world".to_string(),
            timestamp_ms: current_timestamp_ms(),
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: GeneratedTextMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(msg.original_task_id, deserialized.original_task_id);
        assert_eq!(msg.generated_text, deserialized.generated_text);
    }

    #[test]
    fn test_sentence_embedding_serialization() {
        let se = SentenceEmbedding {
            sentence_text: "This is a test sentence.".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
        };
        let serialized = serde_json::to_string(&se).unwrap();
        let deserialized: SentenceEmbedding = serde_json::from_str(&serialized).unwrap();
        assert_eq!(se.sentence_text, deserialized.sentence_text);
        assert_eq!(se.embedding, deserialized.embedding);
    }

    #[test]
    fn test_text_with_embeddings_message_serialization() {
        let msg = TextWithEmbeddingsMessage {
            original_id: "doc-123".to_string(),
            source_url: "http://example.com".to_string(),
            embeddings_data: vec![
                SentenceEmbedding {
                    sentence_text: "Sentence one.".to_string(),
                    embedding: vec![0.1, 0.2],
                },
                SentenceEmbedding {
                    sentence_text: "Sentence two.".to_string(),
                    embedding: vec![0.3, 0.4],
                },
            ],
            model_name: "test-model-v1".to_string(),
            timestamp_ms: current_timestamp_ms(),
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: TextWithEmbeddingsMessage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(msg.original_id, deserialized.original_id);
        assert_eq!(msg.embeddings_data.len(), 2);
        assert_eq!(msg.embeddings_data[0].sentence_text, "Sentence one.");
        assert_eq!(msg.model_name, deserialized.model_name);
    }

    #[test]
    fn test_semantic_search_api_request_serialization() {
        let req = SemanticSearchApiRequest {
            query_text: "Hello world".to_string(),
            top_k: 10,
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: SemanticSearchApiRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.query_text, deserialized.query_text);
        assert_eq!(req.top_k, deserialized.top_k);
    }

    #[test]
    fn test_query_for_embedding_task_serialization() {
        let task = QueryForEmbeddingTask {
            request_id: generate_uuid(),
            text_to_embed: "Hello world".to_string(),
        };
        let serialized = serde_json::to_string(&task).unwrap();
        let deserialized: QueryForEmbeddingTask = serde_json::from_str(&serialized).unwrap();
        assert_eq!(task.request_id, deserialized.request_id);
        assert_eq!(task.text_to_embed, deserialized.text_to_embed);
    }

    #[test]
    fn test_query_embedding_result_serialization() {
        let result = QueryEmbeddingResult {
            request_id: generate_uuid(),
            embedding: Some(vec![0.1, 0.2, 0.3]),
            model_name: Some("test-model-v1".to_string()),
            error_message: None,
        };
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: QueryEmbeddingResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(result.request_id, deserialized.request_id);
        assert_eq!(result.embedding, deserialized.embedding);
        assert_eq!(result.model_name, deserialized.model_name);
    }

    #[test]
    fn test_qdrant_point_payload_serialization() {
        let payload = QdrantPointPayload {
            original_document_id: "doc-123".to_string(),
            source_url: "http://example.com".to_string(),
            sentence_text: "This is a test sentence.".to_string(),
            sentence_order: 1,
            model_name: "test-model-v1".to_string(),
            processed_at_ms: current_timestamp_ms(),
        };
        let serialized = serde_json::to_string(&payload).unwrap();
        let deserialized: QdrantPointPayload = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            payload.original_document_id,
            deserialized.original_document_id
        );
        assert_eq!(payload.source_url, deserialized.source_url);
        assert_eq!(payload.sentence_text, deserialized.sentence_text);
        assert_eq!(payload.sentence_order, deserialized.sentence_order);
        assert_eq!(payload.model_name, deserialized.model_name);
        assert_eq!(payload.processed_at_ms, deserialized.processed_at_ms);
    }

    #[test]
    fn test_semantic_search_nats_task_serialization() {
        let task = SemanticSearchNatsTask {
            request_id: generate_uuid(),
            query_embedding: vec![0.1, 0.2, 0.3],
            top_k: 10,
        };
        let serialized = serde_json::to_string(&task).unwrap();
        let deserialized: SemanticSearchNatsTask = serde_json::from_str(&serialized).unwrap();
        assert_eq!(task.request_id, deserialized.request_id);
        assert_eq!(task.query_embedding, deserialized.query_embedding);
        assert_eq!(task.top_k, deserialized.top_k);
    }

    #[test]
    fn test_semantic_search_result_item_serialization() {
        let item = SemanticSearchResultItem {
            qdrant_point_id: "point-123".to_string(),
            score: 0.5,
            payload: QdrantPointPayload {
                original_document_id: "doc-123".to_string(),
                source_url: "http://example.com".to_string(),
                sentence_text: "This is a test sentence.".to_string(),
                sentence_order: 1,
                model_name: "test-model-v1".to_string(),
                processed_at_ms: current_timestamp_ms(),
            },
        };
        let serialized = serde_json::to_string(&item).unwrap();
        let deserialized: SemanticSearchResultItem = serde_json::from_str(&serialized).unwrap();
        assert_eq!(item.qdrant_point_id, deserialized.qdrant_point_id);
        assert_eq!(item.score, deserialized.score);
        assert_eq!(
            item.payload.original_document_id,
            deserialized.payload.original_document_id
        );
        assert_eq!(item.payload.source_url, deserialized.payload.source_url);
        assert_eq!(
            item.payload.sentence_text,
            deserialized.payload.sentence_text
        );
        assert_eq!(
            item.payload.sentence_order,
            deserialized.payload.sentence_order
        );
        assert_eq!(item.payload.model_name, deserialized.payload.model_name);
        assert_eq!(
            item.payload.processed_at_ms,
            deserialized.payload.processed_at_ms
        );
    }

    #[test]
    fn test_semantic_search_nats_result_serialization() {
        let result = SemanticSearchNatsResult {
            request_id: generate_uuid(),
            results: vec![
                SemanticSearchResultItem {
                    qdrant_point_id: "point-123".to_string(),
                    score: 0.5,
                    payload: QdrantPointPayload {
                        original_document_id: "doc-123".to_string(),
                        source_url: "http://example.com".to_string(),
                        sentence_text: "This is a test sentence.".to_string(),
                        sentence_order: 1,
                        model_name: "test-model-v1".to_string(),
                        processed_at_ms: current_timestamp_ms(),
                    },
                },
                SemanticSearchResultItem {
                    qdrant_point_id: "point-456".to_string(),
                    score: 0.4,
                    payload: QdrantPointPayload {
                        original_document_id: "doc-456".to_string(),
                        source_url: "http://example.com".to_string(),
                        sentence_text: "This is another test sentence.".to_string(),
                        sentence_order: 2,
                        model_name: "test-model-v1".to_string(),
                        processed_at_ms: current_timestamp_ms(),
                    },
                },
            ],
            error_message: None,
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: SemanticSearchNatsResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(result.request_id, deserialized.request_id);
        assert_eq!(result.results.len(), 2);
        assert_eq!(
            result.results[0].qdrant_point_id,
            deserialized.results[0].qdrant_point_id
        );
        assert_eq!(result.results[0].score, deserialized.results[0].score);
        assert_eq!(
            result.results[0].payload.original_document_id,
            deserialized.results[0].payload.original_document_id
        );
        assert_eq!(
            result.results[0].payload.source_url,
            deserialized.results[0].payload.source_url
        );
        assert_eq!(
            result.results[0].payload.sentence_text,
            deserialized.results[0].payload.sentence_text
        );
        assert_eq!(
            result.results[0].payload.sentence_order,
            deserialized.results[0].payload.sentence_order
        );
        assert_eq!(
            result.results[0].payload.model_name,
            deserialized.results[0].payload.model_name
        );
        assert_eq!(
            result.results[0].payload.processed_at_ms,
            deserialized.results[0].payload.processed_at_ms
        );
        assert_eq!(
            result.results[1].qdrant_point_id,
            deserialized.results[1].qdrant_point_id
        );
        assert_eq!(result.results[1].score, deserialized.results[1].score);
        assert_eq!(
            result.results[1].payload.original_document_id,
            deserialized.results[1].payload.original_document_id
        );
        assert_eq!(
            result.results[1].payload.source_url,
            deserialized.results[1].payload.source_url
        );
        assert_eq!(
            result.results[1].payload.sentence_text,
            deserialized.results[1].payload.sentence_text
        );
        assert_eq!(
            result.results[1].payload.sentence_order,
            deserialized.results[1].payload.sentence_order
        );
        assert_eq!(
            result.results[1].payload.model_name,
            deserialized.results[1].payload.model_name
        );
        assert_eq!(
            result.results[1].payload.processed_at_ms,
            deserialized.results[1].payload.processed_at_ms
        );
    }

    #[test]
    fn test_semantic_search_api_response_serialization() {
        let response = SemanticSearchApiResponse {
            search_request_id: generate_uuid(),
            results: vec![
                SemanticSearchResultItem {
                    qdrant_point_id: "point-123".to_string(),
                    score: 0.5,
                    payload: QdrantPointPayload {
                        original_document_id: "doc-123".to_string(),
                        source_url: "http://example.com".to_string(),
                        sentence_text: "This is a test sentence.".to_string(),
                        sentence_order: 1,
                        model_name: "test-model-v1".to_string(),
                        processed_at_ms: current_timestamp_ms(),
                    },
                },
                SemanticSearchResultItem {
                    qdrant_point_id: "point-456".to_string(),
                    score: 0.4,
                    payload: QdrantPointPayload {
                        original_document_id: "doc-456".to_string(),
                        source_url: "http://example.com".to_string(),
                        sentence_text: "This is another test sentence.".to_string(),
                        sentence_order: 2,
                        model_name: "test-model-v1".to_string(),
                        processed_at_ms: current_timestamp_ms(),
                    },
                },
            ],
            error_message: None,
        };

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: SemanticSearchApiResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(response.search_request_id, deserialized.search_request_id);
        assert_eq!(response.results.len(), 2);
        assert_eq!(
            response.results[0].qdrant_point_id,
            deserialized.results[0].qdrant_point_id
        );
        assert_eq!(response.results[0].score, deserialized.results[0].score);
        assert_eq!(
            response.results[0].payload.original_document_id,
            deserialized.results[0].payload.original_document_id
        );
        assert_eq!(
            response.results[0].payload.source_url,
            deserialized.results[0].payload.source_url
        );
        assert_eq!(
            response.results[0].payload.sentence_text,
            deserialized.results[0].payload.sentence_text
        );
        assert_eq!(
            response.results[0].payload.sentence_order,
            deserialized.results[0].payload.sentence_order
        );
        assert_eq!(
            response.results[0].payload.model_name,
            deserialized.results[0].payload.model_name
        );
        assert_eq!(
            response.results[0].payload.processed_at_ms,
            deserialized.results[0].payload.processed_at_ms
        );
        assert_eq!(
            response.results[1].qdrant_point_id,
            deserialized.results[1].qdrant_point_id
        );
        assert_eq!(response.results[1].score, deserialized.results[1].score);
        assert_eq!(
            response.results[1].payload.original_document_id,
            deserialized.results[1].payload.original_document_id
        );
        assert_eq!(
            response.results[1].payload.source_url,
            deserialized.results[1].payload.source_url
        );
        assert_eq!(
            response.results[1].payload.sentence_text,
            deserialized.results[1].payload.sentence_text
        );
        assert_eq!(
            response.results[1].payload.sentence_order,
            deserialized.results[1].payload.sentence_order
        );
        assert_eq!(
            response.results[1].payload.model_name,
            deserialized.results[1].payload.model_name
        );
        assert_eq!(
            response.results[1].payload.processed_at_ms,
            deserialized.results[1].payload.processed_at_ms
        );
    }
}
