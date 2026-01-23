// src/context/template.rs - LLM prompt templates

use super::preset::Preset;

// =============================================================================
// CONTEXT INJECTION (optional)
// =============================================================================

fn format_context(project_ctx: Option<&str>, user_ctx: Option<&str>) -> String {
    let mut out = String::new();

    if let Some(s) = user_ctx {
        let s = s.trim();
        if !s.is_empty() {
            out.push_str(
                r#"

User Context (~/.gitar/gitar.md; personal preferences; follow when relevant):
"#,
            );
            out.push_str(s);
            out.push_str(
                r#"

Rules:
- If user context conflicts with system rules, system rules win.
- If user context conflicts with project context, project context wins.
- If irrelevant, ignore it.
"#,
            );
        }
    }

    if let Some(s) = project_ctx {
        let s = s.trim();
        if !s.is_empty() {
            out.push_str(
                r#"

Project Context (.gitar/gitar.md; repo conventions; authoritative for this project):
"#,
            );
            out.push_str(s);
            out.push_str(
                r#"

Rules:
- If project context conflicts with system rules, system rules win.
- If irrelevant, ignore it.
"#,
            );
        }
    }

    out
}

// =============================================================================
// COMMIT MESSAGE
// =============================================================================

pub const COMMIT_SYSTEM: &str = r#"You generate clear and informative Git commit messages from diffs.

Rules:
1. Focus on PURPOSE, not file listings
2. Ignore build/minified files
3. No markdown. Use plain ASCII characters only. No emojis or Unicode. No empty lines.
4. Be specific

Examples:
"Add user authentication with OAuth2 support"
"Fix payment timeout with retry logic"
"Refactor database queries for connection pooling"
"#;

pub const COMMIT_USER: &str = r#"Generate a commit message in a single-line.
```
{diff}
```

pgsql
Copy code
Respond with ONLY the commit message. (single-line)"#;


pub fn commit_system_with_context(
    preset: Preset,
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let hints = preset.hints().format(preset.name());
    let ctx = format_context(project_context, user_context);
    format!("{}{}{}", COMMIT_SYSTEM, hints, ctx)
}

// =============================================================================
// HISTORY (multi-line commit messages)
// =============================================================================

pub const HISTORY_SYSTEM: &str = r#"You are an expert software engineer who writes clear, informative Git commit messages.

## Format
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
3. Scale detail to complexity
4. Use imperative mood ("Add" not "Added")
5. Use plain ASCII only. No emojis or Unicode."#;

pub const HISTORY_USER: &str = r#"Generate a commit message for this diff.
First line: Type(scope): only
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message:** {original_message}

**Diff:**
```
{diff}
```

makefile
Copy code
Respond with ONLY the commit message."#;


pub fn history_system_with_context(
    preset: Preset,
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let hints = preset.hints().format(preset.name());
    let ctx = format_context(project_context, user_context);
    format!("{}{}{}", HISTORY_SYSTEM, hints, ctx)
}

// =============================================================================
// PR DESCRIPTION
// =============================================================================

pub const PR_SYSTEM: &str = r#"Write a PR description.

Use plain ASCII characters only. No emojis or Unicode.

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

pub const PR_USER: &str = r#"Generate PR description.

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

pub fn pr_system_with_context(project_context: Option<&str>, user_context: Option<&str>) -> String {
    let ctx = format_context(project_context, user_context);
    format!("{}{}", PR_SYSTEM, ctx)
}

// =============================================================================
// CHANGELOG / RELEASE NOTES
// =============================================================================

pub const CHANGELOG_SYSTEM: &str = r#"Create release notes.

Use plain ASCII characters only. No emojis or Unicode.

Format:
# Release Notes
## Features
## Fixes
## Improvements
## Breaking Changes
## Infrastructure

Group related changes, omit empty sections."#;

pub const CHANGELOG_USER: &str = r#"Generate release notes.

**Range:** {range}
**Count:** {count}

**Commits:**
{commits}"#;

pub fn changelog_system_with_context(
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let ctx = format_context(project_context, user_context);
    format!("{}{}", CHANGELOG_SYSTEM, ctx)
}

// =============================================================================
// EXPLAIN (non-technical)
// =============================================================================

pub const EXPLAIN_SYSTEM: &str = r#"Explain code changes to non-technical stakeholders.
No jargon, focus on user impact, be brief.

