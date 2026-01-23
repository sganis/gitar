// src/command/models/mod.rs
use crate::client::LlmClient;
use anyhow::Result;

pub async fn cmd_models(client: &LlmClient) -> Result<()> {
    println!("Fetching available models...\n");
    let models = client.list_models().await?;

    if models.is_empty() {
        println!("No models found.");
    } else {
        println!("Available models:");
        for model in models {
            println!("  {}", model);
        }
    }
    Ok(())
}
