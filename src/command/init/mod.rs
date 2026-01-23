// src/command/init/mod.rs
use anyhow::{bail, Result};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::client::LlmClient;
use crate::config::{normalize_provider, Config, ProviderConfig};
use crate::git;

const USER_CONTEXT_TEMPLATE: &str = r#"# Gitar User Context

<!--
This file is read by gitar and injected into LLM system prompts as "User Context".

Purpose:
- Your personal preferences across all repos (tone, verbosity, style, defaults)

Rules:
- Keep it short. This content is sent to the LLM often.
- Do NOT put secrets here (tokens, passwords, private keys).
-->

<!--
## Preferences
- Prefer short, imperative commit messages
- Focus on purpose and impact, not file listings
- Avoid emojis/unicode

## Style
- Tone: neutral / professional
- Detail level: 1-2 lines unless complex

## Notes
- Anything else you want the assistant to remember
-->
"#;

const PROJECT_CONTEXT_TEMPLATE: &str = r#"# Gitar Project Context

<!--
This file is read by gitar and injected into LLM system prompts as "Project Context".

Purpose:
- Repo-specific conventions and rules (authoritative for this project)

Rules:
- Keep it current, like CONTRIBUTING.md.
- Do NOT put secrets here.
- This should be committed to the repo so the whole team shares it.
-->

<!--
## Project
(What is this repo about? 1-3 lines)

## Conventions
- Commit style:
- PR style:
- Changelog style:

## Build & Test
- Build:
- Test:
- Lint:

## Split Rules (important for `gitar split`)
- What must be committed together?
- What should never be in the same commit?
- Directories/features that are separate units:

## Sensitive Areas
- Paths that should be changed carefully:

## Glossary
- Domain terms used in commits/PRs:
-->
"#;

fn write_file_if_missing(path: &Path, content: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(true)
}

