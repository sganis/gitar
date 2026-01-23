# Gitar Plan Layer Implementation Spec

## Executive Summary

Transform `gitar plan` from a 49-line state inspector into "THE PRODUCT" - an AI-native commit planning and history shaping engine that embodies the philosophy: **"AI proposes. Human approves. Git executes."**

Current state: `split` command exists as a working prototype (474 lines) but only handles unstaged changes with limited interactivity. The `plan` command is a stub. Critical infrastructure (Plan/Executor, diff algorithms, secret detection) exists but is unused.

Target state: Unified `gitar plan` command that analyzes any Git state (working tree, staged, history ranges), proposes optimal commit structure, enables interactive refinement, and safely executes with full validation.

---

## Vision Alignment (from README.md)

The README "Future" section (lines 415-749) defines Gitar's four conceptual layers:

1. 📝 **Narrate** - Read-only explanation (DONE: commit, pr, changelog, explain)
2. 🚀 **Plan** - Commit planning and history shaping (THIS SPEC)
3. 🧰 **Release** - Release automation (Future)
4. 🩹 **Resolve** - Conflict resolution (DONE: excellent reference architecture)

**Plan Layer Requirements** (README lines 506-528):
```
gitar plan will:
* Analyze: working tree, staged, untracked, or history ranges
* Propose: number of commits, grouping, ordering, messages
* Let the human: accept/reject, move files between groups, exclude files, regenerate plan
* Then: execute Git plumbing safely
```

---

## Current State Analysis

### What Exists Today

**1. `command/plan.rs` (49 lines)** - Stub implementation
- Detects conflicts → suggests `gitar resolve`
- Checks staged/unstaged/untracked → suggests `gitar split` or `gitar commit`
- No LLM usage, no Plan infrastructure, just state inspection
- `--apply` flag exists but is no-op

**2. `command/split.rs` (474 lines)** - Working prototype
- ✅ Scans unstaged changes (git diff + git status)
- ✅ Groups files by category (docs, tests, config, code)
- ✅ Generates commit messages via LLM
- ✅ Interactive execution loop (accept/edit/skip/quit per commit)
- ❌ Ignores `--algo` parameter (uses manual truncation)
- ❌ No secret detection/redaction
- ❌ Doesn't use Plan/Executor infrastructure
- ❌ Can't analyze staged changes or history
- ❌ Can't move files between groups
- ❌ Can't exclude files
- ❌ Can't regenerate plan

**3. Core Infrastructure** (exists but unused by plan/split)
- `plan.rs` (117 lines) - Plan struct, Action enum, Display impl
- `executor.rs` (41 lines) - Dry-run capable executor
- `prompt/algo.rs` - 4 diff shaping algorithms (Full, Files, Hunks, Semantic)
- `prompt/secret.rs` - Secret detection with redact/warn/block actions
- `command/mod.rs` - `apply_smart_diff()` helper integrating algorithms + secrets

**4. Reference Architecture** (command/resolve/)
- Excellent "AI proposes, human approves, Git executes" pattern
- Multi-tier decision making (heuristic → LLM region → LLM full-file)
- Interactive confirmation with diff previews
- 6-layer safety validation
- Clear fallback chains

---

## Architecture Gap Analysis

### Missing Pieces

| Feature | README Requirement | Current State | Gap |
|---------|-------------------|---------------|-----|
| Analyze working tree | ✅ Yes | ✅ Yes (split only) | Integrate into plan |
| Analyze staged | ✅ Yes | ❌ No | Need staged mode |
| Analyze history | ✅ Yes | ❌ No | Need history range mode |
| Propose grouping | ✅ Yes | ✅ Yes (split) | Refine with LLM + diff context |
| Move files between groups | ✅ Yes | ❌ No | Need interactive editor |
| Exclude files | ✅ Yes | ❌ No | Need exclusion UI |
| Regenerate plan | ✅ Yes | ❌ No | Need LLM re-call |
| Use Plan infrastructure | ✅ Yes | ❌ No | Refactor to declarative |
| Apply diff algorithms | ✅ Yes | ❌ No (ignored) | Use apply_smart_diff |
| Secret detection | ✅ Yes | ❌ No | Use apply_smart_diff |

