// src/commands/admin.rs
use anyhow::{bail, Context, Result};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::cli::{Cli, HookCommands, HOOK_SCRIPT};
use crate::client::LlmClient;
use crate::config::{normalize_provider, Config, DEFAULT_MAX_DIFF_CHARS};
use crate::git::get_git_dir;

pub fn cmd_hook(command: HookCommands) -> Result<()> {
    let git_dir =
        get_git_dir().context("Could not locate .git directory. Are you in a git repo?")?;
    let hook_path = git_dir.join("hooks").join("prepare-commit-msg");

    match command {
        HookCommands::Install => {
            if hook_path.exists() {
                let existing = fs::read_to_string(&hook_path).unwrap_or_default();
                if existing.contains("gitar-hook") {
                    println!("Gitar hook is already installed.");
                    return Ok(());
                }
                bail!(
                    "A prepare-commit-msg hook already exists at {:?}. Please back it up or delete it first.",
                    hook_path
                );
            }

            fs::write(&hook_path, HOOK_SCRIPT)?;

            #[cfg(unix)]
            {
                let mut perms = fs::metadata(&hook_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&hook_path, perms)?;
            }

            println!("Universal hook installed at {:?}", hook_path);
        }
        HookCommands::Uninstall => {
            if !hook_path.exists() {
                println!("No hook found to uninstall.");
                return Ok(());
            }

            let content = fs::read_to_string(&hook_path)?;
            if content.contains("gitar-hook") {
                fs::remove_file(&hook_path)?;
                println!("Hook uninstalled successfully.");
            } else {
                println!("The existing hook was not created by gitar. Manual removal required.");
            }
        }
    }
    Ok(())
}

pub fn cmd_init(cli: &Cli, file: &Config) -> Result<()> {
    let mut config = file.clone();

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

    config.save()?;

    if let Some(p) = &provider {
        if cli.provider.is_some() {
            println!("Default provider set to: {}", p);
        } else {
            println!("Updated provider: {}", p);
        }
    }

    Ok(())
}

pub fn cmd_config() -> Result<()> {
    let config = Config::load();
    let path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());

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

    println!("\nUsage: gitar --provider <n> [command]");
    println!("Priority: CLI args > provider config > env var > defaults");
    Ok(())
}

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