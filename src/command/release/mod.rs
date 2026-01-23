// src/command/release/mod.rs - Release command orchestration

mod version;
mod tag;

use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::color::{arrow, success, warning};
use crate::git::{self, build_diff_target, get_current_version, get_diff};
use crate::context::load_all_context;
use crate::context::secret::SecretAction;
use crate::context::template::{changelog_system_with_context, version_system_with_context, CHANGELOG_USER, VERSION_USER};
use crate::command::{apply_smart_diff, SHORT_HASH_LEN};

use version::{detect_version_files, update_version_file, VersionFile};
use tag::{create_tag, get_commits_since, get_latest_tag, tag_exists};

// Re-exports for external use
pub use self::tag::CommitInfo;

// =============================================================================
// MAIN COMMAND
// =============================================================================

pub async fn cmd_release(
    client: &LlmClient,
    apply: bool,
    skip_changelog: bool,
    from: Option<String>,
    bump: String,
    to: Option<String>,
    algo: u8,
    base_branch: &str,
    stream: bool,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<()> {
    println!("===========================================================");
    println!("Gitar Release Workflow");
    println!("===========================================================\n");

    // Step 1: Determine the starting point
    let from_ref = match from {
        Some(r) => r,
        None => {
            match get_latest_tag()? {
                Some(tag) => {
                    println!("Starting from latest tag: {}", tag);
                    tag
                }
                None => {
                    println!("No tags found. Analyzing all commits from initial commit.");
                    // Get the first commit
                    let first_commit = git::run_git(&["rev-list", "--max-parents=0", "HEAD"])?
                        .trim()
                        .to_string();
                    first_commit
                }
            }
        }
    };

    // Step 2: Get commits since the starting point
    let commits = get_commits_since(&from_ref, None)?;

    if commits.is_empty() {
        println!("No new commits since {}.", from_ref);
        println!("Nothing to release.");
        return Ok(());
    }

    println!("Found {} new commit(s) since {}\n", commits.len(), from_ref);

    // Step 3: Analyze commits and suggest version bump
    let version_bump = if bump == "auto" {
        // Use LLM to analyze the diff and suggest version bump
        println!("Analyzing changes with LLM to suggest version bump...\n");
        suggest_version_bump_llm(
            client,
            &from_ref,
            to.as_deref(),
            base_branch,
            stream,
            algo,
            max_diff_chars,
            secret_action,
        )
        .await?
    } else {
        // Use explicit bump strategy
        validate_bump_strategy(&bump)?;
        bump.clone()
    };
    println!("Suggested version bump: {}", version_bump);

    // Step 4: Detect version files
    let version_files = detect_version_files()?;

    if version_files.is_empty() {
        println!();
        warning("No version files detected (Cargo.toml, package.json, pyproject.toml)");
        println!("   Skipping version update step.");
    } else {
        println!("\nDetected version files:");
        for file in &version_files {
            println!("  {} (current: {})", file.path.display(), file.current_version);
        }
    }

    // Step 5: Determine new version
    let new_version = if let Some(first_file) = version_files.first() {
        compute_new_version(&first_file.current_version, &version_bump)?
    } else {
        // No version files, ask user for version
        print!("\nEnter release version (e.g., 1.0.0): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    if new_version.is_empty() {
        bail!("Version cannot be empty");
    }

    println!("\nNew version: {}", new_version);

    // Step 6: Check if tag already exists
    let tag_name = format!("v{}", new_version);
    if tag_exists(&tag_name)? {
        bail!("Tag {} already exists. Use a different version or delete the existing tag.", tag_name);
    }

    // Step 7: Generate changelog
    let changelog = if skip_changelog {
        println!("\nSkipping changelog generation (--skip-changelog)");
        format!("Release {}", new_version)
    } else {
        println!("\nGenerating changelog...");
        generate_changelog_content(&commits, &from_ref, &new_version, client, stream, algo, max_diff_chars, secret_action).await?
    };

    // Step 8: Display release plan
    println!("\n===========================================================");
    println!("Release Plan");
    println!("===========================================================");
    println!("Version     : {}", new_version);
    println!("Tag         : {}", tag_name);
    println!("Base        : {}", from_ref);
    println!("Commits     : {}", commits.len());
    if !version_files.is_empty() {
        println!("Files to update:");
        for file in &version_files {
            println!("  {} : {} {} {}", file.path.display(), file.current_version, arrow(), new_version);
        }
    }
    println!("\nChangelog:");
    println!("{}", changelog);
    println!("===========================================================\n");

    // Step 9: Execute or dry-run
    if !apply {
        println!("Dry run complete. Use --apply to execute release.");
        return Ok(());
    }

    // Step 10: User confirmation
    print!("Proceed with release? [y/N]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Release cancelled.");
        return Ok(());
    }

    // Step 11: Execute release
    execute_release(&version_files, &new_version, &tag_name, &changelog, apply)?;

    println!("\n===========================================================");
    success(format!("Release {} completed successfully!", new_version));
    println!("===========================================================");
    println!("\nNext steps:");
    println!("  1. Review the changes: git show {}", tag_name);
    println!("  2. Push the release: git push origin main {}", tag_name);
    println!("  3. Create GitHub release with the changelog");
    println!();

    Ok(())
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn validate_bump_strategy(bump: &str) -> Result<()> {
    match bump {
        "auto" | "major" | "minor" | "patch" => Ok(()),
        _ => bail!("Invalid bump strategy: {}. Must be one of: auto, major, minor, patch", bump),
    }
}

async fn suggest_version_bump_llm(
    client: &LlmClient,
    from_ref: &str,
    to: Option<&str>,
    base_branch: &str,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<String> {
    // Get the current version from version files
    let current = get_current_version();

    // Build diff target
    let diff_target = build_diff_target(Some(from_ref), to, base_branch);
    let diff_target_ref = if diff_target.is_empty() {
        None
    } else {
        Some(diff_target.as_str())
    };

    // Get the diff
    let raw_diff = get_diff(diff_target_ref, false, usize::MAX)?;

    if raw_diff.trim().is_empty() {
        return Ok("patch".to_string());
    }

    // Apply smart diff shaping
    let diff = apply_smart_diff(&raw_diff, max_diff_chars, false, algo, secret_action)?;

    // Build prompt
    let prompt = VERSION_USER
        .replace("{version}", &current)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = version_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    // Call LLM
    let response = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    }

    // Parse the response to extract bump type
    parse_version_bump_from_response(&response)
}

fn parse_version_bump_from_response(response: &str) -> Result<String> {
    let lower = response.to_lowercase();

    // Look for explicit mentions of version bump types
    if lower.contains("major") && (lower.contains("breaking") || lower.contains("recommend major")) {
        Ok("major".to_string())
    } else if lower.contains("minor") && (lower.contains("feature") || lower.contains("recommend minor")) {
        Ok("minor".to_string())
    } else if lower.contains("patch") {
        Ok("patch".to_string())
    } else {
        // Default to patch if unclear
        eprintln!("Warning: Could not parse version bump from LLM response. Defaulting to 'patch'.");
        eprintln!("LLM response: {}", response);
        Ok("patch".to_string())
    }
}

#[allow(dead_code)]
fn suggest_version_bump(commits: &[CommitInfo]) -> String {
    // Simple heuristic based on commit messages
    let has_breaking = commits.iter().any(|c| {
        c.subject.to_lowercase().contains("breaking")
            || c.subject.starts_with("!")
            || c.subject.contains("BREAKING CHANGE")
    });

    let has_feature = commits.iter().any(|c| {
        c.subject.to_lowercase().starts_with("feat")
            || c.subject.to_lowercase().starts_with("feature")
    });

    if has_breaking {
        "major".to_string()
    } else if has_feature {
        "minor".to_string()
    } else {
        "patch".to_string()
    }
}

fn compute_new_version(current: &str, bump: &str) -> Result<String> {
    // Parse version (supports X.Y.Z format)
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() != 3 {
        bail!("Current version '{}' is not in X.Y.Z format", current);
    }

    let major: u32 = parts[0].parse().context("Invalid major version")?;
    let minor: u32 = parts[1].parse().context("Invalid minor version")?;
    let patch: u32 = parts[2].parse().context("Invalid patch version")?;

    let (new_major, new_minor, new_patch) = match bump {
        "major" => (major + 1, 0, 0),
        "minor" => (major, minor + 1, 0),
        "patch" => (major, minor, patch + 1),
        _ => bail!("Invalid bump type: {}", bump),
    };

    Ok(format!("{}.{}.{}", new_major, new_minor, new_patch))
}

async fn generate_changelog_content(
    commits: &[CommitInfo],
    from_ref: &str,
    version: &str,
    client: &LlmClient,
    stream: bool,
    algo: u8,
    max_diff_chars: usize,
    secret_action: SecretAction,
) -> Result<String> {
    // Use LLM-powered changelog generation (same logic as cmd_changelog)
    let end = "HEAD";
    let display = format!("{}..{}", from_ref, end);

    // Build commit list with messages (same format as cmd_changelog)
    let commit_list = commits
        .iter()
        .map(|c| {
            format!(
                "- [{}] {}",
                &c.short_hash[..SHORT_HASH_LEN.min(c.short_hash.len())],
                c.subject
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Get combined diff for the range
    let raw_diff = get_diff(Some(&format!("{}..{}", from_ref, end)), false, usize::MAX)?;
    let diff = if raw_diff.trim().is_empty() {
        String::new()
    } else {
        apply_smart_diff(&raw_diff, max_diff_chars, false, algo, secret_action)?
    };

    // Build the prompt using the same template as cmd_changelog
    // Include version context for release notes
    let range_with_version = format!("{} (Release v{})", display, version);
    let prompt = CHANGELOG_USER
        .replace("{range}", &range_with_version)
        .replace("{count}", &commits.len().to_string())
        .replace("{commits}", &commit_list)
        .replace("{diff}", &diff);

    let (project_ctx, user_ctx) = load_all_context();
    let system = changelog_system_with_context(project_ctx.as_deref(), user_ctx.as_deref());

    // Call LLM to generate the changelog
    let changelog = client.chat(&system, &prompt, stream).await?;
    if stream {
        println!();
    }

    Ok(changelog)
}

fn execute_release(
    version_files: &[VersionFile],
    new_version: &str,
    tag_name: &str,
    changelog: &str,
    dry_run: bool,
) -> Result<()> {
    println!("\nExecuting release...\n");

    // Step 1: Update version files
    if !version_files.is_empty() {
        for file in version_files {
            update_version_file(file, new_version, dry_run)?;
        }

        // Step 2: Stage and commit version changes
        if !dry_run {
            for file in version_files {
                git::run_git(&["add", file.path.to_str().unwrap()])?;
            }

            let commit_message = format!("Release version {}", new_version);
            git::run_git(&["commit", "-m", &commit_message])?;
            success("Created release commit");
        }
    }

    // Step 3: Create annotated tag
    create_tag(tag_name, changelog, dry_run)?;

    Ok(())
}
