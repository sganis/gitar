// src/command/plan/scoring.rs
use super::model::{
    FileKind, GroupTag, PlanCandidate, PlanGroup, PlanScore, PlanningContext, RiskLevel,
    ScoreReason,
};
use std::collections::HashSet;

// =============================================================================
// SCORE THRESHOLDS
// =============================================================================

#[allow(dead_code)]
const ACCEPTABLE_SCORE: i32 = 50;
const PENALTY_MIX_DOCS_SRC: i32 = 15;
const PENALTY_MIX_FORMAT_FUNCTIONAL: i32 = 20;
const PENALTY_MIX_RENAME_FEATURE: i32 = 10;
const PENALTY_MANY_ROOTS: i32 = 8;
const PENALTY_EXCEEDS_SIZE: i32 = 25;
const PENALTY_MISSING_FILES: i32 = 30;
const BONUS_SINGLE_MODULE: i32 = 10;
const BONUS_TESTS_WITH_SRC: i32 = 8;
const BONUS_CLEAR_RATIONALE: i32 = 5;

// =============================================================================
// PUBLIC API
// =============================================================================

/// Score a plan candidate against the planning context
pub fn score_plan(candidate: &PlanCandidate, context: &PlanningContext) -> PlanScore {
    let mut score = PlanScore::new();

    // Check file coverage
    check_file_coverage(candidate, context, &mut score);

    // Score each group
    for (idx, group) in candidate.groups.iter().enumerate() {
        score_group(group, idx, context, &mut score);
    }

    // Global bonuses
    apply_global_bonuses(candidate, context, &mut score);

    // Check constraint violations
    check_constraints(candidate, context, &mut score);

    score
}

/// Select the best candidate from a list
pub fn select_best_candidate(
    candidates: &[PlanCandidate],
    context: &PlanningContext,
) -> Option<(usize, PlanScore)> {
    if candidates.is_empty() {
        return None;
    }

    let scored: Vec<(usize, PlanScore)> = candidates
        .iter()
        .enumerate()
        .map(|(idx, c)| (idx, score_plan(c, context)))
        .collect();

    scored.into_iter().max_by_key(|(_, score)| score.total)
}

// =============================================================================
// FILE COVERAGE CHECK
// =============================================================================

fn check_file_coverage(candidate: &PlanCandidate, context: &PlanningContext, score: &mut PlanScore) {
    let context_files: HashSet<&str> = context.files.iter().map(|f| f.path.as_str()).collect();
    let grouped_files: HashSet<&str> = candidate
        .groups
        .iter()
        .flat_map(|g| g.files.iter().map(|f| f.as_str()))
        .collect();

    // Check for missing files
    let missing: Vec<&str> = context_files
        .difference(&grouped_files)
        .copied()
        .collect();

    if !missing.is_empty() {
        score.add_reason(ScoreReason::penalty(
            PENALTY_MISSING_FILES,
            format!("Missing {} file(s) from grouping", missing.len()),
        ));
        for file in missing.iter().take(3) {
            score.add_violation(format!("Missing file: {}", file));
        }
    }

    // Check for extra files (files in groups but not in context)
    let extra: Vec<&str> = grouped_files
        .difference(&context_files)
        .copied()
        .collect();

    if !extra.is_empty() {
        score.add_reason(ScoreReason::penalty(
            10,
            format!("Extra {} file(s) not in context", extra.len()),
        ));
    }
}

// =============================================================================
// GROUP SCORING
// =============================================================================

fn score_group(group: &PlanGroup, idx: usize, context: &PlanningContext, score: &mut PlanScore) {
    // Penalty: mixing docs and src
    if has_mixed_docs_and_src(group, context) {
        score.add_reason(ScoreReason::penalty(
            PENALTY_MIX_DOCS_SRC,
            format!("Group {} mixes docs and src files", idx + 1),
        ));
    }

    // Penalty: mixing formatting with functional changes
    if has_mixed_format_and_functional(group, context) {
        score.add_reason(ScoreReason::penalty(
            PENALTY_MIX_FORMAT_FUNCTIONAL,
            format!("Group {} mixes formatting with functional changes", idx + 1),
        ));
    }

    // Penalty: mixing renames with feature edits
    if has_mixed_rename_and_feature(group, context) {
        score.add_reason(ScoreReason::penalty(
            PENALTY_MIX_RENAME_FEATURE,
            format!("Group {} mixes renames with feature changes", idx + 1),
        ));
    }

    // Penalty: many unrelated path roots
    let root_count = count_path_roots(group);
    if root_count > 3 {
        score.add_reason(ScoreReason::penalty(
            PENALTY_MANY_ROOTS,
            format!(
                "Group {} touches {} unrelated directories",
                idx + 1,
                root_count
            ),
        ));
    }

    // Bonus: single module cohesion
    if root_count == 1 && group.files.len() > 1 {
        score.add_reason(ScoreReason::bonus(
            BONUS_SINGLE_MODULE,
            format!("Group {} has good single-module cohesion", idx + 1),
        ));
    }

    // Bonus: tests paired with src
    if has_tests_with_related_src(group, context) {
        score.add_reason(ScoreReason::bonus(
            BONUS_TESTS_WITH_SRC,
            format!("Group {} pairs tests with related source", idx + 1),
        ));
    }
}

