// src/command/release/mod.rs - Release command orchestration

mod version;
mod tag;

use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::client::LlmClient;
use crate::git;

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
    _base_branch: &str,
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
    let version_bump = suggest_version_bump(&commits);
    println!("Suggested version bump: {}", version_bump);

    // Step 4: Detect version files
    let version_files = detect_version_files()?;

    if version_files.is_empty() {
        println!("\n⚠️  No version files detected (Cargo.toml, package.json, pyproject.toml)");
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
        generate_changelog_content(&commits, &from_ref, &new_version, client).await?
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
            println!("  {} : {} → {}", file.path.display(), file.current_version, new_version);
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
    println!("✓ Release {} completed successfully!", new_version);
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
    _client: &LlmClient,
) -> Result<String> {
    // For now, generate a simple changelog
    // In the future, this could call cmd_changelog or use LLM
    let mut changelog = format!("# Release {}\n\n", version);
    changelog.push_str(&format!("Changes since {}:\n\n", from_ref));

    for commit in commits {
        changelog.push_str(&format!("- {} ({})\n", commit.subject, commit.short_hash));
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
            println!("✓ Created release commit");
        }
    }

    // Step 3: Create annotated tag
    create_tag(tag_name, changelog, dry_run)?;

    Ok(())
}
