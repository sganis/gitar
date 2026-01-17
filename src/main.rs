// src/main.rs
mod cli;
mod client;
mod commands;
mod config;
mod diff;
mod git;
mod prompt;
mod providers;
mod types;

use anyhow::{bail, Result};
use clap::Parser;

use cli::{Cli, Commands};
use client::LlmClient;
use commands::*;
use config::{Config, ResolvedConfig};
use git::{get_default_branch, is_git_repo};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let file_config = Config::load();

    // Handle commands that don't need git or LLM client
    match &cli.command {
        Commands::Init => return cmd_init(&cli, &file_config),
        Commands::Config => return cmd_config(),
        Commands::Hook { command } => return cmd_hook(command.clone()),
        _ => {}
    }

    // All other commands require a git repo
    if !is_git_repo() {
        bail!("Not a git repository");
    }

    // Handle diff command (doesn't need LLM client)
    if let Commands::Diff {
        target,
        staged,
        max_chars,
        alg,
        stats,
        stats_only,
        compare,
    } = &cli.command
    {
        return cmd_diff(
            target.clone(),
            *staged,
            *max_chars,
            *alg,
            *stats,
            *stats_only,
            *compare,
        );
    }

    // Build config and LLM client for remaining commands
    let config = ResolvedConfig::new(
        cli.api_key.as_ref(),
        cli.model.as_ref(),
        cli.max_tokens,
        cli.temperature,
        cli.base_url.as_ref(),
        cli.provider.as_ref(),
        cli.base_branch.as_ref(),
        if cli.stream { Some(true) } else { None },
        &file_config,
        get_default_branch,
    );
    let client = LlmClient::new(&config)?;

    // Dispatch to command handlers
    match cli.command {
        Commands::Commit {
            push,
            all,
            tag,
            no_tag,
            write_to,
            silent,
            stream,
            alg,
        } => {
            let do_stream = config.stream || stream;
            cmd_commit(
                &client,
                push,
                all,
                tag && !no_tag,
                write_to,
                silent,
                do_stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Staged { alg } => {
            cmd_staged(&client, config.stream, alg, config.max_diff_chars).await?
        }

        Commands::Unstaged { alg } => {
            cmd_unstaged(&client, config.stream, alg, config.max_diff_chars).await?
        }

        Commands::History {
            from,
            to,
            since,
            until,
            limit,
            delay,
            alg,
        } => {
            cmd_history(
                &client,
                from,
                to,
                since,
                until,
                limit,
                delay,
                config.stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Pr {
            base,
            to,
            staged,
            alg,
        } => {
            cmd_pr(
                &client,
                base,
                to,
                &config.base_branch,
                staged,
                config.stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Changelog {
            from,
            to,
            since,
            until,
            limit,
            alg,
        } => {
            cmd_changelog(
                &client,
                from,
                to,
                since,
                until,
                limit,
                config.stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Explain {
            from,
            to,
            since,
            until,
            staged,
            alg,
        } => {
            cmd_explain(
                &client,
                from,
                to,
                since,
                until,
                &config.base_branch,
                staged,
                config.stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Version {
            base,
            to,
            current,
            alg,
        } => {
            cmd_version(
                &client,
                base,
                to,
                &config.base_branch,
                current,
                config.stream,
                alg,
                config.max_diff_chars,
            )
            .await?
        }

        Commands::Models => cmd_models(&client).await?,

        // Already handled above
        Commands::Init | Commands::Config | Commands::Hook { .. } | Commands::Diff { .. } => {
            unreachable!()
        }
    }

    Ok(())
}