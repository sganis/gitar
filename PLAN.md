# Gitar CLI API Improvement Plan

## Overview

Improve the gitar CLI by removing redundancy, enhancing integrations, and adding new features while keeping the flat command structure.

**Core Focus**: Git narrate, plan, and execute. AI-powered git operations, NOT code analysis or security scanning.

## Current Commands (17)

| Command | Purpose | Change |
|---------|---------|--------|
| `commit` | Create commit with AI message | Add `--amend` flag |
| `staged` | Generate message for staged | Keep |
| `unstaged` | Generate message for unstaged | Keep |
| `history` | Describe commits | Keep |
| `pr` | Generate PR description | Keep |
| `changelog` | Generate release notes | Keep |
| `explain` | Explain for stakeholders | Keep |
| `version` | Suggest version bump | **Remove** → `release --bump` |
| `plan` | Multi-commit planning | Add `--resolve` flag |
| `resolve` | Conflict resolution | Keep |
| `release` | Version + changelog + tag | Add `--bump` flag (absorb version) |
| `init` | Create config | Keep |
| `config` | Show config | Keep |
| `hook` | Manage hooks | Keep |
| `models` | List models | Keep |
| `diff` | Debug diff | Keep |

## Proposed Commands (16 current + 2 future = 18)

```
gitar
├── commit [--amend]         # Add amend support
├── staged                   # Keep as-is
├── unstaged                 # Keep as-is
├── history                  # Keep as-is
├── pr                       # Keep as-is
├── changelog                # Keep as-is
├── explain                  # Keep as-is
├── plan [--resolve]         # Add resolve integration
├── resolve                  # Keep as-is
├── release [--bump]         # Absorb version analysis
├── init                     # Keep as-is
├── config                   # Keep as-is
├── hook                     # Keep as-is
├── models                   # Keep as-is
├── diff                     # Keep as-is
│
├── squash                   # [NEW] Squash with AI message
└── rewrite                  # [NEW] Interactive history rewrite
```

---

## Key Changes

### 1. Remove `version` command → `release --bump`

The standalone `version` command is removed. Its LLM-powered analysis moves into `release`:

```bash
# Before
gitar version v1.0.0          # Analyze and suggest bump

# After
gitar release --bump auto     # LLM analyzes, suggests version
gitar release --bump minor    # Override: force minor bump
gitar release --bump patch    # Override: force patch bump
gitar release --bump major    # Override: force major bump
```

**Implementation:**
- Remove `Commands::Version` variant from cli.rs
- Remove `cmd_version` from main.rs dispatch
- Move LLM version analysis logic from `command/version/mod.rs` to `command/release/mod.rs`
- Replace naive heuristic in release with LLM call
- Add `--bump` flag to release command

### 2. Add `--amend` to `commit`

Regenerate message for last commit and amend it:

```bash
gitar commit --amend    # Get diff of HEAD, regenerate message, git commit --amend
```

**Implementation:**
- Add `amend: bool` flag to `Commands::Commit`
- In `cmd_commit`: if amend, get diff of HEAD~1..HEAD, generate message, run `git commit --amend -m "..."`

### 3. Add `--resolve` to `plan`

Auto-resolve conflicts before planning:

```bash
gitar plan --resolve --apply  # Resolve conflicts first, then execute plan
```

**Implementation:**
- Add `resolve: bool` flag to `Commands::Plan`
- In `cmd_plan`: if conflicts detected and `--resolve` flag set, call `cmd_resolve` first

---

## New Commands

### 4. `squash` - Squash Commits

Squash recent commits with AI-generated message:

```bash
gitar squash 3            # Squash last 3 commits into 1
gitar squash HEAD~5       # Squash commits since HEAD~5
gitar squash v1.0.0       # Squash commits since tag
```

**Implementation:**
- Collect commits in range
- Generate unified commit message via LLM
- Execute `git reset --soft` + `git commit`

### 5. `rewrite` - Interactive History Rewrite

Interactive history rewriting with AI-generated messages:

```bash
gitar rewrite HEAD~5      # Rewrite last 5 commits
gitar rewrite v1.0.0      # Rewrite commits since tag
```

**Implementation:**
- Similar to `history` but actually modifies commits
- Interactive: show each commit, propose new message, confirm
- Uses `git rebase --exec` or similar mechanism

---

## Implementation Phases

### Phase 1: Remove Version + Enhance Release
1. Add `--bump` flag to release command (auto|major|minor|patch)
2. Move LLM version analysis from `cmd_version` to `cmd_release`
3. Remove `version` command
4. Update tests

**Files:**
- `src/cli.rs` - Remove Version variant, add bump to Release
- `src/main.rs` - Remove version dispatch
- `src/command/release/mod.rs` - Add LLM version analysis
- `src/command/version/mod.rs` - Remove file

### Phase 2: Add --amend to commit
1. Add `amend: bool` flag to Commit
2. Implement amend logic in `cmd_commit`
3. Add tests

**Files:**
- `src/cli.rs` - Add amend flag
- `src/command/commit/mod.rs` - Implement amend logic

### Phase 3: Add --resolve to plan
1. Add `resolve: bool` flag to Plan
2. Call `cmd_resolve` when conflicts detected and flag set
3. Add tests

**Files:**
- `src/cli.rs` - Add resolve flag
- `src/command/plan/mod.rs` - Integrate resolve

### Phase 4: Add squash command
1. Add `Commands::Squash` variant
2. Create `command/squash/mod.rs`
3. Implement squash logic with LLM message generation

**Files:**
- `src/cli.rs` - Add Squash variant
- `src/main.rs` - Add dispatch
- `src/command/squash/mod.rs` - New file (~200 lines)

### Phase 5: Add rewrite command
1. Add `Commands::Rewrite` variant
2. Create `command/rewrite/mod.rs`
3. Implement interactive rebase with LLM messages

**Files:**
- `src/cli.rs` - Add Rewrite variant
- `src/main.rs` - Add dispatch
- `src/command/rewrite/mod.rs` - New file (~250 lines)

---

## Verification

### Phase 1 (release --bump)
```bash
cargo test
gitar release --bump auto      # Should use LLM analysis
gitar release --bump minor     # Should force minor
gitar version                  # Should fail: command not found
```

### Phase 2 (commit --amend)
```bash
cargo test
gitar commit                   # Normal commit
gitar commit --amend           # Should amend with new message
```

### Phase 3 (plan --resolve)
```bash
cargo test
# Create merge conflict, then:
gitar plan --resolve --apply   # Should resolve then plan
```

### Phase 4-5 (new commands)
```bash
cargo test
gitar squash 2                 # Should squash last 2 commits
gitar rewrite HEAD~3           # Should rewrite last 3 commits
```

---

## Command Summary

| Before (17) | After (16) | Future (+2) |
|-------------|------------|-------------|
| version | ❌ removed | |
| release | release --bump | |
| commit | commit --amend | |
| plan | plan --resolve | |
| | | squash |
| | | rewrite |
