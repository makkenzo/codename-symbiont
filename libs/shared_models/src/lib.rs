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

pub fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
}
