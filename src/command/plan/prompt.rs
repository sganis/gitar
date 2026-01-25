// src/command/plan/prompt.rs
use super::context::context_to_json;
use super::model::{PlanCandidate, PlanGroup, PlanResponse, PlanningContext, RiskLevel};
use crate::context::load_all_context;
use crate::context::Preset;

// =============================================================================
// PLANNING SYSTEM PROMPT
// =============================================================================

const PLAN_SYSTEM: &str = r#"You are an expert at organizing Git commits for clean, reviewable history.

Your task: Given a list of changed files with metadata, produce 2-3 alternative commit grouping strategies.

## Hard Constraints (MUST follow)

1. **Separate formatting-only changes** into their own group
2. **Separate documentation-only changes** into their own group (unless tiny + related)
3. **Keep renames/moves together** - don't mix pure renames with feature changes
4. **Keep tests with the feature they validate** (same group)
5. **Respect size limits**: max_files_per_group and max_groups from constraints
6. **No mixing unrelated top-level directories** unless clearly justified

## Objectives (soft goals, maximize these)

1. **Cohesion**: Group files that change together for the same reason
2. **Minimal cross-topic mixing**: Each group = one logical change
3. **Risk isolation**: Put risky changes in their own group
4. **Good ordering**: Low-risk first, high-risk last

## Output Format

Return ONLY valid JSON (no markdown, no explanation outside JSON):

```json
{
  "candidates": [
    {
      "groups": [
        {
          "title": "Short commit title",
          "summary": "What this group does and why",
          "files": ["path/to/file.rs", "path/to/other.rs"],
          "tags": ["feature", "test"],
          "risk": "low",
          "why": "Brief justification"
        }
      ],
      "rationale": "Why this grouping strategy",
      "risk": "low",
      "confidence": 0.85
    }
  ],
  "assumptions": [],
  "open_questions": []
}
```

## Tags (use 1-2 per group)
- feature: New functionality
- fix: Bug fix
- refactor: Code restructuring
- format: Formatting/whitespace only
- doc: Documentation changes
- test: Test changes
- config: Configuration changes
- rename: File/symbol renames

## Risk Levels
- low: Safe, reversible, well-tested area
- medium: Some complexity, moderate impact
- high: Critical path, breaking change potential

## Rules
- Use plain ASCII only. No emojis or special characters.
- Every file from context MUST appear in exactly one group
- Produce 2-3 candidates with different strategies
- Confidence: 0.0 to 1.0 (how good is this grouping?)
"#;

// =============================================================================
// USER PROMPT BUILDER
// =============================================================================

/// Build the user prompt with planning context
pub fn build_plan_user_prompt(context: &PlanningContext, diff_summary: Option<&str>) -> String {
    let context_json = context_to_json(context);

    let mut prompt = String::new();
    prompt.push_str("Group these file changes into logical commits.\n\n");
    prompt.push_str("## Planning Context\n\n");
    prompt.push_str("```json\n");
    prompt.push_str(&context_json);
    prompt.push_str("\n```\n\n");

    if let Some(summary) = diff_summary {
        prompt.push_str("## Diff Summary\n\n");
        prompt.push_str(summary);
        prompt.push_str("\n\n");
    }

    prompt.push_str("## Instructions\n\n");
    prompt.push_str("1. Analyze the file list and signals\n");
    prompt.push_str("2. Produce 2-3 alternative grouping strategies\n");
    prompt.push_str("3. Follow the constraints strictly\n");
    prompt.push_str("4. Return ONLY valid JSON\n");

    prompt
}

/// Build the system prompt with optional context
pub fn build_plan_system_prompt(
    preset: Preset,
    project_context: Option<&str>,
    user_context: Option<&str>,
) -> String {
    let mut prompt = PLAN_SYSTEM.to_string();

    // Add preset hints
    let hints = preset.hints().format(preset.name());
    prompt.push_str(&hints);

    // Add user context
    if let Some(ctx) = user_context {
        let ctx = ctx.trim();
        if !ctx.is_empty() {
            prompt.push_str("\n\n## User Preferences\n");
            prompt.push_str(ctx);
        }
    }

    // Add project context
    if let Some(ctx) = project_context {
        let ctx = ctx.trim();
        if !ctx.is_empty() {
            prompt.push_str("\n\n## Project Conventions\n");
            prompt.push_str(ctx);
        }
    }

    prompt
}

