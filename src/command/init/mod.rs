// src/command/init/mod.rs
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::client::LlmClient;
use crate::config::{normalize_provider, Config, ProviderConfig, DEFAULT_MAX_DIFF_CHARS};
use crate::context::preset::Preset;
use crate::git;
use crate::prompt;

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
                    Err(e) => {
                        println!("Warning: could not create {}: {}", project_ctx.display(), e)
                    }
                }
            }
            Err(e) => {
                println!(
                    "Warning: could not detect repo root; skipping .gitar/gitar.md ({})",
                    e
                );
            }
        }
    }

    Ok(())
}

// =============================================================================
// INTERACTIVE PROMPTS
// =============================================================================

fn select_provider(current: Option<&str>) -> Result<Option<String>> {
    let providers = vec!["openai", "claude", "gemini", "groq", "ollama"];

    // Find default index
    let default_idx = if let Some(curr) = current {
        providers.iter().position(|&p| p == curr).unwrap_or(0)
    } else {
        0 // openai
    };

    // Format providers (no unicode, just plain text)
    let mut formatted: Vec<String> = providers
        .iter()
        .map(|&p| {
            if current == Some(p) {
                format!("{} (current)", p)
            } else {
                p.to_string()
            }
        })
        .collect();

    // Add cancel option
    formatted.push("Cancel".to_string());

    let idx = prompt::select("Select provider", &formatted, default_idx)?;

    if idx == formatted.len() - 1 {
        Ok(None) // User cancelled
    } else {
        Ok(Some(providers[idx].to_string()))
    }
}

async fn select_model(
    provider: &str,
    api_key: &str,
    current_model: Option<&str>,
) -> Result<Option<String>> {
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

    println!("Found {} available models\n", models.len());

    // Show ALL models (dialoguer handles scrolling with arrow keys automatically)
    let mut display_models: Vec<String> = models
        .iter()
        .map(|m| {
            if current_model == Some(m.as_str()) {
                format!("{} (current)", m)
            } else {
                m.clone()
            }
        })
        .collect();

    // Add special options at the end
    display_models.push("(Enter custom model name)".to_string());
    display_models.push("Cancel".to_string());

    // Find default index - current model if it exists, otherwise first model
    let default_idx = if let Some(curr) = current_model {
        models.iter().position(|m| m == curr).unwrap_or(0)
    } else {
        0
    };

    let idx = prompt::select(
        "Select model (use arrow keys to scroll)",
        &display_models,
        default_idx,
    )?;

    if idx == display_models.len() - 1 {
        // Cancel
        Ok(None)
    } else if idx == display_models.len() - 2 {
        // Enter custom model
        let custom = prompt::input("Enter model name", None)?;
        if custom.is_empty() {
            Ok(None)
        } else {
            Ok(Some(custom))
        }
    } else {
        // Selected a model from the list
        Ok(Some(models[idx].clone()))
    }
}

// =============================================================================
// MAIN COMMAND
// =============================================================================

