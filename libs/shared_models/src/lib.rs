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
}
