// src/command/init.rs
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::config::{normalize_provider, Config};
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

pub fn cmd_init(cli: &Cli, file: &Config) -> Result<()> {
    // First: ensure context files exist (non-destructive)
    ensure_context_files()?;

    // Then: existing config init/update behavior for ~/.gitar.toml
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