fn has_mixed_docs_and_src(group: &PlanGroup, context: &PlanningContext) -> bool {
    let mut has_doc = false;
    let mut has_src = false;

    for file_path in &group.files {
        if let Some(info) = context.files.iter().find(|f| f.path == *file_path) {
            if info.doc {
                has_doc = true;
            } else if info.kind == FileKind::Code && !info.test && !info.config {
                has_src = true;
            }
        }
    }

    has_doc && has_src
}

fn has_mixed_format_and_functional(group: &PlanGroup, context: &PlanningContext) -> bool {
    let mut has_format = false;
    let mut has_functional = false;

    for file_path in &group.files {
        if let Some(info) = context.files.iter().find(|f| f.path == *file_path) {
            if info.kind == FileKind::Format {
                has_format = true;
            } else if info.churn > 0 && info.kind == FileKind::Code {
                has_functional = true;
            }
        }
    }

    // Also check tags
    if group.tags.contains(&GroupTag::Format) {
        has_format = true;
        // If has format tag but also has other non-format tags
        let has_other_tags = group
            .tags
            .iter()
            .any(|t| !matches!(t, GroupTag::Format | GroupTag::Doc));
        if has_other_tags {
            return true;
        }
    }

    has_format && has_functional
}

fn has_mixed_rename_and_feature(group: &PlanGroup, context: &PlanningContext) -> bool {
    let mut has_rename = false;
    let mut has_feature = false;

    for file_path in &group.files {
        if let Some(info) = context.files.iter().find(|f| f.path == *file_path) {
            if info.renamed {
                has_rename = true;
            } else if info.churn > 5 && info.kind == FileKind::Code {
                // Significant code change (not just minor edits)
                has_feature = true;
            }
        }
    }

    // Also check tags
    if group.tags.contains(&GroupTag::Rename) && group.tags.contains(&GroupTag::Feature) {
        return true;
    }

    has_rename && has_feature
}

fn count_path_roots(group: &PlanGroup) -> usize {
    let roots: HashSet<&str> = group
        .files
        .iter()
        .map(|f| {
            if let Some(idx) = f.find('/') {
                &f[..idx]
            } else {
                "root"
            }
        })
        .collect();
    roots.len()
}

fn has_tests_with_related_src(group: &PlanGroup, context: &PlanningContext) -> bool {
    let mut has_test = false;
    let mut has_src = false;

    for file_path in &group.files {
        if let Some(info) = context.files.iter().find(|f| f.path == *file_path) {
            if info.test {
                has_test = true;
            } else if info.kind == FileKind::Code && !info.config {
                has_src = true;
            }
        }
    }

    has_test && has_src
}

// =============================================================================
// GLOBAL BONUSES
// =============================================================================

fn apply_global_bonuses(candidate: &PlanCandidate, _context: &PlanningContext, score: &mut PlanScore) {
    // Bonus: clear rationale
    if !candidate.rationale.is_empty() && candidate.rationale.len() > 20 {
        score.add_reason(ScoreReason::bonus(
            BONUS_CLEAR_RATIONALE,
            "Clear rationale provided",
        ));
    }

    // Bonus: high confidence
    if candidate.confidence > 0.8 {
        score.add_reason(ScoreReason::bonus(5, "High confidence plan"));
    }

    // Penalty: too many groups
    if candidate.groups.len() > 6 {
        score.add_reason(ScoreReason::penalty(
            5,
            format!("Many groups ({})", candidate.groups.len()),
        ));
    }

    // Bonus: appropriate risk assessment
    let high_risk_groups = candidate
        .groups
        .iter()
        .filter(|g| g.risk == RiskLevel::High)
        .count();

    if high_risk_groups > 0 && candidate.risk == RiskLevel::High {
        score.add_reason(ScoreReason::bonus(3, "Risk properly escalated"));
    }
}

// =============================================================================
// CONSTRAINT CHECKING
// =============================================================================