### Design Violations

**split.rs violates established patterns:**
1. Manual diff truncation instead of using `prompt/algo.rs` algorithms
2. No `apply_smart_diff()` usage (skips secret detection)
3. Imperative execution (direct git calls) instead of declarative Plan/Executor
4. Local `CommitPlan` struct instead of shared `Plan` from `plan.rs`
5. 474 lines exceeds Rust Vibe Coding Standards (300-500 optimal, 600 hard limit)

---

## Proposed Architecture

### Phase 1: Unified Analysis Engine

**File: `src/command/plan/analyze.rs` (NEW)**

Responsibilities:
- Detect repo state (conflicts, staged, unstaged, untracked)
- Route to appropriate analyzer based on mode
- Return structured `AnalysisResult`

```rust
pub enum AnalysisMode {
    Auto,           // Detect best mode from repo state
    WorkingTree,    // Unstaged + untracked
    Staged,         // Only staged changes
    History {       // Commit range
        from: String,
        to: Option<String>,
    },
}

pub struct AnalysisResult {
    pub mode: AnalysisMode,
    pub diff: String,           // Raw diff
    pub files: Vec<FileChange>, // Parsed changes
    pub stats: DiffStats,       // From diff algorithm
}
```

### Phase 2: Smart Grouping with LLM

**File: `src/command/plan/group.rs` (NEW)**

Responsibilities:
- Apply diff shaping algorithm (use `apply_smart_diff()`)
- Scan for secrets (integrated in `apply_smart_diff()`)
- Group files using heuristics + LLM
- Generate commit messages per group

Key improvement over split.rs:
- Uses proper diff algorithms (respect `--algo` flag)
- Includes secret detection before LLM call
- Leverages existing infrastructure

```rust
pub struct CommitGroup {
    pub id: usize,
    pub title: String,        // Short label
    pub message: String,      // Full commit message
    pub files: Vec<FileChange>,
    pub estimated_tokens: usize,
}

pub async fn create_groups(
    analysis: &AnalysisResult,
    client: &LlmClient,
    algo: u8,
    max_chars: usize,
) -> Result<Vec<CommitGroup>> {
    // 1. Apply diff algorithm + secret detection
    let shaped = apply_smart_diff(&analysis.diff, max_chars, false, algo, secret_action)?;

    // 2. Heuristic grouping (docs, tests, config, etc.)
    let initial = group_by_heuristics(&analysis.files);

    // 3. LLM refinement with context
    let refined = refine_with_llm(client, initial, &shaped).await?;

    Ok(refined)
}
```

### Phase 3: Interactive Plan Editor

**File: `src/command/plan/editor.rs` (NEW)**

Responsibilities:
- Display proposed plan to user
- Handle interactive commands:
  - `[Enter]` Accept plan
  - `[r]` Regenerate plan (re-call LLM)
  - `[m]` Move file between groups
  - `[e]` Exclude file from all groups
  - `[t]` Edit group title/message
  - `[q]` Quit without executing

Pattern: Similar to resolve's per-region interactive loop but for groups.

```rust
pub struct PlanEditor {
    groups: Vec<CommitGroup>,
    excluded: Vec<String>,  // Excluded file paths
}

impl PlanEditor {
    pub fn display(&self) {
        // Show numbered groups with files
        // Show excluded files section
        // Show command options
    }

    pub async fn run_interactive(
        &mut self,
        client: &LlmClient,
        analysis: &AnalysisResult,
    ) -> Result<EditResult> {
        loop {
            self.display();

            let action = prompt_action()?;
            match action {
                Action::Accept => return Ok(EditResult::Approved(self.groups.clone())),
                Action::Regenerate => {
                    self.groups = regenerate_plan(client, analysis).await?;
                }
                Action::MoveFile { file, from, to } => {
                    self.move_file(&file, from, to)?;
                }
                Action::ExcludeFile(file) => {
                    self.excluded.push(file);
                    self.remove_from_groups(&file);
                }
                Action::EditMessage { group_id } => {
                    self.edit_message(group_id)?;
                }
                Action::Quit => return Ok(EditResult::Cancelled),
            }
        }
    }
}
```

