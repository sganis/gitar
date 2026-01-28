// src/context/template.rs - LLM prompt templates

use std::borrow::Cow;

use super::preset::Preset;
use super::prompt::{get_custom_system, get_custom_user};

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

User Context (~/.gitar/context.md; personal preferences; follow when relevant):
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

Project Context (.gitar/context.md; repo conventions; authoritative for this project):
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
    let base = get_custom_system("commit").unwrap_or(COMMIT_SYSTEM);
    let hints = preset.hints().format(preset.name());
    let ctx = format_context(project_context, user_context);
    format!("{}{}{}", base, hints, ctx)
}

/// Get the commit user prompt (custom or default)
pub fn get_commit_user() -> Cow<'static, str> {
    match get_custom_user("commit") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(COMMIT_USER),
    }
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
    let base = get_custom_system("history").unwrap_or(HISTORY_SYSTEM);
    let hints = preset.hints().format(preset.name());
    let ctx = format_context(project_context, user_context);
    format!("{}{}{}", base, hints, ctx)
}

/// Get the history user prompt (custom or default)
pub fn get_history_user() -> Cow<'static, str> {
    match get_custom_user("history") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(HISTORY_USER),
    }
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
    let base = get_custom_system("pr").unwrap_or(PR_SYSTEM);
    let ctx = format_context(project_context, user_context);
    format!("{}{}", base, ctx)
}

/// Get the PR user prompt (custom or default)
pub fn get_pr_user() -> Cow<'static, str> {
    match get_custom_user("pr") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(PR_USER),
    }
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
    let base = get_custom_system("changelog").unwrap_or(CHANGELOG_SYSTEM);
    let ctx = format_context(project_context, user_context);
    format!("{}{}", base, ctx)
}

/// Get the changelog user prompt (custom or default)
pub fn get_changelog_user() -> Cow<'static, str> {
    match get_custom_user("changelog") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(CHANGELOG_USER),
    }
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

pub fn explain_system_with_context(
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let base = get_custom_system("explain").unwrap_or(EXPLAIN_SYSTEM);
    let ctx = format_context(project_context, user_context);
    format!("{}{}", base, ctx)
}

/// Get the explain user prompt (custom or default)
pub fn get_explain_user() -> Cow<'static, str> {
    match get_custom_user("explain") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(EXPLAIN_USER),
    }
}

// =============================================================================
// VERSION BUMP
// =============================================================================

pub const VERSION_SYSTEM: &str = r#"Recommend semantic version bump based on these strict criteria:

MAJOR (x.0.0): Breaking changes only
- Removing or renaming public API
- Changing function signatures or behavior incompatibly
- Removing features users depend on

MINOR (0.x.0): New user-facing functionality only
- Adding new commands, flags, or features
- New capabilities that users can use
- NOT documentation, NOT build scripts, NOT CI changes

PATCH (0.0.x): Everything else
- Bug fixes
- Documentation changes (README, comments, help text)
- Build/CI/script improvements
- Refactoring without behavior change
- Dependency updates (non-breaking)
- Code style or formatting changes

Default to PATCH when uncertain. Documentation is NEVER a feature.

Use plain ASCII characters only. No emojis or Unicode.

Output format (exactly):
Recommendation: <major|minor|patch>
Reasoning: <brief explanation>
Breaking: <Yes|No>"#;

pub const VERSION_USER: &str = r#"Recommend version bump.

**Current:** {version}
**Diff:**
```
{diff}
```

"#;

pub fn version_system_with_context(
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let base = get_custom_system("version").unwrap_or(VERSION_SYSTEM);
    let ctx = format_context(project_context, user_context);
    format!("{}{}", base, ctx)
}

/// Get the version user prompt (custom or default)
pub fn get_version_user() -> Cow<'static, str> {
    match get_custom_user("version") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(VERSION_USER),
    }
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
        assert!(!WEEKLY_SYSTEM.is_empty());
        assert!(!WEEKLY_USER.is_empty());
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
            WEEKLY_SYSTEM,
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
        assert!(WEEKLY_USER.contains("{stats}"));
        assert!(WEEKLY_USER.contains("{diff}"));
    }

    #[test]
    fn context_is_optional_and_delimited() {
        let p = commit_system_with_context(Preset::Rust, Some("Repo rules"), Some("User prefs"));
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

        let p6 = weekly_system_with_context(Some("Repo rules"), Some("User prefs"));
        assert!(p6.contains("Project Context"));
        assert!(p6.contains("User Context"));
    }
}

