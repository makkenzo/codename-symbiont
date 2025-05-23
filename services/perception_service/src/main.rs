use shared_models::{PerceiveUrlTask, RawTextMessage, current_timestamp_ms};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[perception_service] Starting...");

    // TODO: Настроить подключение к NATS (из переменной окружения)
    // TODO: Подписаться на тему для получения PerceiveUrlTask
    // TODO: В цикле обрабатывать входящие задачи
    //       - Скрапить URL
    //       - Формировать RawTextMessage
    //       - Публиковать RawTextMessage

    let example_task = PerceiveUrlTask {
        url: "http://example.com".to_string(),
    };
    println!("[perception_service] Example task: {:?}", example_task);

    let example_raw_msg = RawTextMessage {
        id: uuid::Uuid::new_v4().to_string(),
        source_url: example_task.url.clone(),
        raw_text: "Example raw text.".to_string(),
        timestamp_ms: current_timestamp_ms(),
    };
    println!(
        "[perception_service] Example raw message: {:?}",
        example_raw_msg
    );

    println!("[perception_service] Exiting (stub).");
    Ok(())
}
