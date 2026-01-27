// src/command/plan/model.rs
use serde::{Deserialize, Serialize};

// =============================================================================
// FILE METADATA
// =============================================================================

/// File-level metadata for planning context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub kind: FileKind,
    pub churn: usize, // additions + deletions
    pub renamed: bool,
    pub test: bool,
    pub doc: bool,
    pub config: bool,
    #[serde(default)]
    pub large_file: bool, // >= 50 MB
}

impl FileInfo {
    pub fn new(path: String) -> Self {
        Self {
            path,
            kind: FileKind::Code,
            churn: 0,
            renamed: false,
            test: false,
            doc: false,
            config: false,
            large_file: false,
        }
    }

    pub fn with_kind(mut self, kind: FileKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_churn(mut self, churn: usize) -> Self {
        self.churn = churn;
        self
    }

    pub fn with_renamed(mut self, renamed: bool) -> Self {
        self.renamed = renamed;
        self
    }

    pub fn with_test(mut self, test: bool) -> Self {
        self.test = test;
        self
    }

    pub fn with_doc(mut self, doc: bool) -> Self {
        self.doc = doc;
        self
    }

    pub fn with_config(mut self, config: bool) -> Self {
        self.config = config;
        self
    }

    pub fn with_large_file(mut self, large_file: bool) -> Self {
        self.large_file = large_file;
        self
    }
}

/// File type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Code,
    Test,
    Doc,
    Config,
    Format,
}

// =============================================================================
// PLANNING CONTEXT
// =============================================================================

/// Signals detected from the change set
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangeSignals {
    pub large_rename_set: bool,      // Many files renamed
    pub mostly_whitespace: bool,     // Formatting-only changes
    pub touches_tests_and_src: bool, // Mixed test + source changes
    pub single_module: bool,         // All files in same directory
    pub dependency_update: bool,     // Cargo.toml/package.json changes
    pub docs_only: bool,             // Only documentation files
    pub has_large_files: bool,       // Files >= 50 MB present
}

/// Default constraints for planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanConstraints {
    pub max_files_per_group: usize,
    pub max_groups: usize,
    pub separate_formatting: bool,
    pub separate_docs: bool,
    pub separate_renames: bool,
    pub keep_tests_with_src: bool,
}

impl Default for PlanConstraints {
    fn default() -> Self {
        Self {
            max_files_per_group: 15,
            max_groups: 8,
            separate_formatting: true,
            separate_docs: true,
            separate_renames: true,
            keep_tests_with_src: true,
        }
    }
}

/// Complete planning context sent to LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningContext {
    pub files: Vec<FileInfo>,
    pub signals: ChangeSignals,
    pub constraints: PlanConstraints,
}

impl PlanningContext {
    pub fn new(files: Vec<FileInfo>) -> Self {
        Self {
            files,
            signals: ChangeSignals::default(),
            constraints: PlanConstraints::default(),
        }
    }

    pub fn with_signals(mut self, signals: ChangeSignals) -> Self {
        self.signals = signals;
        self
    }

    pub fn with_constraints(mut self, constraints: PlanConstraints) -> Self {
        self.constraints = constraints;
        self
    }
}

// =============================================================================
// PLAN GROUP (LLM OUTPUT)
// =============================================================================

/// A group of files that belong in one commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanGroup {
    pub title: String,
    pub summary: String,
    pub files: Vec<String>,
    #[serde(default)]
    pub tags: Vec<GroupTag>,
    #[serde(default)]
    pub risk: RiskLevel,
    #[serde(default)]
    pub why: String,
}

/// Tags describing the nature of changes in a group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupTag {
    Feature,
    Fix,
    Refactor,
    Format,
    Doc,
    Test,
    Config,
    Rename,
    LargeFile,
}

/// Risk level for a group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
}

// =============================================================================
// PLAN CANDIDATE (LLM OUTPUT)
// =============================================================================

/// A complete grouping proposal from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCandidate {
    pub groups: Vec<PlanGroup>,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub risk: RiskLevel,
    #[serde(default)]
    pub confidence: f32,
}

/// Response from LLM with multiple candidates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResponse {
    pub candidates: Vec<PlanCandidate>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

// =============================================================================
// PLAN SCORE (DETERMINISTIC SCORING)
// =============================================================================

/// Reason for a score penalty or bonus
#[derive(Debug, Clone)]
pub struct ScoreReason {
    pub points: i32,
    pub description: String,
}