### Phase 4: Safe Execution with Plan/Executor

**File: `src/command/plan/execute.rs` (NEW)**

Responsibilities:
- Convert CommitGroups to Plan struct
- Use Executor for dry-run and execution
- Implement safety checks (like resolve command)
- Show diff preview before each commit
- Handle per-commit confirmation

```rust
pub async fn execute_plan(
    groups: &[CommitGroup],
    mode: &AnalysisMode,
    dry_run: bool,
) -> Result<()> {
    // Convert to Plan infrastructure
    let mut plan = Plan::new("Commit plan execution");

    for (idx, group) in groups.iter().enumerate() {
        // Stage files
        for file in &group.files {
            plan.push(Action::git(&["add", file.path.as_str()]));
        }

        // Show diff preview (per resolve pattern)
        plan.push(Action::suggest_with_detail(
            format!("Preview commit {}/{}", idx + 1, groups.len()),
            "git diff --cached --stat".into(),
        ));

        // Commit
        plan.push(Action::git(&["commit", "-m", &group.message]));
    }

    // Execute with safety checks
    Executor::execute(&plan, dry_run)?;

    // Validation (like resolve's 6-layer checks)
    validate_execution()?;

    Ok(())
}
```

### Phase 5: Unified Command Entry Point

**File: `src/command/plan/mod.rs` (REFACTOR)**

Current 49-line stub becomes the orchestrator:

```rust
pub async fn cmd_plan(
    client: &LlmClient,
    config: &ResolvedConfig,
    mode: Option<AnalysisMode>,
    apply: bool,
    algo: u8,
) -> Result<()> {
    // 1. Analyze
    let analysis = analyze::detect_and_analyze(mode).await?;

    // 2. Group + Generate Messages
    let groups = group::create_groups(&analysis, client, algo, config.max_diff_chars).await?;

    // 3. Interactive Editing
    let mut editor = editor::PlanEditor::new(groups);
    let result = editor.run_interactive(client, &analysis).await?;

    let approved_groups = match result {
        EditResult::Approved(g) => g,
        EditResult::Cancelled => {
            println!("Plan cancelled.");
            return Ok(());
        }
    };

    // 4. Execute
    if apply {
        execute::execute_plan(&approved_groups, &analysis.mode, false).await?;
        println!("✓ Plan executed successfully");
    } else {
        // Dry run / preview only
        execute::execute_plan(&approved_groups, &analysis.mode, true).await?;
        println!("(Dry run - use --apply to execute)");
    }

    Ok(())
}
```

---

## Migration Strategy

### Step 1: Extract and Modularize split.rs

Current split.rs is 474 lines (violates 300-500 Goldilocks Zone). Refactor into:

```
src/command/plan/
├── mod.rs           # Orchestrator (from current plan.rs stub)
├── analyze.rs       # State detection + parsing (from split scan_diff)
├── group.rs         # Grouping logic (from split group_changes + generate_plan)
├── editor.rs        # Interactive UI (NEW - enhanced from split execute_plan)
└── execute.rs       # Safe execution (NEW - uses Plan/Executor)
```

Each file stays under 500 lines (Rust Vibe Coding Standard).

### Step 2: Deprecate split Command

After `plan` reaches feature parity:
1. Mark `split` as deprecated in CLI help
2. Make `split` an alias to `plan --mode working-tree`
3. Eventually remove in v2.0

### Step 3: Integrate Existing Infrastructure

**Use apply_smart_diff() helper** (command/mod.rs:96-137)
- Already integrates diff algorithms + secret detection
- Used by all other commands (commit, pr, changelog, explain)
- split.rs bypassed this - now we fix it

**Use Plan/Executor pattern** (plan.rs + executor.rs)
- Makes execution declarative
- Enables dry-run mode
- Matches resolve command's safety pattern

**Load context** (context.rs)
- Project context from `.gitar/gitar.md`
- User context from `~/.gitar/gitar.md`
- Already cached via OnceLock