pub async fn cmd_init(cli: &Cli, file: &Config, show: bool) -> Result<()> {
    // Handle --show flag: display resolved configuration
    if show {
        return show_config(file);
    }

    // First: ensure context files exist (non-destructive)
    ensure_context_files()?;

    // Then: existing config init/update behavior for ~/.gitar.toml
    let mut config = file.clone();

    // Check if running in interactive mode (no CLI params provided AND stdin is a TTY)
    let wants_interactive = cli.provider.is_none()
        && cli.api_key.is_none()
        && cli.model.is_none()
        && cli.base_url.is_none()
        && !cli.stream
        && cli.max_tokens.is_none()
        && cli.temperature.is_none()
        && cli.preset.is_none()
        && cli.base_branch.is_none();

    let interactive = wants_interactive && prompt::is_interactive();

    if wants_interactive && !interactive {
        // User ran `gitar init` without args in non-TTY (e.g., test, script, CI)
        // Just create context files and exit successfully
        println!("Created context files. Use 'gitar init --provider <provider> ...' to configure settings.");
        return Ok(());
    }

    if interactive {
        println!("==========================================================");
        println!("Gitar Interactive Configuration");
        println!("==========================================================\n");

        // Step 1: Select provider
        let selected_provider = match select_provider(config.default_provider.as_deref())? {
            Some(p) => p,
            None => {
                println!("Configuration cancelled.");
                return Ok(());
            }
        };
        println!("Selected provider: {}", selected_provider);

        // Step 2: Get API key (skip for providers that don't need it)
        let needs_api_key = !matches!(selected_provider.as_str(), "ollama" | "local");

        let provider_config = config.get_provider_mut(&selected_provider);
        let current_key = provider_config.api_key.clone();

        let api_key = if !needs_api_key {
            // Providers like ollama don't need API keys
            println!("\n[INFO] {} doesn't require an API key", selected_provider);
            String::new()
        } else if let Some(ref key) = current_key {
            let masked = format!(
                "{}...{}",
                &key[..4.min(key.len())],
                &key[key.len().saturating_sub(4)..]
            );
            let input = prompt::input(&format!("API key (current: {})", masked), Some(""))?;
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
                        if prompt::confirm("Use this key?", true)? {
                            env_key
                        } else {
                            prompt::input("Enter API key", None)?
                        }
                    } else {
                        prompt::input("Enter API key", None)?
                    }
                } else {
                    prompt::input(
                        &format!("Enter API key (or set ${} env var)", env_var),
                        None,
                    )?
                }
            } else {
                prompt::input("Enter API key", None)?
            }
        };

        // Validate API key for providers that need it
        if api_key.is_empty() && needs_api_key {
            bail!(
                "API key cannot be empty for provider: {}",
                selected_provider
            );
        }

        // Step 3: Select model (query API)
        let current_model = provider_config.model.as_deref();
        let selected_model = match select_model(&selected_provider, &api_key, current_model).await?
        {
            Some(m) => Some(m),
            None => {
                println!("Configuration cancelled.");
                return Ok(());
            }
        };

        // Update config
        let provider_config = config.get_provider_mut(&selected_provider);
        provider_config.api_key = Some(api_key);
        if let Some(model) = selected_model {
            provider_config.model = Some(model);
        }
        config.default_provider = Some(selected_provider.clone());

        // Step 4: Optional preset
        let presets = vec![
            "auto (detect from project)",
            "rust",
            "javascript",
            "python",
            "conventional",
            "none",
            "Cancel",
        ];

        let preset_idx = prompt::select("Commit message style preset", &presets, 0)?;
        if preset_idx == presets.len() - 1 {
            println!("Configuration cancelled.");
            return Ok(());
        }

        let preset = match preset_idx {
            0 => "auto",
            1 => "rust",
            2 => "javascript",
            3 => "python",
            4 => "conventional",
            5 => "none",
            _ => "auto",
        };
        config.preset = Some(preset.to_string());

        config.save()?;

        println!("\n==========================================================");
        println!("Configuration saved successfully!");
        println!("==========================================================");
        println!("\nProvider: {}", selected_provider);
        if let Some(model) = config
            .get_provider(&selected_provider)
            .and_then(|p| p.model.as_ref())
        {
            println!("Model: {}", model);
        }
        println!("Preset: {}", preset);
        println!("\nYou can now use gitar commands like:");
        println!("  gitar plan           # Plan commits (or just: gitar)");
        println!("  gitar tell --commit  # Generate commit message");
        println!("  gitar tell           # Explain changes for stakeholders");
        println!("  gitar fix            # Resolve merge conflicts");
        println!("  gitar release        # Create a release");
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

// =============================================================================
// SHOW CONFIG
// =============================================================================

fn show_config(config: &Config) -> Result<()> {
    let path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());

    // Detect preset for display
    let detected_preset = if git::is_git_repo() {
        git::get_repo_root()
            .map(|root| Preset::detect(&PathBuf::from(root)))
            .ok()
    } else {
        None
    };

    let effective_preset = config
        .preset
        .as_ref()
        .and_then(|s| Preset::from_str(s))
        .or(detected_preset)
        .unwrap_or(Preset::Default);

    println!("Config file: {}\n", path);
    println!(
        "default_provider: {}",
        config.default_provider.as_deref().unwrap_or("(not set)")
    );
    println!(
        "base_branch:      {}",
        config.base_branch.as_deref().unwrap_or("(not set)")
    );
    println!(
        "max_diff_chars:   {}",
        config
            .max_diff_chars
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("(default: {})", DEFAULT_MAX_DIFF_CHARS))
    );

    // Show preset with detection info
    let preset_display = match &config.preset {
        Some(p) => format!("{} (configured)", p),
        None => format!("{} (auto-detected)", effective_preset.name()),
    };
    println!("preset:           {}", preset_display);

    let providers = [
        ("openai", &config.openai, "OPENAI_API_KEY"),
        ("claude", &config.claude, "ANTHROPIC_API_KEY"),
        ("gemini", &config.gemini, "GEMINI_API_KEY"),
        ("groq", &config.groq, "GROQ_API_KEY"),
        ("ollama", &config.ollama, "(none)"),
    ];

    for (name, pc, env_var) in providers {
        if let Some(p) = pc {
            println!("\n[{}]", name);
            println!(
                "  api_key:     {}",
                p.api_key
                    .as_deref()
                    .map(|k| format!("{}...", &k[..8.min(k.len())]))
                    .unwrap_or_else(|| format!("(env: {})", env_var))
            );
            println!(
                "  model:       {}",
                p.model.as_deref().unwrap_or("(default)")
            );
            println!(
                "  max_tokens:  {}",
                p.max_tokens
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "(default)".into())
            );
            println!(
                "  temperature: {}",
                p.temperature
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "(default)".into())
            );
            println!(
                "  stream:      {}",
                p.stream
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(default: false)".into())
            );
            if let Some(url) = &p.base_url {
                println!("  base_url:    {}", url);
            }
        }
    }

    Ok(())
}
