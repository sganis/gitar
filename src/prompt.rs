// src/prompts.rs

pub const HISTORY_SYSTEM_PROMPT: &str = r#"You are an expert software engineer who writes clear, informative Git commit messages.

## Commit Message Format
<Type>(<scope>):
<description line 1>
<description line 2 if needed>

## Types
- Feat: New feature
- Fix: Bug fix
- Refactor: Code restructuring without behavior change
- Docs: Documentation changes
- Style: Formatting, whitespace (no code logic change)
- Test: Adding or modifying tests
- Chore: Build process, dependencies, config
- Perf: Performance improvement

## Rules
1. First line: Type(scope): only, capitalized (no description on this line)
2. Following lines: describe WHAT changed and WHY
3. Scale detail to complexity: simple changes get 1-2 lines, complex changes get more
4. Use imperative mood ("Add" not "Added")
5. Be specific about impact and reasoning
6. Use plain ASCII characters only. Do not use emojis or Unicode symbols."#;

pub const HISTORY_USER_PROMPT: &str = r#"Generate a commit message for this diff.
First line: Type(scope): only (capitalized, nothing else on this line)
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message (if any):** {original_message}

**Diff:**
```
{diff}
```
Respond with ONLY the commit message (no markdown, no extra explanation)."#;

pub const COMMIT_SYSTEM_PROMPT: &str = r#"You generate clear and informative Git commit messages from diffs.

Rules:
1. Focus on PURPOSE, not file listings
2. Ignore build/minified files
3. No markdown. Use plain ASCII characters only. Do not use emojis or Unicode symbols. Do not use empty lines between lines.
4. Be specific

Examples:
"Add user authentication with OAuth2 support"
"Fix payment timeout with retry logic"
"Refactor database queries for connection pooling"
"#;

pub const COMMIT_USER_PROMPT: &str = r#"Generate a commit message in a single-line.
```
{diff}
```
Respond with ONLY the commit message. (single-line)"#;

pub const PR_SYSTEM_PROMPT: &str = r#"Write a PR description.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## Summary
Brief overview.

## What Changed
- Key changes

## Why
Motivation.

## Risks
- Issues or "None"

## Testing
- How tested

## Rollout
- Deploy notes or "Standard""#;

pub const PR_USER_PROMPT: &str = r#"Generate PR description.

**Branch:** {branch}
**Commits:**
{commits}

**Stats:**
{stats}

**Diff:**
```
{diff}
```
"#;

pub const CHANGELOG_SYSTEM_PROMPT: &str = r#"Create release notes.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
# Release Notes
## Features
## Fixes
## Improvements
## Breaking Changes
## Infrastructure

Group related changes, omit empty sections."#;

pub const CHANGELOG_USER_PROMPT: &str = r#"Generate release notes.

**Range:** {range}
**Count:** {count}

**Commits:**
{commits}"#;

pub const EXPLAIN_SYSTEM_PROMPT: &str = r#"Explain code changes to non-technical stakeholders.
No jargon, focus on user impact, be brief.

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Format:
## What's Changing
Summary.

## User Impact
- Effects

## Risk Level
Low/Medium/High

## Actions
- QA needed"#;

pub const EXPLAIN_USER_PROMPT: &str = r#"Explain for non-technical person.

**Stats:**
{stats}

**Diff:**
```
{diff}
```"#;

pub const VERSION_SYSTEM_PROMPT: &str = r#"Recommend semantic version bump.
- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Fixes/refactors

Use plain ASCII characters only. Do not use emojis or Unicode symbols.

Output: Recommendation + Reasoning + Breaking: Yes/No"#;

pub const VERSION_USER_PROMPT: &str = r#"Recommend version bump.