### Step 4: Extend for History Mode

History mode analyzes commits (not working tree):

```rust
// New in analyze.rs
pub async fn analyze_history(from: &str, to: Option<&str>) -> Result<AnalysisResult> {
    let range = match to {
        Some(t) => format!("{}..{}", from, t),
        None => format!("{}..HEAD", from),
    };

    // Get commit list
    let log = git::run_git(&["log", "--format=%H", &range])?;
    let commits: Vec<&str> = log.lines().collect();

    // Get cumulative diff
    let diff = git::run_git(&["diff", from, to.unwrap_or("HEAD")])?;

    // Parse file changes
    let files = parse_diff(&diff)?;

    Ok(AnalysisResult {
        mode: AnalysisMode::History { from: from.into(), to: to.map(Into::into) },
        diff,
        files,
        stats: DiffStats::default(),
    })
}
```

This enables history rewriting workflows (long-term goal).

---

## CLI Design

### New Command Structure

```bash
# Auto-detect mode (smart default)
gitar plan                    # Analyzes current repo state, suggests best mode
gitar plan --apply            # Execute after approval

# Explicit modes
gitar plan --mode working     # Unstaged + untracked (replaces split)
gitar plan --mode staged      # Only staged changes
gitar plan --mode history --from v1.0.0 --to HEAD

# Algorithm and configuration
gitar plan --algo 3           # Use hunk-level diff shaping
gitar plan --preset rust      # Rust commit style

# Non-interactive batch mode
gitar plan --apply --yes      # Auto-approve (dangerous, requires explicit --yes)
```

### Updated split Command

```bash
# Deprecated - internally calls `plan --mode working`
gitar split                   # Works but shows deprecation notice
gitar split --algo 3          # Now respects algo flag (fixed!)
```

### Backward Compatibility

- `split` remains as alias to `plan --mode working` for 6 months
- Deprecation notice: "Note: `gitar split` is deprecated. Use `gitar plan --mode working` instead."
- Remove in v2.0.0

---

## Safety & Validation

Following resolve command's 6-layer validation pattern:

### Pre-Execution Safety

1. **Diff algorithm validation** - Ensure shaped diff fits max_chars
2. **Secret detection** - Block/warn/redact before LLM call
3. **Context limits** - Estimate token usage per group
4. **Conflict detection** - Refuse to operate if unresolved conflicts exist

### Execution Safety

5. **Diff preview** - Show `git diff --cached` before each commit
6. **User confirmation** - Require explicit approval per commit
7. **State validation** - Verify files staged correctly
8. **Atomic operations** - Each commit is atomic (can rollback)

### Post-Execution Validation

9. **Commit verification** - Confirm commit created with correct message
10. **Working tree check** - Ensure no unexpected changes remain
11. **History integrity** - Verify no orphaned commits

---

## Test Strategy

### Unit Tests (in-file, per Rust Vibe Coding Standards)

Each module should have 100-200 lines of tests:

**analyze.rs tests:**
- `detect_mode_from_clean_repo()`
- `detect_mode_with_staged_changes()`
- `detect_mode_with_conflicts()`
- `parse_diff_multiple_files()`

**group.rs tests:**
- `group_by_heuristics_separates_docs_and_code()`
- `apply_smart_diff_respects_algo_flag()`
- `secret_detection_blocks_api_keys()`

**editor.rs tests:**
- `move_file_between_groups()`
- `exclude_file_removes_from_all_groups()`
- `regenerate_plan_calls_llm_again()`

**execute.rs tests:**
- `convert_groups_to_plan_structure()`
- `dry_run_does_not_modify_repo()`

### Integration Tests (tests/cli.rs pattern)

```rust
#[test]
fn plan_working_tree_mode() {
    setup_repo_with_changes();
    Command::cargo_bin("gitar")
        .args(&["plan", "--mode", "working"])
        .assert()
        .success();
}

#[test]
fn plan_respects_algo_flag() {
    Command::cargo_bin("gitar")
        .args(&["plan", "--algo", "3"])
        .assert()
        .success();
}
```

### Manual Testing Scenarios