pub const WEEKLY_SYSTEM: &str = r#"Generate a Weekly Highlights Report for senior leadership.

OUTPUT FORMAT (STRICT, NO EXCEPTIONS):
- The entire output MUST be one or more "highlight blocks".
- NOTHING is allowed outside highlight blocks (no intro, no outro, no extra paragraphs).

Each highlight block MUST be exactly:
1) Title line: "Highlight N (LEVEL):" where N starts at 1 and increments by 1
2) Subtitle line: ALL CAPS (ASCII only)
3) Body: 4-5 sentences TOTALING ~100 words (ASCII only). No headings, no bullets.
4) The last sentence must be a short description of the production rollout or next steps.

LEVEL RULES (STRICT):
- LEVEL must be exactly one of: CEO, VP, DIRECTOR, MANAGER
- Do NOT use any other levels.

ONE THEME PER HIGHLIGHT (STRICT):
- Each highlight MUST represent exactly ONE major initiative/theme.
- Within one highlight, ALL sentences must support the SAME theme.
- You may include multiple related deliverables inside the same theme (e.g., feature + docs + config + rollout), as long as they are part of the same initiative.

HIGHLIGHT COUNT RULES (VERY STRICT):
- Default to EXACTLY ONE highlight.
- You MUST produce EXACTLY ONE highlight unless ALL conditions below are true:
  A) The input describes at least TWO independent major initiatives that are NOT part of the same rollout, AND
  B) The initiatives impact different primary stakeholder groups OR different products/programs, AND
  C) They cannot be reasonably merged into one theme without losing clarity.
- If A/B/C are not all true, MERGE everything into ONE highlight by choosing a single umbrella theme.
- Never exceed 4 highlights.

MERGE GUIDANCE (USE THIS TO STAY AT ONE HIGHLIGHT):
- Treat docs, refactors, config changes, templating, scoring tweaks, and setup improvements as supporting work for the main initiative, not separate highlights.
- If the work is in the same repo/tool/product (e.g., Gitar planning + context + init + scoring), it is ONE highlight.
- If uncertain, choose ONE highlight.

PARAGRAPH RULES (APPLY TO EVERY PARAGRAPH):
Each paragraph must:
1) Start with delivery language: "The [Team] delivered..." or "The [Team] deployed..." or "The [Team] shipped..."
2) Name a relevant stakeholder (see User Context if provided)
3) State the outcome or improvement
4) End with a next step

STYLE:
Executive summary tone. Focus on deliverables and business value, not code details.
Plain ASCII only.

RANKING:
- If more than one highlight is allowed by the strict rules, rank by business impact.
- Choose LEVEL per highlight from CEO/VP/DIRECTOR/MANAGER based on impact.

PERSONALIZATION:
If User Context is provided below:
- Use the author's department/team name instead of generic "The Team"
- Target the report to the listed stakeholders (customers, leadership, etc.)
- Reference the author's projects when relevant
- If examples are provided, follow their format and style closely.

SELF-CHECK BEFORE FINAL ANSWER (SILENT):
- Output contains only highlight blocks.
- If output has more than one highlight, confirm A/B/C are all true.
- Every highlight has title + subtitle + 4-5 sentences (~100 words total).
- LEVEL is only CEO/VP/DIRECTOR/MANAGER.
"#;



pub const WEEKLY_USER: &str = r#"Generate a Weekly Highlight Report.

**Stats:**
{stats}

**Diff:**
```
{diff}
```

"#;

pub fn weekly_system_with_context(
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let base = get_custom_system("weekly").unwrap_or(WEEKLY_SYSTEM);
    let ctx = format_context(project_context, user_context);
    format!("{}{}", base, ctx)
}

/// Get the weekly user prompt (custom or default)
pub fn get_weekly_user() -> Cow<'static, str> {
    match get_custom_user("weekly") {
        Some(s) => Cow::Borrowed(s),
        None => Cow::Borrowed(WEEKLY_USER),
    }
}