**Current:** {version}
**Diff:**
```
{diff}
```"#;

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompts_not_empty() {
        assert!(!HISTORY_SYSTEM_PROMPT.is_empty());
        assert!(!COMMIT_SYSTEM_PROMPT.is_empty());
        assert!(!PR_SYSTEM_PROMPT.is_empty());
        assert!(!CHANGELOG_SYSTEM_PROMPT.is_empty());
        assert!(!EXPLAIN_SYSTEM_PROMPT.is_empty());
        assert!(!VERSION_SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn commit_system_prompt_contains_types() {
        assert!(HISTORY_SYSTEM_PROMPT.contains("Feat"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Fix"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Refactor"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Docs"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Style"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Test"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Chore"));
        assert!(HISTORY_SYSTEM_PROMPT.contains("Perf"));
    }

    #[test]
    fn prompts_disallow_emojis() {
        let prompts = [
            HISTORY_SYSTEM_PROMPT,
            COMMIT_SYSTEM_PROMPT,
            PR_SYSTEM_PROMPT,
            CHANGELOG_SYSTEM_PROMPT,
            EXPLAIN_SYSTEM_PROMPT,
            VERSION_SYSTEM_PROMPT,
        ];
        for prompt in prompts {
            assert!(
                prompt.contains("ASCII") || prompt.contains("emoji"),
                "Prompt should mention ASCII or emoji restriction"
            );
        }
    }

    #[test]
    fn version_prompt_contains_semver() {
        assert!(VERSION_SYSTEM_PROMPT.contains("MAJOR"));
        assert!(VERSION_SYSTEM_PROMPT.contains("MINOR"));
        assert!(VERSION_SYSTEM_PROMPT.contains("PATCH"));
    }

    #[test]
    fn pr_prompt_contains_sections() {
        assert!(PR_SYSTEM_PROMPT.contains("Summary"));
        assert!(PR_SYSTEM_PROMPT.contains("What Changed"));
        assert!(PR_SYSTEM_PROMPT.contains("Why"));
        assert!(PR_SYSTEM_PROMPT.contains("Risks"));
        assert!(PR_SYSTEM_PROMPT.contains("Testing"));
    }

    #[test]
    fn changelog_prompt_contains_sections() {
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Features"));
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Fixes"));
        assert!(CHANGELOG_SYSTEM_PROMPT.contains("Breaking Changes"));
    }

    #[test]
    fn commit_prompt_substitution() {
        let diff = "test diff";
        let original = "Original message";
        let prompt = HISTORY_USER_PROMPT
            .replace("{original_message}", original)
            .replace("{diff}", diff);
        assert!(prompt.contains("test diff"));
        assert!(prompt.contains("Original message"));
        assert!(!prompt.contains("{diff}"));
        assert!(!prompt.contains("{original_message}"));
    }

    #[test]
    fn pr_prompt_substitution() {
        let prompt = PR_USER_PROMPT
            .replace("{branch}", "feature/test")
            .replace("{commits}", "- commit 1\n- commit 2")
            .replace("{stats}", "2 files changed")
            .replace("{diff}", "diff content");
        assert!(prompt.contains("feature/test"));
        assert!(prompt.contains("- commit 1"));
        assert!(prompt.contains("2 files changed"));
        assert!(prompt.contains("diff content"));
        assert!(!prompt.contains("{branch}"));
        assert!(!prompt.contains("{commits}"));
        assert!(!prompt.contains("{stats}"));
        assert!(!prompt.contains("{diff}"));
    }

    #[test]
    fn changelog_prompt_substitution() {
        let prompt = CHANGELOG_USER_PROMPT
            .replace("{range}", "v1.0.0..HEAD")
            .replace("{count}", "10")
            .replace("{commits}", "- [abc123] Fix bug");
        assert!(prompt.contains("v1.0.0..HEAD"));
        assert!(prompt.contains("10"));
        assert!(prompt.contains("- [abc123] Fix bug"));
        assert!(!prompt.contains("{range}"));
        assert!(!prompt.contains("{count}"));
        assert!(!prompt.contains("{commits}"));
    }

    #[test]
    fn version_prompt_substitution() {
        let prompt = VERSION_USER_PROMPT
            .replace("{version}", "1.2.3")
            .replace("{diff}", "some changes");
        assert!(prompt.contains("1.2.3"));
        assert!(prompt.contains("some changes"));
        assert!(!prompt.contains("{version}"));
        assert!(!prompt.contains("{diff}"));
    }

    #[test]
    fn explain_prompt_substitution() {
        let prompt = EXPLAIN_USER_PROMPT
            .replace("{stats}", "5 files, +100 -50")
            .replace("{diff}", "diff here");
        assert!(prompt.contains("5 files, +100 -50"));
        assert!(prompt.contains("diff here"));
        assert!(!prompt.contains("{stats}"));
        assert!(!prompt.contains("{diff}"));
    }
}
