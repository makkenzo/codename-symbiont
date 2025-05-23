use futures::StreamExt;
use log::{debug, error, info, warn};
use rand::seq::SliceRandom;
use rand::thread_rng;
use shared_models::{GenerateTextTask, GeneratedTextMessage, current_timestamp_ms};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

const GENERATE_TEXT_TASK_SUBJECT: &str = "tasks.generation.text";
const TEXT_GENERATED_EVENT_SUBJECT: &str = "events.text.generated";

type MarkovChainModel = HashMap<String, Vec<String>>;

#[derive(Clone, Debug)]
struct MarkovModel {
    chain: MarkovChainModel,
    starters: Vec<String>,
}

impl MarkovModel {
    fn new() -> Self {
        MarkovModel {
            chain: HashMap::new(),
            starters: Vec::new(),
        }
    }

    fn train(&mut self, text: &str) {
        if text.is_empty() {
            warn!("[MARKOV_TRAIN] Input text for training is empty.");
            return;
        }
        info!("[MARKOV_TRAIN] Training Markov model...");

        let words: Vec<String> = text.split_whitespace().map(String::from).collect();

        if words.len() < 2 {
            warn!(
                "[MARKOV_TRAIN] Not enough words in text to train (need at least 2). Text: '{}'",
                text
            );
            if !words.is_empty() {
                self.starters.push(words[0].clone());
            }
            return;
        }

        self.starters.push(words[0].clone());

        for i in 0..(words.len() - 1) {
            let current_word = words[i].clone();
            let next_word = words[i + 1].clone();

            self.chain
                .entry(current_word)
                .or_insert_with(Vec::new)
                .push(next_word);
        }

        self.starters.sort();
        self.starters.dedup();
        info!(
            "[MARKOV_TRAIN] Training complete. Model has {} states. {} starter words.",
            self.chain.len(),
            self.starters.len()
        );
        if self.chain.len() < 20 && !self.chain.is_empty() {
            debug!(
                "[MARKOV_TRAIN] Model sample: {:?}",
                self.chain.iter().take(5).collect::<Vec<_>>()
            );
        }
        if self.starters.len() < 20 && !self.starters.is_empty() {
            debug!(
                "[MARKOV_TRAIN] Starters sample: {:?}",
                self.starters.iter().take(5).collect::<Vec<_>>()
            );
        }
    }

    fn generate(&self, max_length: u32) -> String {
        if self.chain.is_empty() || self.starters.is_empty() {
            warn!(
                "[MARKOV_GENERATE] Model is not trained or has no starters. Cannot generate text."
            );
            return String::from("Model not trained.");
        }

        let mut rng = thread_rng();
        let mut current_word = self.starters.choose(&mut rng).unwrap().clone();
        let mut result_text = vec![current_word.clone()];

        for _ in 0..(max_length - 1) {
            if let Some(next_words) = self.chain.get(current_word.as_str()) {
                if let Some(next_word) = next_words.choose(&mut rng) {
                    result_text.push(next_word.clone());
                    current_word = next_word.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        result_text.join(" ")
    }
}

async fn handle_generate_text_task(
    task: GenerateTextTask,
    nats_client: Arc<async_nats::Client>,
    markov_model: Arc<MarkovModel>,
) {
    info!(
        "[TEXT_GEN_HANDLER] Received GenerateTextTask (id: {}), max_length: {}",
        task.task_id, task.max_length
    );
    if let Some(prompt) = &task.prompt {
        info!("[TEXT_GEN_HANDLER] Prompt: {}", prompt);
        // TODO: Использовать prompt
    }

    let generated_output = markov_model.generate(task.max_length);
    info!("[TEXT_GEN_HANDLER] Generated text: '{}'", generated_output);

    let result_message = GeneratedTextMessage {
        original_task_id: task.task_id.clone(),
        generated_text: generated_output,
        timestamp_ms: current_timestamp_ms(),
    };

    match serde_json::to_vec(&result_message) {
        Ok(payload_json) => {
            info!(
                "[NATS_PUB_PREP] Publishing GeneratedTextMessage (task_id: {}) to subject: {}",
                result_message.original_task_id, TEXT_GENERATED_EVENT_SUBJECT
            );
            if let Err(e) = nats_client
                .publish(TEXT_GENERATED_EVENT_SUBJECT, payload_json.into())
                .await
            {
                error!(
                    "[NATS_PUB_FAIL] Failed to publish GeneratedTextMessage (task_id: {}): {}",
                    result_message.original_task_id, e
                );
            } else {
                info!(
                    "[NATS_PUB_SUCCESS] Successfully published GeneratedTextMessage (task_id: {})",
                    result_message.original_task_id
                );
            }
        }
        Err(e) => {
            error!(
                "[SERIALIZE_FAIL] Failed to serialize GeneratedTextMessage (task_id: {}): {}",
                result_message.original_task_id, e
            );
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting...");

    let mut model = MarkovModel::new();
    let training_text = "я пошел гулять в парк и увидел там собаку собака была очень веселая и я решил с ней поиграть";

    model.train(training_text);
    let markov_model_instance = Arc::new(model);
    info!("[MAIN] Markov model initialized and trained.");

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

    let mut subscriber = match nats_client.subscribe(GENERATE_TEXT_TASK_SUBJECT).await {
        Ok(sub) => {
            info!(
                "[NATS_SUB_SUCCESS] Subscribed to subject: {}",
                GENERATE_TEXT_TASK_SUBJECT
            );
            sub
        }
        Err(err) => {
            error!(
                "[NATS_SUB_FAIL] Failed to subscribe to {}: {}",
                GENERATE_TEXT_TASK_SUBJECT, err
            );
            return Err(Box::new(err) as Box<dyn std::error::Error>);
        }
    };
    info!("[NATS_LOOP] Waiting for text generation tasks...");

    while let Some(message) = subscriber.next().await {
        info!(
            "[NATS_MSG_RECV] Received message on subject: {}",
            message.subject
        );
        debug!("[NATS_MSG_PAYLOAD] Payload (raw): {:?}", message.payload);

        match serde_json::from_slice::<GenerateTextTask>(&message.payload) {
            Ok(task) => {
                info!(
                    "[TASK_DESERIALIZED] Deserialized GenerateTextTask (id: {})",
                    task.task_id
                );

                let client_clone = Arc::clone(&nats_client);
                let model_clone = Arc::clone(&markov_model_instance);

                tokio::spawn(async move {
                    handle_generate_text_task(task, client_clone, model_clone).await;
                });
            }
            Err(e) => {
                warn!(
                    "[TASK_DESERIALIZE_FAIL] Failed to deserialize GenerateTextTask: {}. Payload: {}",
                    e,
                    String::from_utf8_lossy(&message.payload)
                );
            }
        }
    }

    info!("[NATS_LOOP_END] Subscription ended or NATS connection lost.");
    Ok(())
}