impl ScoreReason {
    pub fn penalty(points: i32, description: impl Into<String>) -> Self {
        Self {
            points: -points.abs(),
            description: description.into(),
        }
    }

    pub fn bonus(points: i32, description: impl Into<String>) -> Self {
        Self {
            points: points.abs(),
            description: description.into(),
        }
    }
}

/// Complete score for a plan candidate
#[derive(Debug, Clone)]
pub struct PlanScore {
    pub total: i32,
    pub reasons: Vec<ScoreReason>,
    pub violations: Vec<String>,
}

impl PlanScore {
    pub fn new() -> Self {
        Self {
            total: 100, // Start with base score
            reasons: Vec::new(),
            violations: Vec::new(),
        }
    }

    pub fn add_reason(&mut self, reason: ScoreReason) {
        self.total += reason.points;
        self.reasons.push(reason);
    }

    pub fn add_violation(&mut self, violation: impl Into<String>) {
        self.violations.push(violation.into());
    }

    pub fn is_acceptable(&self) -> bool {
        self.total >= 50 && self.violations.is_empty()
    }
}

impl Default for PlanScore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// UNIT TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_info_builder() {
        let info = FileInfo::new("src/main.rs".to_string())
            .with_kind(FileKind::Code)
            .with_churn(50)
            .with_test(false);

        assert_eq!(info.path, "src/main.rs");
        assert_eq!(info.kind, FileKind::Code);
        assert_eq!(info.churn, 50);
        assert!(!info.test);
    }

    #[test]
    fn plan_constraints_default() {
        let constraints = PlanConstraints::default();
        assert_eq!(constraints.max_files_per_group, 15);
        assert_eq!(constraints.max_groups, 8);
        assert!(constraints.separate_formatting);
        assert!(constraints.separate_docs);
    }

    #[test]
    fn plan_score_calculation() {
        let mut score = PlanScore::new();
        assert_eq!(score.total, 100);

        score.add_reason(ScoreReason::penalty(20, "Mixed docs and src"));
        assert_eq!(score.total, 80);

        score.add_reason(ScoreReason::bonus(10, "Good cohesion"));
        assert_eq!(score.total, 90);

        assert!(score.is_acceptable());
    }

    #[test]
    fn plan_score_with_violations() {
        let mut score = PlanScore::new();
        score.add_violation("Group exceeds max files");
        assert!(!score.is_acceptable());
    }

    #[test]
    fn plan_group_deserialize() {
        let json = r#"{
            "title": "Update auth",
            "summary": "Add OAuth support",
            "files": ["src/auth.rs"],
            "tags": ["feature"],
            "risk": "medium",
            "why": "New feature"
        }"#;
        let group: PlanGroup = serde_json::from_str(json).unwrap();
        assert_eq!(group.title, "Update auth");
        assert_eq!(group.files.len(), 1);
        assert_eq!(group.tags, vec![GroupTag::Feature]);
        assert_eq!(group.risk, RiskLevel::Medium);
    }

    #[test]
    fn plan_candidate_deserialize() {
        let json = r#"{
            "groups": [{
                "title": "Add feature",
                "summary": "New feature",
                "files": ["src/main.rs"]
            }],
            "rationale": "Single cohesive change",
            "risk": "low",
            "confidence": 0.9
        }"#;
        let candidate: PlanCandidate = serde_json::from_str(json).unwrap();
        assert_eq!(candidate.groups.len(), 1);
        assert_eq!(candidate.confidence, 0.9);
    }

    #[test]
    fn plan_response_deserialize() {
        let json = r#"{
            "candidates": [{
                "groups": [{
                    "title": "Update",
                    "summary": "Changes",
                    "files": ["a.rs"]
                }],
                "rationale": "Simple",
                "confidence": 0.8
            }],
            "assumptions": [],
            "open_questions": []
        }"#;
        let response: PlanResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.candidates.len(), 1);
    }

    #[test]
    fn change_signals_default() {
        let signals = ChangeSignals::default();
        assert!(!signals.large_rename_set);
        assert!(!signals.mostly_whitespace);
        assert!(!signals.docs_only);
        assert!(!signals.has_large_files);
    }

    #[test]
    fn risk_level_default() {
        let risk = RiskLevel::default();
        assert_eq!(risk, RiskLevel::Low);
    }
}
