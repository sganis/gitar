// src/main.rs
use anyhow::{bail, Result};
use clap::Parser;

use gitar::cli::{Cli, Commands, ExplainCommands};
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
    if let Some(ref cmd) = cli.command {
        match cmd {
            Commands::Init => return cmd_init(&cli, &file_config).await,
            Commands::Config => return cmd_config(),
            Commands::Hook { command } => return cmd_hook(command.clone()),
            _ => {}
        }
    }

    // All other commands require a git repo
    if !is_git_repo() {
        bail!("Not a git repository");
    }

    // Handle diff command (doesn't need LLM client)
    if let Some(Commands::Diff {
        target,
        staged,
        max_chars,
        algo,
        stats,
        stats_only,
        compare,
    }) = &cli.command
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

    // Handle default command (no subcommand = plan)
    let command = cli.command.unwrap_or(Commands::Plan {
        apply: false,
        fix: false,
        suggest: false,
        working: false,
        staged: false,
        history: None,
        to: None,
        interactive: true,
        yes: false,
        algo: 4,
    });

    // Dispatch to command handlers
    match command {
        Commands::Plan {
            apply,
            fix,
            suggest,
            working,
            staged,
            history,
            to,
            interactive,
            yes,
            algo,
        } => {
            // Determine analysis mode from scope flags
            let analysis_mode = if let Some(ref from_ref) = history {
                Some(AnalysisMode::History {
                    from: from_ref.clone(),
                    to: to.clone(),
                })
            } else if staged {
                Some(AnalysisMode::Staged)
            } else if working {
                Some(AnalysisMode::WorkingTree)
            } else {
                None // Auto-detect
            };

            let is_interactive = interactive && !yes;

            cmd_plan(
                &client,
                &config,
                analysis_mode,
                apply,
                fix,
                suggest,
                is_interactive,
                algo,
            )
            .await?
        }

        Commands::Explain { command } => match command {
            ExplainCommands::Commit {
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

            ExplainCommands::Staged { algo } => {
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

            ExplainCommands::Unstaged { algo } => {
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

            ExplainCommands::Pr {
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

            ExplainCommands::Changelog {
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

            ExplainCommands::History {
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

            ExplainCommands::Report {
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
        },

        Commands::Fix { apply, yes, stream } => {
            let do_stream = config.stream || stream;
            cmd_fix(
                &client,
                apply,
                yes,
                do_stream,
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

        Commands::Models => cmd_models(&client).await?,

        // Already handled above
        Commands::Init | Commands::Config | Commands::Hook { .. } | Commands::Diff { .. } => {
            unreachable!()
        }
    }

    Ok(())
}