fn home_dir() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("HOME") {
        if !h.trim().is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    if let Ok(h) = std::env::var("USERPROFILE") {
        if !h.trim().is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    let drive = std::env::var("HOMEDRIVE").ok();
    let path = std::env::var("HOMEPATH").ok();
    match (drive, path) {
        (Some(d), Some(p)) if !d.trim().is_empty() && !p.trim().is_empty() => {
            Some(PathBuf::from(format!("{}{}", d, p)))
        }
        _ => None,
    }
}

fn ensure_context_files() -> Result<()> {
    // 1) User context: ~/.gitar/gitar.md
    if let Some(hd) = home_dir() {
        let user_ctx = hd.join(".gitar").join("gitar.md");
        match write_file_if_missing(&user_ctx, USER_CONTEXT_TEMPLATE) {
            Ok(true) => println!("Created user context: {}", user_ctx.display()),
            Ok(false) => {}
            Err(e) => println!("Warning: could not create {}: {}", user_ctx.display(), e),
        }
    } else {
        println!("Warning: could not determine home directory; skipping ~/.gitar/gitar.md");
    }

    // 2) Project context: <repo-root>/.gitar/gitar.md (only if in a git repo)
    if git::is_git_repo() {
        match git::get_repo_root() {
            Ok(root) => {
                let project_ctx = PathBuf::from(root).join(".gitar").join("gitar.md");
                match write_file_if_missing(&project_ctx, PROJECT_CONTEXT_TEMPLATE) {
                    Ok(true) => println!("Created project context: {}", project_ctx.display()),
                    Ok(false) => {}
                    Err(e) => println!("Warning: could not create {}: {}", project_ctx.display(), e),
                }
            }
            Err(e) => {
                println!("Warning: could not detect repo root; skipping .gitar/gitar.md ({})", e);
            }
        }
    }

    Ok(())
}

// =============================================================================
// INTERACTIVE PROMPTS
// =============================================================================

fn prompt_user(message: &str) -> Result<String> {
    print!("{}", message);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_with_default(message: &str, default: &str) -> Result<String> {
    let result = prompt_user(message)?;
    if result.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(result)
    }
}

fn select_provider(current: Option<&str>) -> Result<String> {
    println!("\nAvailable providers:");
    let providers = vec!["openai", "claude", "gemini", "groq", "ollama"];

    for (i, provider) in providers.iter().enumerate() {
        if current == Some(provider) {
            println!("  {}. {} (current)", i + 1, provider);
        } else {
            println!("  {}. {}", i + 1, provider);
        }
    }

    let default = current.unwrap_or("openai");
    let prompt = format!("\nSelect provider [1-{}] (default: {}): ", providers.len(), default);
    let choice = prompt_with_default(&prompt, "1")?;

    if let Ok(index) = choice.parse::<usize>() {
        if index > 0 && index <= providers.len() {
            return Ok(providers[index - 1].to_string());
        }
    }

    // Try to match by name
    let normalized = normalize_provider(&choice);
    if providers.contains(&normalized) {
        return Ok(normalized.to_string());
    }

    Ok(default.to_string())
}

async fn select_model(provider: &str, api_key: &str) -> Result<Option<String>> {
    println!("\nFetching available models from {}...", provider);

    // Create a temporary config with the provider settings
    let mut temp_config = ProviderConfig::default();
    temp_config.api_key = Some(api_key.to_string());

    // Create temp client
    let client = match LlmClient::new_with_provider(provider, &temp_config) {
        Ok(client) => client,
        Err(e) => {
            println!("Warning: Could not create client: {}", e);
            return Ok(None);
        }
    };

    // Fetch models
    let models = match client.list_models().await {
        Ok(m) => m,
        Err(e) => {
            println!("Warning: Could not fetch models: {}", e);
            println!("You can set the model manually later with: gitar init --provider {} --model <model-name>", provider);
            return Ok(None);
        }
    };

    if models.is_empty() {
        println!("No models found for provider {}", provider);
        return Ok(None);
    }

    println!("\nAvailable models:");
    for (i, model) in models.iter().enumerate().take(20) {
        println!("  {}. {}", i + 1, model);
    }

    if models.len() > 20 {
        println!("  ... and {} more", models.len() - 20);
    }

    let prompt = format!("\nSelect model number or enter model name (1-{}, or Enter for default): ", models.len().min(20));
    let choice = prompt_user(&prompt)?;

    if choice.is_empty() {
        return Ok(None);
    }

    if let Ok(index) = choice.parse::<usize>() {
        if index > 0 && index <= models.len() {
            return Ok(Some(models[index - 1].clone()));
        }
    }

    // User entered model name directly
    Ok(Some(choice))
}

// =============================================================================
// MAIN COMMAND
// =============================================================================

pub async fn cmd_init(cli: &Cli, file: &Config) -> Result<()> {
    // First: ensure context files exist (non-destructive)
    ensure_context_files()?;

    // Then: existing config init/update behavior for ~/.gitar.toml
    let mut config = file.clone();

    // Check if running in interactive mode (no CLI params provided)
    let interactive = cli.provider.is_none()
        && cli.api_key.is_none()
        && cli.model.is_none()
        && cli.base_url.is_none()
        && !cli.stream
        && cli.max_tokens.is_none()
        && cli.temperature.is_none()
        && cli.preset.is_none()
        && cli.base_branch.is_none();

    if interactive {
        println!("==========================================================");
        println!("Gitar Interactive Configuration");
        println!("==========================================================\n");

        // Step 1: Select provider
        let selected_provider = select_provider(config.default_provider.as_deref())?;
        println!("Selected provider: {}", selected_provider);

        // Step 2: Get API key
        let provider_config = config.get_provider_mut(&selected_provider);
        let current_key = provider_config.api_key.clone();

        let api_key = if let Some(ref key) = current_key {
            let masked = format!("{}...{}", &key[..4.min(key.len())], &key[key.len().saturating_sub(4)..]);
            let prompt = format!("\nAPI key (current: {}, Enter to keep): ", masked);
            let input = prompt_user(&prompt)?;
            if input.is_empty() {
                key.clone()
            } else {
                input
            }
        } else {
            let env_var = match selected_provider.as_str() {
                "openai" => "OPENAI_API_KEY",
                "claude" => "ANTHROPIC_API_KEY",
                "gemini" => "GEMINI_API_KEY",
                "groq" => "GROQ_API_KEY",
                _ => "",
            };

            // Check environment variable
            if !env_var.is_empty() {
                if let Ok(env_key) = std::env::var(env_var) {
                    if !env_key.is_empty() {
                        println!("\nFound API key in ${}", env_var);
                        let use_env = prompt_with_default("Use this key? [Y/n]: ", "y")?;
                        if use_env.to_lowercase() != "n" {
                            env_key
                        } else {
                            prompt_user("\nEnter API key: ")?
                        }
                    } else {
                        prompt_user("\nEnter API key: ")?
                    }
                } else {
                    prompt_user(&format!("\nEnter API key (or set ${} env var): ", env_var))?
                }
            } else {
                prompt_user("\nEnter API key: ")?
            }
        };

        if api_key.is_empty() {
            bail!("API key cannot be empty");
        }

        // Step 3: Select model (query API)
        let selected_model = select_model(&selected_provider, &api_key).await?;

        // Update config
        let provider_config = config.get_provider_mut(&selected_provider);
        provider_config.api_key = Some(api_key);
        if let Some(model) = selected_model {
            provider_config.model = Some(model);
        }
        config.default_provider = Some(selected_provider.clone());

        // Step 4: Optional preset
        println!("\nCommit message style preset:");
        println!("  1. auto (detect from project files)");
        println!("  2. rust");
        println!("  3. javascript");
        println!("  4. python");
        println!("  5. conventional (Conventional Commits)");
        println!("  6. none");

        let preset_choice = prompt_with_default("\nSelect preset [1-6] (default: auto): ", "1")?;
        let preset = match preset_choice.as_str() {
            "1" | "auto" => "auto",
            "2" | "rust" => "rust",
            "3" | "javascript" | "js" => "javascript",
            "4" | "python" | "py" => "python",
            "5" | "conventional" => "conventional",
            "6" | "none" => "none",
            _ => "auto",
        };
        config.preset = Some(preset.to_string());

        config.save()?;

        println!("\n==========================================================");
        println!("Configuration saved successfully!");
        println!("==========================================================");
        println!("\nProvider: {}", selected_provider);
        if let Some(model) = config.get_provider(&selected_provider).and_then(|p| p.model.as_ref()) {
            println!("Model: {}", model);
        }
        println!("Preset: {}", preset);
        println!("\nYou can now use gitar commands like:");
        println!("  gitar commit");
        println!("  gitar plan");
        println!("  gitar explain");
        println!();

        return Ok(());
    }

    // Non-interactive mode: use CLI args
    let provider = cli
        .provider
        .as_ref()
        .map(|p| normalize_provider(p).to_string())
        .or_else(|| {
            config
                .default_provider
                .as_ref()
                .map(|p| normalize_provider(p).to_string())
        });

    if let Some(ref p) = provider {
        let pc = config.get_provider_mut(p);
        if cli.api_key.is_some() {
            pc.api_key = cli.api_key.clone();
        }
        if cli.model.is_some() {
            pc.model = cli.model.clone();
        }
        if cli.max_tokens.is_some() {
            pc.max_tokens = cli.max_tokens;
        }
        if cli.temperature.is_some() {
            pc.temperature = cli.temperature;
        }
        if cli.base_url.is_some() {
            pc.base_url = cli.base_url.clone();
        }
        if cli.stream {
            pc.stream = Some(true);
        }

        if cli.provider.is_some() {
            config.default_provider = Some(p.clone());
        }
    } else if cli.stream
        || cli.api_key.is_some()
        || cli.model.is_some()
        || cli.max_tokens.is_some()
        || cli.temperature.is_some()
    {
        bail!("Please specify --provider when setting provider-specific options like --stream, --model, --api-key, etc.");
    }

    if cli.base_branch.is_some() {
        config.base_branch = cli.base_branch.clone();
    }

    // Handle preset: normalize aliases to canonical names
    if let Some(ref preset_str) = cli.preset {
        let lower = preset_str.to_lowercase();
        let normalized = match lower.as_str() {
            "rs" => "rust",
            "js" => "javascript",
            "py" => "python",
            "auto" | "default" => "auto",
            _ => &lower,
        };
        config.preset = Some(normalized.to_string());
    }

    config.save()?;

    if let Some(p) = &provider {
        if cli.provider.is_some() {
            println!("Default provider set to: {}", p);
        } else {
            println!("Updated provider: {}", p);
        }
    }

    if cli.preset.is_some() {
        println!(
            "Preset set to: {}",
            config.preset.as_deref().unwrap_or("auto")
        );
    }

    Ok(())
}
