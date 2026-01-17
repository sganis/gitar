// src/commands/diff.rs
use anyhow::Result;

use crate::diff::{get_llm_diff_preview, DiffAlg};
use crate::git::{get_diff, get_diff_stats};

pub fn cmd_diff(
    target: Option<String>,
    staged: bool,
    max_chars: usize,
    alg: Option<u8>,
    include_stats: bool,
    stats_only: bool,
    compare: bool,
) -> Result<()> {
    let raw_diff = if staged {
        get_diff(None, true, usize::MAX)?
    } else {
        get_diff(target.as_deref(), false, usize::MAX)?
    };

    if raw_diff.trim().is_empty() {
        println!("No changes to show.");
        return Ok(());
    }

    let diff_stats = if include_stats || alg.is_some() || compare {
        Some(get_diff_stats(target.as_deref(), staged)?)
    } else {
        None
    };

    if compare {
        println!("================================================================");
        println!("                     ALGORITHM COMPARISON                      ");
        println!("================================================================\n");

        for alg_num in 1..=4u8 {
            let algorithm = DiffAlg::from_num(alg_num);
            let (output, stats) =
                get_llm_diff_preview(&raw_diff, diff_stats.as_deref(), max_chars, algorithm, true);

            println!("{}", stats.display());

            if !stats_only {
                println!("{}", output);
            }
        }

        println!("Algorithms:");
        println!("  --alg 1  Full: complete git diff (ignores --max-chars)");
        println!("  --alg 2  Files: selective files, ranked by priority (default)");
        println!("  --alg 3  Hunks: selective hunks, ranked by importance");
        println!("  --alg 4  Semantic: JSON IR with scored hunks");
        return Ok(());
    }

    // If --alg is specified, use that algorithm and show stats
    if let Some(alg_num) = alg {
        let algorithm = DiffAlg::from_num(alg_num);
        let (output, stats) =
            get_llm_diff_preview(&raw_diff, diff_stats.as_deref(), max_chars, algorithm, false);

        println!("{}\n", stats.display());

        if !stats_only {
            println!("{}", output);
        }
    } else {
        // No --alg specified: just show raw diff (or with stats if requested)
        if let Some(ref stats) = diff_stats {
            println!("=== diff --stat ===\n{}\n", stats);
        }

        if !stats_only {
            let truncated = if raw_diff.len() > max_chars {
                &raw_diff[..max_chars]
            } else {
                &raw_diff
            };
            println!("{}", truncated);
            if raw_diff.len() > max_chars {
                println!("\n[... truncated at {} chars ...]", max_chars);
            }
        }
    }

    Ok(())
}