1. **Empty repo** - Should suggest "working tree clean"
2. **Conflicts present** - Should suggest `gitar resolve`
3. **Mixed changes** - Should detect and group properly
4. **Large diff** - Should apply algorithm and not exceed max_chars
5. **Secrets in diff** - Should redact/warn/block based on config
6. **History mode** - Should analyze commit range correctly

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1)
- [ ] Create `command/plan/` directory structure
- [ ] Implement `analyze.rs` (working tree + staged modes)
- [ ] Write unit tests for analyze module
- [ ] Refactor split's `scan_diff()` into analyze module

**Deliverable:** `gitar plan` detects repo state and parses changes

### Phase 2: Smart Grouping (Week 1-2)
- [ ] Implement `group.rs` with diff algorithm integration
- [ ] Integrate `apply_smart_diff()` from command/mod.rs
- [ ] Port split's grouping heuristics
- [ ] Add LLM-based refinement
- [ ] Write unit tests for grouping logic

**Deliverable:** Plan generation with proper diff shaping + secret detection

### Phase 3: Interactive Editor (Week 2)
- [ ] Implement `editor.rs` with display and prompts
- [ ] Add move file between groups
- [ ] Add exclude file functionality
- [ ] Add regenerate plan (re-call LLM)
- [ ] Add edit message functionality
- [ ] Write unit tests for editor operations

**Deliverable:** Interactive plan refinement UI

### Phase 4: Safe Execution (Week 2-3)
- [ ] Implement `execute.rs` using Plan/Executor
- [ ] Add diff preview before commits
- [ ] Add per-commit confirmation
- [ ] Implement safety validation layers
- [ ] Write unit tests for execution

**Deliverable:** Safe commit execution with validation

### Phase 5: Integration & Polish (Week 3)
- [ ] Wire up all modules in plan/mod.rs
- [ ] Add CLI flags to cli.rs
- [ ] Update main.rs routing
- [ ] Add integration tests
- [ ] Deprecate split command
- [ ] Update CLAUDE.md documentation

**Deliverable:** Complete `gitar plan` command with working tree + staged modes

### Phase 6: History Mode (Week 4)
- [ ] Extend analyze.rs for history ranges
- [ ] Add history-specific grouping logic
- [ ] Add rebase-based execution for history rewriting
- [ ] Add comprehensive safety checks for history modification
- [ ] Add integration tests for history mode

**Deliverable:** Full history rewriting capability

---

## File Size Budget (Rust Vibe Coding Standards)

Target: 300-500 lines per file (600 hard limit)

| File | Logic | Tests | Total | Status |
|------|-------|-------|-------|--------|
| plan/mod.rs | 150 | 50 | 200 | ✅ Under limit |
| plan/analyze.rs | 200 | 150 | 350 | ✅ Under limit |
| plan/group.rs | 250 | 150 | 400 | ✅ Under limit |
| plan/editor.rs | 300 | 150 | 450 | ✅ Under limit |
| plan/execute.rs | 200 | 100 | 300 | ✅ Under limit |

**Current split.rs:** 474 lines (no tests) - REFACTORED across modules above

---

## Critical Files to Modify

### New Files
- `src/command/plan/mod.rs` (orchestrator, replaces current stub)
- `src/command/plan/analyze.rs` (state detection)
- `src/command/plan/group.rs` (grouping + LLM)
- `src/command/plan/editor.rs` (interactive UI)
- `src/command/plan/execute.rs` (safe execution)

### Modified Files
- `src/cli.rs` - Add mode flags to Plan command
- `src/main.rs` - Update routing for new plan signature
- `src/command/mod.rs` - Export plan submodules
- `src/command/split.rs` - Add deprecation notice, make it call plan

### Deleted Files (Future)
- `src/command/split.rs` - Remove in v2.0.0 after deprecation period

---

## Success Criteria

### Functional Requirements
✅ Analyze working tree, staged, and history ranges
✅ Apply diff algorithms (1-4) correctly
✅ Detect and handle secrets before LLM call
✅ Generate optimal commit groupings via LLM
✅ Support interactive refinement (move/exclude/regenerate)
✅ Execute safely with Plan/Executor infrastructure
✅ Show diff preview before each commit
✅ Validate all operations (6+ safety layers)