fn check_constraints(
    candidate: &PlanCandidate,
    context: &PlanningContext,
    score: &mut PlanScore,
) {
    let constraints = &context.constraints;

    // Check max groups
    if candidate.groups.len() > constraints.max_groups {
        score.add_reason(ScoreReason::penalty(
            PENALTY_EXCEEDS_SIZE,
            format!(
                "Exceeds max groups ({} > {})",
                candidate.groups.len(),
                constraints.max_groups
            ),
        ));
        score.add_violation(format!(
            "Group count {} exceeds max {}",
            candidate.groups.len(),
            constraints.max_groups
        ));
    }

    // Check max files per group
    for (idx, group) in candidate.groups.iter().enumerate() {
        if group.files.len() > constraints.max_files_per_group {
            score.add_reason(ScoreReason::penalty(
                PENALTY_EXCEEDS_SIZE,
                format!(
                    "Group {} exceeds max files ({} > {})",
                    idx + 1,
                    group.files.len(),
                    constraints.max_files_per_group
                ),
            ));
            score.add_violation(format!(
                "Group {} has {} files (max {})",
                idx + 1,
                group.files.len(),
                constraints.max_files_per_group
            ));
        }
    }

    // Check separate formatting constraint
    if constraints.separate_formatting {
        check_formatting_separation(candidate, context, score);
    }

    // Check separate docs constraint
    if constraints.separate_docs {
        check_docs_separation(candidate, context, score);
    }
}

fn check_formatting_separation(
    candidate: &PlanCandidate,
    context: &PlanningContext,
    score: &mut PlanScore,
) {
    // If there are formatting files, they should be in their own group
    let format_files: Vec<&str> = context
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Format)
        .map(|f| f.path.as_str())
        .collect();

    if format_files.is_empty() {
        return;
    }

    // Check if any group mixes format files with other types
    for (idx, group) in candidate.groups.iter().enumerate() {
        let has_format = group.files.iter().any(|f| format_files.contains(&f.as_str()));
        let has_other = group.files.iter().any(|f| !format_files.contains(&f.as_str()));

        if has_format && has_other {
            score.add_violation(format!(
                "Group {} mixes formatting with other changes",
                idx + 1
            ));
        }
    }
}

fn check_docs_separation(
    candidate: &PlanCandidate,
    context: &PlanningContext,
    score: &mut PlanScore,
) {
    // If there are doc files, check they're separated from code
    let doc_files: Vec<&str> = context
        .files
        .iter()
        .filter(|f| f.doc)
        .map(|f| f.path.as_str())
        .collect();

    let code_files: Vec<&str> = context
        .files
        .iter()
        .filter(|f| f.kind == FileKind::Code && !f.test && !f.config && !f.doc)
        .map(|f| f.path.as_str())
        .collect();

    if doc_files.is_empty() || code_files.is_empty() {
        return;
    }

    // Check if any group mixes doc files with code (small violations allowed)
    for (idx, group) in candidate.groups.iter().enumerate() {
        let doc_count = group
            .files
            .iter()
            .filter(|f| doc_files.contains(&f.as_str()))
            .count();
        let code_count = group
            .files
            .iter()
            .filter(|f| code_files.contains(&f.as_str()))
            .count();

        // Only flag if significant mixing (both > 1)
        if doc_count > 1 && code_count > 1 {
            score.add_violation(format!("Group {} mixes docs with code", idx + 1));
        }
    }
}

// =============================================================================
// FORMATTING
// =============================================================================