/// Get the system and user prompts for planning
pub fn get_plan_prompts(
    context: &PlanningContext,
    preset: Preset,
    diff_summary: Option<&str>,
) -> (String, String) {
    let (project_ctx, user_ctx) = load_all_context();
    let system = build_plan_system_prompt(preset, project_ctx.as_deref(), user_ctx.as_deref());
    let user = build_plan_user_prompt(context, diff_summary);
    (system, user)
}

// =============================================================================
// RESPONSE PARSING
// =============================================================================

/// Parse LLM response into PlanResponse
pub fn parse_plan_response(response: &str) -> Result<PlanResponse, ParseError> {
    let response = response.trim();

    // Try direct JSON parse
    if let Ok(plan) = serde_json::from_str::<PlanResponse>(response) {
        return Ok(plan);
    }

    // Try extracting JSON from code block
    if let Some(json) = extract_json_block(response) {
        if let Ok(plan) = serde_json::from_str::<PlanResponse>(&json) {
            return Ok(plan);
        }
    }

    // Try finding JSON object in response
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            let json = &response[start..=end];
            if let Ok(plan) = serde_json::from_str::<PlanResponse>(json) {
                return Ok(plan);
            }
        }
    }

    Err(ParseError::InvalidJson(response.to_string()))
}

/// Extract JSON from markdown code block
fn extract_json_block(text: &str) -> Option<String> {
    // Look for ```json ... ``` block
    let json_start = text.find("```json").or_else(|| text.find("```"))?;
    let content_start = text[json_start..].find('\n')? + json_start + 1;
    let content_end = text[content_start..].find("```")? + content_start;

    Some(text[content_start..content_end].trim().to_string())
}

