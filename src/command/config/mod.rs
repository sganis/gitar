// src/command/config/mod.rs
use anyhow::Result;

use crate::config::{Config, DEFAULT_MAX_DIFF_CHARS};
use crate::git;
use crate::context::preset::Preset;
use std::path::PathBuf;

pub fn cmd_config() -> Result<()> {
    let config = Config::load();
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
            println!("  model:       {}", p.model.as_deref().unwrap_or("(default)"));
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

    println!("\nPreset detection (if auto):");
    println!("  Cargo.toml       -> rust");
    println!("  package.json     -> javascript");
    println!("  pyproject.toml   -> python");
    println!("  setup.py         -> python");
    println!("  requirements.txt -> python");

    println!("\nUsage: gitar --provider <n> [--preset <p>] [command]");
    println!("Priority: CLI args > config file > auto-detect > defaults");
    Ok(())
}
