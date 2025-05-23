use std::{env, time::Duration};

use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    println!("Connecting to NATS server at {}...", nats_url);

    let client = async_nats::connect(&nats_url).await.map_err(Box::new)?;
    println!("Connected to NATS!");

    let subject = "greet.hello";
    let payload = "Hello from nats_tester!";

    let mut subscriber = client.subscribe(subject).await?;
    println!("Subscribed to subject: {}", subject);

    client.publish(subject, payload.into()).await?;
    println!("Published message: '{}' to subject: {}", payload, subject);

    if let Ok(Some(message)) = tokio::time::timeout(Duration::from_secs(5), subscriber.next()).await
    {
        println!(
            "Received message: '{}' on subject: {}",
            String::from_utf8_lossy(&message.payload),
            message.subject
        );
    } else {
        println!("Did not receive a message within the timeout.");
    }

    Ok(())
}
