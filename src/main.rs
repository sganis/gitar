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
    if let Some(ref cmd) = cli.command {
        match cmd {
            Commands::Init => return cmd_init(&cli, &file_config).await,
            Commands::Config => return cmd_config(),
            Commands::Hook { install, uninstall } => {
                let action = if *install {
                    HookAction::Install
                } else if *uninstall {
                    HookAction::Uninstall
                } else {
                    anyhow::bail!("Please specify --install or --uninstall")
                };
                return cmd_hook(action);
            }
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
        if cli.insecure { Some(true) } else { None },
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
            dispatch_plan(
                &client, &config, apply, fix, suggest, working, staged, history, to, interactive,
                yes, algo,
            )
            .await?
        }

        // Alias: run -> plan
        Commands::Run {
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
            dispatch_plan(
                &client, &config, apply, fix, suggest, working, staged, history, to, interactive,
                yes, algo,
            )
            .await?
        }

        Commands::Explain {
            commit,
            pr,
            changelog,
            history,
            report: _,
            reference,
            to,
            staged,
            since,
            until,
            limit,
            delay,
            algo,
            stream,
            push,
            all,
            amend,
            tag,
            no_tag,
            write_to,
            silent,
        } => {
            let do_stream = config.stream || stream;

            if commit {
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
            } else if pr {
                cmd_pr(
                    &client,
                    reference, // base ref
                    to,
                    &config.base_branch,
                    staged,
                    do_stream,
                    algo,
                    config.max_diff_chars,
                    config.secret_action,
                )
                .await?
            } else if changelog {
                cmd_changelog(
                    &client,
                    reference, // from ref
                    to,
                    since,
                    until,
                    limit,
                    do_stream,
                    algo,
                    config.max_diff_chars,
                    config.secret_action,
                )
                .await?
            } else if history {
                cmd_history(
                    &client,
                    config.preset,
                    reference, // from ref
                    to,
                    since,
                    until,
                    limit,
                    delay,
                    do_stream,
                    algo,
                    config.max_diff_chars,
                    config.secret_action,
                )
                .await?
            } else {
                // Default: report
                cmd_explain(
                    &client,
                    reference, // from ref
                    to,
                    since,
                    until,
                    &config.base_branch,
                    staged,
                    do_stream,
                    algo,
                    config.max_diff_chars,
                    config.secret_action,
                )
                .await?
            }
        }

        // Alias: commit -> explain --commit
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

        // Alias: pr -> explain --pr
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

        // Alias: changelog -> explain --changelog
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

        // Alias: history -> explain --history
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

        // Alias: resolve -> fix
        Commands::Resolve { apply, yes, stream } => {
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
        Commands::Init
        | Commands::Config
        | Commands::Hook { .. }
        | Commands::Diff { .. } => unreachable!()
    }

    Ok(())
}

async fn dispatch_plan(
    client: &LlmClient,
    config: &ResolvedConfig,
    apply: bool,
    fix: bool,
    suggest: bool,
    working: bool,
    staged: bool,
    history: Option<String>,
    to: Option<String>,
    interactive: bool,
    yes: bool,
    algo: u8,
) -> Result<()> {
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
        client,
        config,
        analysis_mode,
        apply,
        fix,
        suggest,
        is_interactive,
        algo,
    )
    .await
}
