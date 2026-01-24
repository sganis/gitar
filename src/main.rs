// src/main.rs
use anyhow::{bail, Result};
use clap::Parser;

use gitar::cli::{Cli, Commands};
use gitar::client::LlmClient;
use gitar::command::*;
use gitar::config::{Config, ResolvedConfig};
use gitar::git::{get_default_branch, get_repo_root_path, is_git_repo};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let file_config = Config::load();

    // Handle commands that don't need git or LLM client
    match &cli.command {
        Commands::Init => return cmd_init(&cli, &file_config).await,
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
        algo,
        stats,
        stats_only,
        compare,
    } = &cli.command
    {
        return cmd_diff(
            target.clone(),
            *staged,
            *max_chars,
            *algo,
            *stats,
            *stats_only,
            *compare,
        );
    }

    // Run command now requires LLM client (handled below with other commands)

    let repo_root_path = get_repo_root_path()?;

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
        if cli.insecure_tls { Some(true) } else { None },
        cli.preset.as_ref(),
        &file_config,
        get_default_branch,
        &repo_root_path,
    );
    let client = LlmClient::new(&config)?;

    // Dispatch to command handlers
    match cli.command {
        Commands::Commit {
            push,
            all,
            amend,
            tag,
            no_tag,
            write_to,
            silent,
            stream,
            algo,
        } => {
            let do_stream = config.stream || stream;
            cmd_commit(
                &client,
                config.preset,
                push,
                all,
                amend,
                tag && !no_tag,
                write_to,
                silent,
                do_stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Staged { algo } => {
            cmd_staged(
                &client,
                config.preset,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Unstaged { algo } => {
            cmd_unstaged(
                &client,
                config.preset,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::History {
            from,
            to,
            since,
            until,
            limit,
            delay,
            algo,
        } => {
            cmd_history(
                &client,
                config.preset,
                from,
                to,
                since,
                until,
                limit,
                delay,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Pr {
            base,
            to,
            staged,
            algo,
        } => {
            cmd_pr(
                &client,
                base,
                to,
                &config.base_branch,
                staged,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Changelog {
            from,
            to,
            since,
            until,
            limit,
            algo,
        } => {
            cmd_changelog(
                &client,
                from,
                to,
                since,
                until,
                limit,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Explain {
            from,
            to,
            since,
            until,
            staged,
            algo,
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
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Release {
            apply,
            skip_changelog,
            skip_changelog_file,
            changelog_file,
            from,
            bump,
            to,
            algo,
        } => {
            cmd_release(
                &client,
                apply,
                skip_changelog,
                skip_changelog_file,
                changelog_file,
                from,
                bump,
                to,
                algo,
                &config.base_branch,
                config.stream,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Resolve { apply, yes, stream } => {
            let do_stream = config.stream || stream;
            cmd_resolve(
                &client,
                apply,
                yes,
                do_stream,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Squash { target, algo } => {
            cmd_squash(
                &client,
                config.preset,
                target,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Rewrite { target, algo } => {
            cmd_rewrite(
                &client,
                config.preset,
                target,
                config.stream,
                algo,
                config.max_diff_chars,
                config.secret_action,
            )
            .await?
        }

        Commands::Run {
            apply,
            resolve,
            suggest,
            mode,
            from,
            to,
            interactive,
            yes,
            algo,
        } => {
            // Parse mode
            let analysis_mode = match mode.as_deref() {
                Some("working") => Some(AnalysisMode::WorkingTree),
                Some("staged") => Some(AnalysisMode::Staged),
                Some("history") => {
                    let from_ref = from
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("--from is required for history mode"))?;
                    Some(AnalysisMode::History {
                        from: from_ref.clone(),
                        to: to.clone(),
                    })
                }
                Some("auto") | None => None,
                Some(m) => bail!("Invalid mode: {}", m),
            };

            let is_interactive = interactive && !yes;

            cmd_run(
                &client,
                &config,
                analysis_mode,
                apply,
                resolve,
                suggest,
                is_interactive,
                algo,
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