Use plain ASCII characters only. No emojis or Unicode.

Format:
## What's Changing
Summary.

## User Impact
- Effects

## Risk Level
Low/Medium/High

## Actions
- QA needed"#;

pub const EXPLAIN_USER: &str = r#"Explain for non-technical person.

**Stats:**
{stats}

**Diff:**
```
{diff}
```

"#;

pub fn explain_system_with_context(project_context: Option<&str>, user_context: Option<&str>) -> String {
    let ctx = format_context(project_context, user_context);
    format!("{}{}", EXPLAIN_SYSTEM, ctx)
}

// =============================================================================
// VERSION BUMP
// =============================================================================

pub const VERSION_SYSTEM: &str = r#"Recommend semantic version bump.
- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Fixes/refactors

Use plain ASCII characters only. No emojis or Unicode.

Output: Recommendation + Reasoning + Breaking: Yes/No"#;

pub const VERSION_USER: &str = r#"Recommend version bump.

**Current:** {version}
**Diff:**
```
{diff}
```

"#;

pub fn version_system_with_context(project_context: Option<&str>, user_context: Option<&str>) -> String {
    let ctx = format_context(project_context, user_context);
    format!("{}{}", VERSION_SYSTEM, ctx)
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_not_empty() {
        assert!(!COMMIT_SYSTEM.is_empty());
        assert!(!COMMIT_USER.is_empty());
        assert!(!HISTORY_SYSTEM.is_empty());
        assert!(!HISTORY_USER.is_empty());
        assert!(!PR_SYSTEM.is_empty());
        assert!(!PR_USER.is_empty());
        assert!(!CHANGELOG_SYSTEM.is_empty());
        assert!(!CHANGELOG_USER.is_empty());
        assert!(!EXPLAIN_SYSTEM.is_empty());
        assert!(!EXPLAIN_USER.is_empty());
        assert!(!VERSION_SYSTEM.is_empty());
        assert!(!VERSION_USER.is_empty());
    }

    #[test]
    fn templates_mention_ascii() {
        let templates = [
            COMMIT_SYSTEM,
            HISTORY_SYSTEM,
            PR_SYSTEM,
            CHANGELOG_SYSTEM,
            EXPLAIN_SYSTEM,
            VERSION_SYSTEM,
        ];
        for t in templates {
            assert!(
                t.contains("ASCII") || t.contains("emoji"),
                "Template should mention ASCII restriction"
            );
        }
    }

    #[test]
    fn user_template_has_placeholders() {
        assert!(COMMIT_USER.contains("{diff}"));
        assert!(HISTORY_USER.contains("{diff}"));
        assert!(HISTORY_USER.contains("{original_message}"));
        assert!(PR_USER.contains("{branch}"));
        assert!(PR_USER.contains("{commits}"));
        assert!(PR_USER.contains("{stats}"));
        assert!(PR_USER.contains("{diff}"));
        assert!(CHANGELOG_USER.contains("{range}"));
        assert!(CHANGELOG_USER.contains("{commits}"));
        assert!(EXPLAIN_USER.contains("{stats}"));
        assert!(EXPLAIN_USER.contains("{diff}"));
        assert!(VERSION_USER.contains("{version}"));
        assert!(VERSION_USER.contains("{diff}"));
    }

    #[test]
    fn context_is_optional_and_delimited() {
        let p = commit_system_with_context(
            Preset::Rust,
            Some("Repo rules"),
            Some("User prefs"),
        );
        assert!(p.contains("Project Context"));
        assert!(p.contains("User Context"));

        let p2 = pr_system_with_context(Some("Repo rules"), Some("User prefs"));
        assert!(p2.contains("Project Context"));
        assert!(p2.contains("User Context"));

        let p3 = changelog_system_with_context(Some("Repo rules"), Some("User prefs"));
        assert!(p3.contains("Project Context"));
        assert!(p3.contains("User Context"));

        let p4 = explain_system_with_context(Some("Repo rules"), Some("User prefs"));
        assert!(p4.contains("Project Context"));
        assert!(p4.contains("User Context"));

        let p5 = version_system_with_context(Some("Repo rules"), Some("User prefs"));
        assert!(p5.contains("Project Context"));
        assert!(p5.contains("User Context"));
    }
}