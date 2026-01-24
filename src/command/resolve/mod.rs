// src/command/resolve/mod.rs
use anyhow::{bail, Context, Result};
use std::fs;

use crate::client::LlmClient;
use crate::context::secret::SecretAction;
use crate::git;
use crate::prompt;

mod diff_preview;
mod git_helper;
mod heuristic;
mod llm;
mod parser;

pub use parser::{has_conflict_markers, ConflictInput, ConflictResolver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptMode {
    Auto,
    Ours,
    Theirs,
    Both,
}

impl AcceptMode {
    fn from_env() -> Self {
        // Temporary: avoids CLI churn
        // Values: auto|ours|theirs|both
        let v = std::env::var("GITAR_RESOLVE_ACCEPT").unwrap_or_default();
        let v = v.trim().to_lowercase();
        match v.as_str() {
            "ours" => Self::Ours,
            "theirs" => Self::Theirs,
            "both" => Self::Both,
            _ => Self::Auto,
        }
    }
}

// =============================================================================
// UX
// =============================================================================
//
// (prompt_yes_no removed - now using prompt::confirm)

// =============================================================================
// TEST HOOK (kept for your fake resolver integration test)
// =============================================================================

#[allow(dead_code)]
pub async fn cmd_resolve_with_resolver<R: ConflictResolver>(
    resolver: &R,
    apply: bool,
    yes: bool,
) -> Result<()> {
    let files = git::list_conflicted_files()?;
    if files.is_empty() {
        bail!("No conflicted files detected");
    }

    eprintln!("Conflicts detected: {}", files.len());

    for path in files {
        let base = git::read_index_stage(1, &path)?;
        let ours = git::read_index_stage(2, &path)?;
        let theirs = git::read_index_stage(3, &path)?;

        let working = fs::read_to_string(&path)
            .with_context(|| format!("Failed reading working file: {}", path))?;

        if !has_conflict_markers(&working) {
            eprintln!("- {}  (no markers found; skipping)", path);
            continue;
        }

        let regions = parser::build_regions(&working, 6)
            .with_context(|| format!("Failed parsing conflict markers in {}", path))?;

        let input = ConflictInput {
            path: path.clone(),
            base,
            ours,
            theirs,
            working: working.clone(),
            regions,
        };

        if !apply {
            eprintln!(
                "- {}  Preview: {} conflict region(s) ready to resolve (use --apply).",
                path,
                input.regions.len()
            );
            continue;
        }

        let proposed = resolver
            .resolve(&input)
            .with_context(|| format!("Resolver failed for {}", path))?;

        if has_conflict_markers(&proposed) {
            bail!(
                "Resolver output still contains conflict markers for {}",
                path
            );
        }

        if !yes {
            eprintln!("- {}  Proposed diff:", path);
            diff_preview::show_preview_diff(&path, &working, &proposed)?;
        }

        let do_apply = if yes {
            true
        } else {
            prompt::confirm(&format!("Apply resolution for {}?", path), false)?
        };
        if !do_apply {
            eprintln!("- {}  Skipped.", path);
            continue;
        }

        fs::write(&path, proposed.as_bytes())
            .with_context(|| format!("Failed writing resolved file: {}", path))?;

        let after = fs::read_to_string(&path).unwrap_or_default();
        if has_conflict_markers(&after) {
            bail!(
                "Resolved file still contains conflict markers after write: {}",
                path
            );
        }

        git_helper::git_add(&path)?;

        let conflicted = git_helper::git_conflicted_names()?;
        if conflicted.iter().any(|p| p == &path) {
            bail!("File still reported as conflicted after staging: {}", path);
        }

        let cached = git_helper::git_cached_names()?;
        if !cached.iter().any(|p| p == &path) {
            bail!("File was not staged after git add: {}", path);
        }

        eprintln!("- {}  Resolved and staged.", path);
    }

    Ok(())
}

// =============================================================================
// PUBLIC COMMAND (keeps existing signature)
// =============================================================================

/// Resolve merge/rebase/cherry-pick conflicts.
///
/// vNext:
/// - `GITAR_RESOLVE_ACCEPT=ours|theirs|both|auto` (no CLI change)
/// - If accept != auto, resolves without LLM.
/// - If accept == auto: heuristic first, else per-region LLM (fallback to full-file LLM).
/// - Interactive apply shows a diff preview (git diff --no-index)
pub async fn cmd_resolve(
    client: &LlmClient,
    apply: bool,
    yes: bool,
    stream: bool,
    _max_chars: usize,
    _secret_action: SecretAction,
) -> Result<()> {
    let accept = AcceptMode::from_env();

    let files = git::list_conflicted_files()?;
    if files.is_empty() {
        bail!("No conflicted files detected");
    }

    eprintln!("Conflicts detected: {}", files.len());

    for path in files {
        let base = git::read_index_stage(1, &path)?;
        let ours = git::read_index_stage(2, &path)?;
        let theirs = git::read_index_stage(3, &path)?;

        let working = fs::read_to_string(&path)
            .with_context(|| format!("Failed reading working file: {}", path))?;

        let marker_count = working.matches("<<<<<<<").count();
        eprintln!(
            "- {}  (markers: {}, base/ours/theirs: {}/{}/{}, accept: {:?})",
            path,
            marker_count,
            if base.is_some() { "yes" } else { "no" },
            if ours.is_some() { "yes" } else { "no" },
            if theirs.is_some() { "yes" } else { "no" },
            accept
        );

        if !has_conflict_markers(&working) {
            eprintln!("  Skipping: no conflict markers found in working file");
            continue;
        }

        let regions = parser::build_regions(&working, 6)
            .with_context(|| format!("Failed parsing conflict markers in {}", path))?;
        if regions.is_empty() {
            eprintln!("  Skipping: could not find conflict regions");
            continue;
        }

        let input = ConflictInput {
            path: path.clone(),
            base,
            ours,
            theirs,
            working: working.clone(),
            regions,
        };

        if !apply {
            eprintln!(
                "  Preview: {} conflict region(s). Use --apply to apply resolution.",
                input.regions.len()
            );
            continue;
        }

        let proposed = match accept {
            AcceptMode::Ours => {
                eprintln!("  Accept mode: ours (no LLM).");
                parser::resolve_working_by_accept_mode(&working, parser::AcceptRewrite::Ours)?
            }
            AcceptMode::Theirs => {
                eprintln!("  Accept mode: theirs (no LLM).");
                parser::resolve_working_by_accept_mode(&working, parser::AcceptRewrite::Theirs)?
            }
            AcceptMode::Both => {
                eprintln!("  Accept mode: both (no LLM).");
                parser::resolve_working_by_accept_mode(&working, parser::AcceptRewrite::Both)?
            }
            AcceptMode::Auto => {
                // Step 1: attempt full heuristic (fast path)
                let (heuristic_all, ok) = heuristic::resolve_working_by_heuristics(&working);
                if ok {
                    eprintln!("  Auto-resolved by heuristic (no LLM).");
                    heuristic_all
                } else {
                    // Step 2: per-region LLM with fallback to full-file
                    eprintln!("  Needs LLM: trying per-region resolution...");
                    let resolved = if yes {
                        llm::resolve_working_per_region_with_fallback(client, stream, &input).await
                    } else {
                        llm::resolve_working_per_region_interactive(client, stream, &input).await
                    };
                    resolved.with_context(|| format!("LLM resolution failed for {}", path))?
                }
            }
        };

        if has_conflict_markers(&proposed) {
            bail!(
                "Proposed output still contains conflict markers for {}",
                path
            );
        }

        if !yes {
            eprintln!("  Proposed diff:");
            diff_preview::show_preview_diff(&path, &working, &proposed)?;
        }

        let do_apply = if yes {
            true
        } else {
            prompt::confirm(&format!("  Apply resolution for {}?", path), false)?
        };

        if !do_apply {
            eprintln!("  Skipped.");
            continue;
        }

        fs::write(&path, proposed.as_bytes())
            .with_context(|| format!("Failed writing resolved file: {}", path))?;

        let after = fs::read_to_string(&path).unwrap_or_default();
        if has_conflict_markers(&after) {
            bail!(
                "Resolved file still contains conflict markers after write: {}",
                path
            );
        }

        git_helper::git_add(&path)?;

        let conflicted = git_helper::git_conflicted_names()?;
        if conflicted.iter().any(|p| p == &path) {
            bail!("File still reported as conflicted after staging: {}", path);
        }

        if git_helper::git_has_unmerged_entries(&path)? {
            bail!(
                "File still has unmerged index entries after git add: {}",
                path
            );
        }

        eprintln!("  Resolved and staged: {}", path);
    }

    Ok(())
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn accept_mode_from_env_default_auto() {
        std::env::remove_var("GITAR_RESOLVE_ACCEPT");
        assert_eq!(AcceptMode::from_env(), AcceptMode::Auto);
    }

    #[test]
    #[serial]
    fn accept_mode_from_env_ours() {
        std::env::set_var("GITAR_RESOLVE_ACCEPT", "ours");
        assert_eq!(AcceptMode::from_env(), AcceptMode::Ours);
    }

    #[test]
    #[serial]
    fn accept_mode_from_env_theirs() {
        std::env::set_var("GITAR_RESOLVE_ACCEPT", "theirs");
        assert_eq!(AcceptMode::from_env(), AcceptMode::Theirs);
    }

    #[test]
    #[serial]
    fn accept_mode_from_env_both() {
        std::env::set_var("GITAR_RESOLVE_ACCEPT", "both");
        assert_eq!(AcceptMode::from_env(), AcceptMode::Both);
    }

    #[test]
    fn parse_two_conflicts() {
        let s = "\
a
<<<<<<< ours
x
=======
y
>>>>>>> theirs
b
<<<<<<< ours2
m
=======
n
>>>>>>> theirs2
c
";
        let regions = parser::parse_conflict_regions_raw(s).unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].0, 2); // start line
        assert!(regions[0].4.contains("x"));
        assert!(regions[0].5.contains("y"));
    }

    #[test]
    fn heuristic_whitespace_only_prefers_ours() {
        let ours = "a = 1;\n";
        let theirs = "a=1;\n";
        let chosen = heuristic::choose_region_heuristic(ours, theirs).unwrap();
        assert_eq!(chosen, ours);
    }

    #[test]
    fn accept_ours_rewrites() {
        let s = "\
a
<<<<<<< ours
X
=======
Y
>>>>>>> theirs
b
";
        let out = parser::resolve_working_by_accept_mode(s, parser::AcceptRewrite::Ours).unwrap();
        assert_eq!(out, "a\nX\nb\n");
    }

    #[test]
    fn accept_theirs_rewrites() {
        let s = "\
a
<<<<<<< ours
X
=======
Y
>>>>>>> theirs
b
";
        let out = parser::resolve_working_by_accept_mode(s, parser::AcceptRewrite::Theirs).unwrap();
        assert_eq!(out, "a\nY\nb\n");
    }

    #[test]
    fn accept_both_rewrites() {
        let s = "\
a
<<<<<<< ours
X
=======
Y
>>>>>>> theirs
b
";
        let out = parser::resolve_working_by_accept_mode(s, parser::AcceptRewrite::Both).unwrap();
        assert_eq!(out, "a\nX\nY\nb\n");
    }
}
