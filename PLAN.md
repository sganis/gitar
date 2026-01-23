# Gitar Vision Completion Plan

> **Mission**: "Gitar is an AI-native Git interface that helps you understand your history, plan it, and safely execute it"

## Current State Summary

### Four Conceptual Layers
| Layer | Status | Commands |
|-------|--------|----------|
| Narrate (read-only) | **MATURE** | changelog, explain, pr, history, version |
| Plan (commit planning) | **EXISTS** (needs polish) | plan |
| Release (guided workflow) | **NOT IMPLEMENTED** | - |
| Resolve (conflict resolution) | **COMPLETE** | resolve |

### Issues Found
1. **Code duplication**: Context loading (commit vs repo.rs), diff pipeline pattern (8+ commands)
2. **Oversized files**: algo.rs (824 lines), analyze.rs (546 lines)
3. **Deprecated code**: split command still present
4. **Incomplete features**: plan history mode, release command

---

## Implementation Plan

### Phase 1: Split Oversized Files

**1.1 Split `src/context/algo.rs` (824 lines → 5 files)**

Create `src/context/algo/` directory:
```
src/context/algo/
├── mod.rs      (~100 lines) - Types, shape_diff(), re-exports
├── full.rs     (~80 lines)  - Algorithm 1: full diff
├── file.rs     (~100 lines) - Algorithm 2: selective files
├── hunk.rs     (~120 lines) - Algorithm 3: selective hunks
└── semantic.rs (~200 lines) - Algorithm 4: semantic JSON
```

**1.2 Extract shared utilities to `src/util/file.rs`**

Move duplicated functions:
- `categorize_file()` from plan/analyze.rs (lines 404-451)
- `group_by_heuristics()` from plan/group.rs (lines 109-177)

New file: `src/util/file.rs` (~80 lines)

---

### Phase 2: Consolidate Duplicated Code

**2.1 Remove duplicate context loading from `src/command/commit/mod.rs`**

Delete lines 15-55 (home_dir, load_user_context, load_project_context).
Use `crate::context::repo::load_all_context()` instead.

**2.2 Create shared pipeline module `src/pipeline/`**

```
src/pipeline/
├── mod.rs  (~20 lines) - Re-exports
└── diff.rs (~100 lines) - DiffRequest, DiffResponse, process_diff()
```

```rust
pub struct DiffRequest<'a> {
    pub raw_diff: &'a str,
    pub max_chars: usize,
    pub algo: u8,
    pub secret_action: SecretAction,
    pub silent: bool,
}

pub fn process_diff(req: &DiffRequest) -> Result<DiffResponse>
```

Refactor these commands to use pipeline:
- explain, pr, changelog, version, history, commit

---

### Phase 3: Complete Plan Command

**3.1 Implement history mode execution**

File: `src/command/plan/execute.rs` (lines 40-42 currently just warns)

Add `execute_history_mode()`:
- Build rebase-todo script from commit groups
- Execute interactive rebase with prepared script
- Handle conflicts gracefully

**3.2 Add editor capabilities**

File: `src/command/plan/editor.rs`

New actions:
- `merge 1-3` - Merge commits 1 through 3 into single commit
- `reorder 3 1` - Move commit 3 to position 1
- `split 2` - Split commit 2 into multiple (re-analyze files)

---

### Phase 4: Implement Release Command

**New directory: `src/command/release/`**

```
src/command/release/
├── mod.rs     (~200 lines) - Main workflow orchestration
├── version.rs (~100 lines) - Version file detection/update
└── tag.rs     (~80 lines)  - Tag creation helpers
```

**Workflow:**
1. Analyze commits since last tag (or --from ref)
2. Call existing version suggestion logic
3. Generate changelog (reuse cmd_changelog)
4. Detect version files (Cargo.toml, package.json, pyproject.toml)
5. Update version files
6. Create release commit
7. Create annotated tag
8. Display summary (never auto-push)

**CLI:**
```rust
Release {
    #[arg(long)] apply: bool,           // Execute (default: dry-run)
    #[arg(long)] skip_changelog: bool,  // Skip changelog generation
    #[arg(long)] from: Option<String>,  // Base ref (default: latest tag)
}
```

---

### Phase 5: Cleanup

**5.1 Remove deprecated split command**

Delete:
- `src/command/split/mod.rs` (478 lines)
- References in cli.rs, main.rs, command/mod.rs

**5.2 Fix remaining warnings**

- Remove `#![allow(unused_imports)]` from context/mod.rs
- Remove `#![allow(dead_code)]` from context/algo/mod.rs after migration

---

## Final Module Structure

```
src/
├── main.rs, lib.rs, cli.rs, client.rs, config.rs
├── git.rs, types.rs, plan.rs, executor.rs
├── context/
│   ├── mod.rs, repo.rs, secret.rs, preset.rs, template.rs
│   └── algo/
│       ├── mod.rs, full.rs, file.rs, hunk.rs, semantic.rs
├── pipeline/
│   ├── mod.rs, diff.rs
├── util/
│   ├── mod.rs, diff.rs, file.rs (NEW)
└── command/
    ├── commit/, changelog/, explain/, pr/, version/, history/
    ├── diff/, config/, hook/, init/, models/
    ├── plan/
    │   ├── mod.rs, analyze.rs, group.rs, editor.rs, execute.rs
    ├── resolve/
    │   ├── mod.rs, parser.rs, heuristic.rs, llm.rs, diff_preview.rs, git_helper.rs
    └── release/ (NEW)
        ├── mod.rs, version.rs, tag.rs
```

---

## Critical Files to Modify

| File | Action | Lines Affected |
|------|--------|----------------|
| `src/context/algo.rs` | Split into 5 files | All 824 lines |
| `src/command/commit/mod.rs` | Remove duplicate code | Lines 15-55 |
| `src/command/plan/execute.rs` | Add history mode | Lines 40-42 + new function |
| `src/command/plan/editor.rs` | Add merge/reorder | Add ~100 lines |
| `src/cli.rs` | Add Release, remove Split | ~20 lines |
| `src/main.rs` | Add Release routing | ~10 lines |
| `src/command/split/mod.rs` | DELETE | 478 lines removed |

---

## Verification Plan

1. **After each phase**: Run `cargo check` and `cargo test`
2. **After algo split**: Run `gitar diff --compare` to verify algorithms work
3. **After pipeline refactor**: Test each command manually with a sample repo
4. **After release command**: Test dry-run flow in test repo
5. **Final**: Run full test suite `cargo test -- --nocapture`

---

## Execution Order

1. Phase 1.1: Split algo.rs (foundation for everything else)
2. Phase 1.2: Extract util/file.rs
3. Phase 2.1: Remove commit duplication
4. Phase 2.2: Create pipeline module
5. Phase 3.1: Plan history mode
6. Phase 3.2: Plan editor improvements
7. Phase 4: Release command
8. Phase 5: Cleanup (remove split, fix warnings)