/// Parse error types
#[derive(Debug)]
pub enum ParseError {
    InvalidJson(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidJson(s) => write!(f, "Invalid JSON response: {}", s),
        }
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// FALLBACK: HEURISTIC GROUPING
// =============================================================================

/// Create a fallback plan from heuristic grouping (no LLM)
pub fn create_fallback_plan(context: &PlanningContext) -> PlanResponse {
    let mut groups: Vec<PlanGroup> = Vec::new();

    // Group 1: Documentation
    let doc_files: Vec<String> = context
        .files
        .iter()
        .filter(|f| f.doc)
        .map(|f| f.path.clone())
        .collect();

    if !doc_files.is_empty() {
        groups.push(PlanGroup {
            title: "Update documentation".to_string(),
            summary: "Documentation changes".to_string(),
            files: doc_files,
            tags: vec![super::model::GroupTag::Doc],
            risk: RiskLevel::Low,
            why: "Separated docs for clean history".to_string(),
        });
    }

    // Group 2: Tests
    let test_files: Vec<String> = context
        .files
        .iter()
        .filter(|f| f.test)
        .map(|f| f.path.clone())
        .collect();

    if !test_files.is_empty() {
        groups.push(PlanGroup {
            title: "Update tests".to_string(),
            summary: "Test changes".to_string(),
            files: test_files,
            tags: vec![super::model::GroupTag::Test],
            risk: RiskLevel::Low,
            why: "Separated tests".to_string(),
        });
    }

    // Group 3: Config
    let config_files: Vec<String> = context
        .files
        .iter()
        .filter(|f| f.config)
        .map(|f| f.path.clone())
        .collect();

    if !config_files.is_empty() {
        groups.push(PlanGroup {
            title: "Update configuration".to_string(),
            summary: "Configuration changes".to_string(),
            files: config_files,
            tags: vec![super::model::GroupTag::Config],
            risk: RiskLevel::Low,
            why: "Separated config".to_string(),
        });
    }

    // Group 4: Renames
    let rename_files: Vec<String> = context
        .files
        .iter()
        .filter(|f| f.renamed)
        .map(|f| f.path.clone())
        .collect();

    if !rename_files.is_empty() {
        groups.push(PlanGroup {
            title: "Rename files".to_string(),
            summary: "File renames".to_string(),
            files: rename_files,
            tags: vec![super::model::GroupTag::Rename],
            risk: RiskLevel::Low,
            why: "Separated renames".to_string(),
        });
    }

    // Group 5: Code (everything else)
    let code_files: Vec<String> = context
        .files
        .iter()
        .filter(|f| !f.doc && !f.test && !f.config && !f.renamed)
        .map(|f| f.path.clone())
        .collect();

    if !code_files.is_empty() {
        groups.push(PlanGroup {
            title: "Update code".to_string(),
            summary: "Code changes".to_string(),
            files: code_files,
            tags: vec![super::model::GroupTag::Feature],
            risk: RiskLevel::Medium,
            why: "Main code changes".to_string(),
        });
    }

    let candidate = PlanCandidate {
        groups,
        rationale: "Heuristic grouping by file category".to_string(),
        risk: RiskLevel::Low,
        confidence: 0.6,
    };

    PlanResponse {
        candidates: vec![candidate],
        assumptions: vec![],
        open_questions: vec![],
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::model::FileInfo;

    fn make_context() -> PlanningContext {
        PlanningContext::new(vec![
            FileInfo::new("src/main.rs".to_string()),
            FileInfo::new("README.md".to_string()).with_doc(true),
        ])
    }

    #[test]
    fn build_user_prompt_includes_context() {
        let context = make_context();
        let prompt = build_plan_user_prompt(&context, None);

        assert!(prompt.contains("src/main.rs"));
        assert!(prompt.contains("README.md"));
        assert!(prompt.contains("Planning Context"));
    }

    #[test]
    fn build_user_prompt_includes_diff_summary() {
        let context = make_context();
        let prompt = build_plan_user_prompt(&context, Some("10 files changed"));

        assert!(prompt.contains("Diff Summary"));
        assert!(prompt.contains("10 files changed"));
    }

    #[test]
    fn build_system_prompt_basic() {
        let prompt = build_plan_system_prompt(Preset::Rust, None, None);

        assert!(prompt.contains("Hard Constraints"));
        assert!(prompt.contains("Objectives"));
        assert!(prompt.contains("Output Format"));
    }

    #[test]
    fn parse_plan_response_direct_json() {
        let json = r#"{
            "candidates": [{
                "groups": [{
                    "title": "Test",
                    "summary": "Test",
                    "files": ["a.rs"]
                }],
                "rationale": "Test",
                "confidence": 0.9
            }],
            "assumptions": [],
            "open_questions": []
        }"#;

        let result = parse_plan_response(json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().candidates.len(), 1);
    }

    #[test]
    fn parse_plan_response_code_block() {
        let response = r#"Here is the plan:

```json
{
    "candidates": [{
        "groups": [{
            "title": "Test",
            "summary": "Test",
            "files": ["a.rs"]
        }],
        "rationale": "Test",
        "confidence": 0.9
    }],
    "assumptions": [],
    "open_questions": []
}
```

Let me know if you need changes."#;

        let result = parse_plan_response(response);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_plan_response_embedded_json() {
        let response = r#"Based on the files, here is my recommendation: {
            "candidates": [{
                "groups": [{
                    "title": "Test",
                    "summary": "Test",
                    "files": ["a.rs"]
                }],
                "rationale": "Test",
                "confidence": 0.9
            }],
            "assumptions": [],
            "open_questions": []
        } - this should work well."#;

        let result = parse_plan_response(response);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_plan_response_invalid() {
        let result = parse_plan_response("This is not JSON at all");
        assert!(result.is_err());
    }

    #[test]
    fn create_fallback_plan_groups_correctly() {
        let context = PlanningContext::new(vec![
            FileInfo::new("src/main.rs".to_string()),
            FileInfo::new("README.md".to_string()).with_doc(true),
            FileInfo::new("tests/test.rs".to_string()).with_test(true),
        ]);

        let plan = create_fallback_plan(&context);
        assert_eq!(plan.candidates.len(), 1);

        let candidate = &plan.candidates[0];
        assert_eq!(candidate.groups.len(), 3); // docs, tests, code
    }

    #[test]
    fn extract_json_block_works() {
        let text = "Some text\n```json\n{\"key\": \"value\"}\n```\nMore text";
        let result = extract_json_block(text);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "{\"key\": \"value\"}");
    }

    #[test]
    fn extract_json_block_no_block() {
        let text = "No code block here";
        let result = extract_json_block(text);
        assert!(result.is_none());
    }
}
