// src/command/init/mod.rs
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::Cli;
use crate::client::LlmClient;
use crate::config::{
    normalize_provider, provider_to_url, Config, ProviderConfig, DEFAULT_MAX_DIFF_CHARS,
    CONFIG_PATH_ENV,
};
use crate::context::preset::Preset;
use crate::git;
use crate::prompt;

const USER_CONTEXT_TEMPLATE: &str = r#"<!-- Gitar User Context
Everything OUTSIDE HTML comment blocks is sent to the LLM.
Uncomment sections below.
Run "gitar init --show" to verify what gets sent.
-->

<!--
## About Me
I work in the ___ team as a ___.
My main projects are ___.

## My Stakeholders
- Customers: ___
- Team: ___
- Product: ___
- Leadership: ___

## Preferences
- Commit style: conventional commits, imperative mood
- Weekly reports: focus on customer impact and deliverables
- Tone: professional, concise

-->
"#;

const PROJECT_CONTEXT_TEMPLATE: &str = r#"<!-- Gitar Project Context

Everything OUTSIDE HTML comment blocks is sent to the LLM.
Uncomment sections below.
This file should be committed to the repo so the whole team shares it.

## Project
Brief description of what this repo does.

## Conventions
- Commit style: imperative mood, max 72 chars
- PR style: include ticket number

## Glossary
- Domain-specific terms used in commits/PRs

-->
"#;

const USER_PROMPTS_TEMPLATE: &str = r#"# Gitar Custom Prompts
#
# Override default LLM prompts here. Uncomment sections you want to customize.
# Each section has a system prompt and a user prompt.
# Missing sections use built-in defaults.
#
# Placeholders (required in user prompts):
#   commit:    {diff}
#   history:   {diff}, {original_message}
#   pr:        {branch}, {commits}, {stats}, {diff}
#   changelog: {range}, {count}, {commits}
#   explain:   {stats}, {diff}
#   version:   {version}, {diff}
#   weekly:    {stats}, {diff}

# [commit]
# system = """
# You generate clear and informative Git commit messages from diffs.
# Rules:
# 1. Focus on PURPOSE, not file listings
# 2. Ignore build/minified files
# 3. No markdown. Use plain ASCII characters only.
# 4. Be specific
# """
# user = """
# Generate a commit message in a single-line.
# ```
# {diff}
# ```
# Respond with ONLY the commit message.
# """

# [history]
# system = """
# You are an expert software engineer who writes clear, informative Git commit messages.
# """
# user = """
# Generate a commit message for this diff.
# **Original message:** {original_message}
# **Diff:**
# ```
# {diff}
# ```
# """

# [pr]
# system = """
# Write a PR description.
# Use plain ASCII characters only.
# """
# user = """
# Generate PR description.
# **Branch:** {branch}
# **Commits:** {commits}
# **Stats:** {stats}
# **Diff:**
# ```
# {diff}
# ```
# """

# [changelog]
# system = """
# Create release notes. Use plain ASCII characters only.
# """
# user = """
# Generate release notes.
# **Range:** {range}
# **Count:** {count}
# **Commits:** {commits}
# """

# [explain]
# system = """
# Explain code changes to non-technical stakeholders.
# No jargon, focus on user impact, be brief.
# """
# user = """
# Explain for non-technical person.
# **Stats:** {stats}
# **Diff:**
# ```
# {diff}
# ```
# """

# [version]
# system = """
# Recommend semantic version bump (major/minor/patch).
# """
# user = """
# Recommend version bump.
# **Current:** {version}
# **Diff:**
# ```
# {diff}
# ```
# """

# [weekly]
# system = """
# Generate a Weekly Highlights Report for senior leadership.
# """
# user = """
# Generate the Weekly Highlights report.
# **Stats:** {stats}
# **Diff:**
# ```
# {diff}
# ```
# """
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

