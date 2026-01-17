// src/commands/models.rs
use anyhow::Result;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use crate::client::LlmClient;


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