### Non-Functional Requirements
✅ Each file ≤ 500 lines (Rust Vibe Coding Standard)
✅ 80% test coverage (unit + integration)
✅ Follows "AI proposes, human approves, Git executes" philosophy
✅ Backward compatible (split → plan alias)
✅ Uses existing infrastructure (no NIH syndrome)
✅ Clear error messages with context

### User Experience
✅ `gitar plan` is the primary workflow command
✅ Auto-detects best mode from repo state
✅ Interactive editing feels natural (like resolve command)
✅ Dry-run mode shows what would happen
✅ Clear visual feedback at each step

---

## Risk Mitigation

### Risk: Breaking existing split users
**Mitigation:** Keep split as alias for 6 months with deprecation notice

### Risk: History mode is dangerous (rewriting commits)
**Mitigation:**
- Require explicit `--mode history` flag
- Show clear warning about rewriting history
- Implement extra confirmation prompt
- Add `--dry-run` to preview changes

### Risk: File size explosion (violating 500-line limit)
**Mitigation:**
- Strict module boundaries
- Regular refactoring checkpoints
- Use `cargo check` to track compile times
- Propose sub-module splits immediately if approaching 450 lines

### Risk: Test coverage gaps
**Mitigation:**
- Write tests alongside implementation (TDD approach)
- Target 80% coverage per module
- Add integration tests for critical paths
- Manual testing checklist before each milestone

---

## Open Questions for User

1. **History Mode Timeline:** Should history rewriting (Phase 6) be in the initial implementation, or phase 2 after working tree + staged modes are stable?

2. **Split Deprecation:** Is 6 months enough for deprecation period, or should we keep `split` indefinitely as an alias?

3. **Interactive UI Complexity:** The editor needs to handle move/exclude/edit operations. Should we start with basic accept/regenerate only, then add advanced features incrementally?

4. **Dry-Run Default:** Should `gitar plan` default to dry-run (require `--apply` to execute), or default to execute (require `--dry-run` to preview)?

---

## Verification Plan

### End-to-End Test Scenario

```bash
# Setup: Create test repo with mixed changes
git init test-plan
cd test-plan
echo "# Docs" > README.md
echo "fn main() {}" > src/main.rs
echo "[test]" > tests/test.rs
git add .
git commit -m "initial"

# Make changes across categories
echo "## More docs" >> README.md           # Docs change
echo "fn new() {}" >> src/main.rs          # Code change
echo "[test2]" >> tests/test.rs            # Test change
echo "debug=true" > .config.toml           # Config change

# Run plan command
gitar plan --mode working --algo 4

# Expected output:
# Gitar Context shows algo 4 + token estimates
# Proposes 4 commits:
#   1. docs: Update README
#   2. feat: Add new() function
#   3. test: Add test2
#   4. chore: Add debug config
# Interactive prompt: [Enter] Accept | [r] Regenerate | [m] Move file | ...

# User accepts
[Enter]

# Shows diff preview for commit 1
git diff --cached README.md

# Prompt: Apply commit 1/4? [Y/n]
Y

# Commit created
# Process repeats for commits 2, 3, 4

# Final validation
git log --oneline -4
# Should show 4 commits with correct messages

# Verify no uncommitted changes
git status
# Should be clean
```

---

## Philosophy Alignment

This spec embodies Gitar's core principles:

> **"AI proposes. Human approves. Git executes."**

- **AI proposes:** LLM analyzes diff, suggests optimal grouping and messages
- **Human approves:** Interactive editor allows refinement before execution
- **Git executes:** Safe, validated operations via Plan/Executor infrastructure

Never auto-rewrite. Never auto-commit without showing a plan. Never hide what Git is doing.

---

## Next Steps

1. **User Review:** Confirm this spec aligns with vision
2. **Timeline Approval:** 3-4 weeks reasonable for Phase 1-5?
3. **Answer Open Questions:** See section above
4. **Begin Implementation:** Start with Phase 1 (Foundation)