fn ensure_context_files() -> Result<()> {
    // 1) User context: ~/.gitar/context.md
    if let Some(base) = Config::config_base_dir() {
        let user_ctx = base.join("context.md");
        match write_file_if_missing(&user_ctx, USER_CONTEXT_TEMPLATE) {
            Ok(true) => println!("Created user context: {}", user_ctx.display()),
            Ok(false) => {}
            Err(e) => println!("Warning: could not create {}: {}", user_ctx.display(), e),
        }

        // Create prompts.toml template
        let prompts_path = base.join("prompts.toml");
        match write_file_if_missing(&prompts_path, USER_PROMPTS_TEMPLATE) {
            Ok(true) => println!("Created prompts template: {}", prompts_path.display()),
            Ok(false) => {}
            Err(e) => println!("Warning: could not create {}: {}", prompts_path.display(), e),
        }
    } else {
        println!("Warning: could not determine config directory; skipping user config files");
    }

    // 2) Project context: <repo-root>/.gitar/context.md (only if in a git repo)
    if git::is_git_repo() {
        match git::get_repo_root() {
            Ok(root) => {
                let project_ctx = PathBuf::from(&root).join(".gitar").join("context.md");
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
                    "Warning: could not detect repo root; skipping .gitar/context.md ({})",
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
    base_url: Option<&str>,
    current_model: Option<&str>,
) -> Result<Option<String>> {
    println!("\nFetching available models from {}...", provider);

    // Create a temporary config with the provider settings
    let mut temp_config = ProviderConfig::default();
    temp_config.api_key = Some(api_key.to_string());
    if let Some(url) = base_url {
        temp_config.base_url = Some(url.to_string());
    }

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

pub async fn cmd_init(cli: &Cli, file: &Config, show: bool, auto_apply: Option<bool>) -> Result<()> {
    // Handle --show flag: display resolved configuration
    if show {
        return show_config(file);
    }

    // Handle --auto-apply flag (non-interactive shortcut)
    if let Some(value) = auto_apply {
        let mut config = file.clone();
        config.auto_apply = Some(value);
        config.save()?;
        println!(
            "auto_apply set to: {} (commands will {} by default)",
            value,
            if value { "execute" } else { "dry-run" }
        );
        return Ok(());
    }

    // First: ensure context files exist (non-destructive)
    ensure_context_files()?;

    // Then: existing config init/update behavior for ~/.gitar/gitar.toml
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
        let default_url = provider_to_url(&selected_provider);
        if let Some(url) = default_url {
            println!("Base URL: {}", url);
        }

        // For ollama, allow customizing the base URL
        let custom_base_url = if selected_provider == "ollama" {
            let current_url = config
                .get_provider(&selected_provider)
                .and_then(|p| p.base_url.clone());
            let default = current_url
                .as_deref()
                .or(default_url)
                .unwrap_or("http://localhost:11434/v1");
            let input = prompt::input(
                &format!("Base URL (press Enter for {})", default),
                Some(""),
            )?;
            if input.is_empty() {
                None // Use default, don't store in config
            } else {
                Some(input)
            }
        } else {
            None
        };

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
        let effective_base_url =
            custom_base_url.as_deref().or(provider_config.base_url.as_deref());
        let selected_model = match select_model(
            &selected_provider,
            &api_key,
            effective_base_url,
            current_model,
        )
        .await?
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
        if let Some(url) = custom_base_url {
            provider_config.base_url = Some(url);
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

        // Step 5: Auto-apply mode
        let current_auto_apply = config.auto_apply.unwrap_or(false);
        let auto_apply_prompt = if current_auto_apply {
            "Enable auto-apply mode? (commands execute instead of dry-run) [current: enabled]"
        } else {
            "Enable auto-apply mode? (commands execute instead of dry-run)"
        };
        config.auto_apply = Some(prompt::confirm(auto_apply_prompt, current_auto_apply)?);

        config.save()?;

        println!("\n==========================================================");
        println!("Configuration saved successfully!");
        println!("==========================================================");
        println!("\nProvider: {}", selected_provider);
        if let Some(base_url) = config
            .get_provider(&selected_provider)
            .and_then(|p| p.base_url.as_ref())
        {
            println!("Base URL: {}", base_url);
        }
        if let Some(model) = config
            .get_provider(&selected_provider)
            .and_then(|p| p.model.as_ref())
        {
            println!("Model: {}", model);
        }
        println!("Preset: {}", preset);
        println!(
            "Auto-apply: {}",
            if config.auto_apply.unwrap_or(false) {
                "enabled (commands execute by default)"
            } else {
                "disabled (commands dry-run by default)"
            }
        );
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
    // Show system config path if set
    if let Some(system_path) = Config::system_path() {
        let exists = system_path.exists();
        println!(
            "System config: {} {}",
            system_path.display(),
            if exists { "" } else { "(not found)" }
        );
    } else {
        println!(
            "System config: (not set, use ${} to configure)",
            CONFIG_PATH_ENV
        );
    }

    let user_path = Config::path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unknown)".into());
    let user_exists = Config::path().is_some_and(|p| p.exists());
    println!(
        "User config:   {} {}",
        user_path,
        if user_exists { "" } else { "(not found)" }
    );
    println!();

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

    println!("Resolved configuration (system + user merged):");
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

    // Show auto_apply
    let auto_apply_display = match config.auto_apply {
        Some(true) => "true (commands execute by default)",
        Some(false) => "false (commands dry-run by default)",
        None => "false (default: dry-run)",
    };
    println!("auto_apply:       {}", auto_apply_display);

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

    // Show context files
    println!("\n--- Context Files ---");

    // User context
    if let Some(base) = Config::config_base_dir() {
        let ctx_path = base.join("context.md");
        if ctx_path.exists() {
            println!("User context:    {}", ctx_path.display());
        } else {
            println!("User context:    (not found)");
        }
    }

    // Project context
    if git::is_git_repo() {
        if let Ok(root) = git::get_repo_root() {
            let ctx_path = PathBuf::from(&root).join(".gitar").join("context.md");
            if ctx_path.exists() {
                println!("Project context: {}", ctx_path.display());
            } else {
                println!("Project context: (not found)");
            }
        }
    }

    // Show loaded context content preview
    use crate::context::repo::load_all_context;
    let (project_ctx, user_ctx) = load_all_context();

    if user_ctx.is_some() || project_ctx.is_some() {
        println!("\n--- Loaded Context (sent to LLM) ---");
    }

    if let Some(ctx) = &user_ctx {
        let preview = if ctx.len() > 200 {
            format!("{}...", &ctx[..200])
        } else {
            ctx.clone()
        };
        println!("\nUser context preview:\n{}", preview);
    }

    if let Some(ctx) = &project_ctx {
        let preview = if ctx.len() > 200 {
            format!("{}...", &ctx[..200])
        } else {
            ctx.clone()
        };
        println!("\nProject context preview:\n{}", preview);
    }

    if user_ctx.is_none() && project_ctx.is_none() {
        println!("\n(No context loaded - context.md files are empty or only contain comments)");
    }

    Ok(())
}