/// Format score for display
pub fn format_score(score: &PlanScore) -> String {
    let mut out = String::new();
    out.push_str(&format!("Score: {} ", score.total));

    if score.is_acceptable() {
        out.push_str("(acceptable)");
    } else {
        out.push_str("(needs improvement)");
    }

    if !score.reasons.is_empty() {
        out.push_str("\n\nReasons:");
        for reason in &score.reasons {
            let sign = if reason.points >= 0 { "+" } else { "" };
            out.push_str(&format!("\n  {} {}: {}", sign, reason.points, reason.description));
        }
    }

    if !score.violations.is_empty() {
        out.push_str("\n\nViolations:");
        for v in &score.violations {
            out.push_str(&format!("\n  - {}", v));
        }
    }

    out
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::plan::model::{FileInfo, FileKind};

    fn make_context(files: Vec<(&str, FileKind, bool)>) -> PlanningContext {
        let file_infos: Vec<FileInfo> = files
            .into_iter()
            .map(|(path, kind, renamed)| {
                FileInfo::new(path.to_string())
                    .with_kind(kind)
                    .with_renamed(renamed)
                    .with_churn(10)
                    .with_doc(kind == FileKind::Doc)
                    .with_test(kind == FileKind::Test)
            })
            .collect();
        PlanningContext::new(file_infos)
    }

    fn make_group(files: Vec<&str>) -> PlanGroup {
        PlanGroup {
            title: "Test group".to_string(),
            summary: "Test summary".to_string(),
            files: files.into_iter().map(|s| s.to_string()).collect(),
            tags: vec![],
            risk: RiskLevel::Low,
            why: String::new(),
        }
    }

    fn make_candidate(groups: Vec<PlanGroup>) -> PlanCandidate {
        PlanCandidate {
            groups,
            rationale: "Test rationale with enough text".to_string(),
            risk: RiskLevel::Low,
            confidence: 0.8,
        }
    }

    #[test]
    fn score_perfect_plan() {
        let context = make_context(vec![
            ("src/main.rs", FileKind::Code, false),
            ("src/lib.rs", FileKind::Code, false),
        ]);
        let candidate = make_candidate(vec![make_group(vec!["src/main.rs", "src/lib.rs"])]);

        let score = score_plan(&candidate, &context);
        assert!(score.is_acceptable());
        assert!(score.violations.is_empty());
    }

    #[test]
    fn score_missing_files() {
        let context = make_context(vec![
            ("src/main.rs", FileKind::Code, false),
            ("src/lib.rs", FileKind::Code, false),
        ]);
        let candidate = make_candidate(vec![make_group(vec!["src/main.rs"])]);

        let score = score_plan(&candidate, &context);
        assert!(score.total < 100);
        assert!(!score.violations.is_empty());
    }

    #[test]
    fn score_mixed_docs_src() {
        let context = make_context(vec![
            ("src/main.rs", FileKind::Code, false),
            ("README.md", FileKind::Doc, false),
        ]);
        let candidate = make_candidate(vec![make_group(vec!["src/main.rs", "README.md"])]);

        let score = score_plan(&candidate, &context);
        assert!(score.total < 100);
        assert!(
            score
                .reasons
                .iter()
                .any(|r| r.description.contains("mixes docs"))
        );
    }

    #[test]
    fn score_single_module_bonus() {
        let context = make_context(vec![
            ("src/a.rs", FileKind::Code, false),
            ("src/b.rs", FileKind::Code, false),
            ("src/c.rs", FileKind::Code, false),
        ]);
        let candidate = make_candidate(vec![make_group(vec!["src/a.rs", "src/b.rs", "src/c.rs"])]);

        let score = score_plan(&candidate, &context);
        assert!(
            score
                .reasons
                .iter()
                .any(|r| r.description.contains("cohesion"))
        );
    }

    #[test]
    fn score_tests_with_src_bonus() {
        let context = make_context(vec![
            ("src/main.rs", FileKind::Code, false),
            ("tests/main_test.rs", FileKind::Test, false),
        ]);
        let candidate = make_candidate(vec![make_group(vec!["src/main.rs", "tests/main_test.rs"])]);

        let score = score_plan(&candidate, &context);
        assert!(
            score
                .reasons
                .iter()
                .any(|r| r.description.contains("tests with related"))
        );
    }

    #[test]
    fn score_exceeds_max_groups() {
        let context = make_context(vec![
            ("a.rs", FileKind::Code, false),
            ("b.rs", FileKind::Code, false),
            ("c.rs", FileKind::Code, false),
            ("d.rs", FileKind::Code, false),
            ("e.rs", FileKind::Code, false),
            ("f.rs", FileKind::Code, false),
            ("g.rs", FileKind::Code, false),
            ("h.rs", FileKind::Code, false),
            ("i.rs", FileKind::Code, false),
        ]);

        let groups: Vec<PlanGroup> = (0..9).map(|i| {
            let file = format!("{}.rs", (b'a' + i) as char);
            make_group(vec![&file])
        }).collect();

        let candidate = make_candidate(groups);
        let score = score_plan(&candidate, &context);

        assert!(!score.violations.is_empty());
    }

    #[test]
    fn select_best_candidate_picks_highest() {
        let context = make_context(vec![
            ("src/main.rs", FileKind::Code, false),
            ("README.md", FileKind::Doc, false),
        ]);

        // Bad candidate: mixes docs and src
        let bad = make_candidate(vec![make_group(vec!["src/main.rs", "README.md"])]);

        // Good candidate: separates them
        let good = make_candidate(vec![
            make_group(vec!["src/main.rs"]),
            make_group(vec!["README.md"]),
        ]);

        let result = select_best_candidate(&[bad, good], &context);
        assert!(result.is_some());
        let (idx, _) = result.unwrap();
        assert_eq!(idx, 1); // Second candidate is better
    }

    #[test]
    fn format_score_output() {
        let mut score = PlanScore::new();
        score.add_reason(ScoreReason::penalty(10, "Test penalty"));
        score.add_reason(ScoreReason::bonus(5, "Test bonus"));
        score.add_violation("Test violation");

        let output = format_score(&score);
        assert!(output.contains("Score:"));
        assert!(output.contains("Test penalty"));
        assert!(output.contains("Test bonus"));
        assert!(output.contains("Test violation"));
    }
}
