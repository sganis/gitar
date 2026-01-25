# Handoff: Improve `gitar plan` commit grouping quality

## Goal

Improve the quality and consistency of `gitar plan` so it produces **better commit groups** (more coherent, less mixing, better separation of refactors/docs/formatting/tests) **without turning into an open-ended LLM loop**.

Primary success criteria:

* Groups are **topic-cohesive** and “make sense” as independent commits/PR chunks
* Clear separation of:

  * formatting-only
  * docs-only
  * refactors (rename/move) vs feature changes
  * tests tied to code changes
* Deterministic, testable behavior: **0–1 LLM calls by default**, optional bounded second pass

Non-goals:

* No autonomous retry loops
* No “keep asking until good”
* Don’t break existing CLI signature or UX

---

## Current mental model

`gitar plan` is a **skill**:

* builds context (diff + repo state)
* optionally calls LLM
* outputs a **Plan** (groups + rationale + steps)
* executor applies plan (or user executes manually)

Weakness observed:

* LLM grouping is sometimes not good enough; alternate groupings would be better.

---

## Architecture changes (what to improve)

### 1) Add a dedicated “Planning Context” pipeline (highest impact)

Create a `planning_context` builder distinct from “tell/release” context.

**Inputs to extract**

* Git status + staged/unstaged summary
* File list with stats per file:

  * added/removed lines
  * file type (code/test/doc/config)
  * path prefixes (src/, tests/, docs/)
* Change fingerprints:

  * rename/move detection (from `git diff --name-status`)
  * formatting-only detection (heuristic)
  * dependency/config changes (Cargo.toml, package.json, lock files)
* Optional: hunk-level clusters (only if cheap)

**Outputs**
A compact JSON-ish summary (or structured text) optimized for grouping:

* `files[]` with tags: `{path, kind, churn, renamed?, test?, doc?, config?}`
* `signals[]` such as: “large rename set”, “mostly whitespace”, “touches tests+src”
* `constraints` defaults (see below)

Keep it small, stable, and testable.

---

### 2) Introduce constraints + objective into the planner prompt

Stop asking the LLM “group the commits” generally.

Add explicit **hard constraints** (must follow):

* Separate formatting-only into its own group
* Separate docs-only into its own group
* Keep renames/moves together (or separate “rename-only”)
* Keep tests with the feature they validate, unless tests are broad refactor
* Limit group size: `max_files_per_group`, `max_groups` (defaults)
* Avoid mixing unrelated top-level dirs unless justified

Add **objective** (soft goals):

* maximize cohesion (same feature/module)
* minimize cross-topic mixing
* minimize risk (big refactors isolated)
* produce an order: low-risk first, risky last

---

### 3) Add a deterministic plan scorer (no LLM)

After the LLM returns a plan, score it and optionally repair.

**Plan scoring heuristics (examples)**
Penalty points for:

* group mixes `docs` and `src` (unless small + justified)
* group mixes `formatting-only` with functional changes
* group contains both large rename set + feature edits (unless justified)
* group touches many unrelated path roots
* group exceeds size caps

Bonus points for:

* single-module cohesion (same directory subtree)
* tests paired with related src changes
* clear rationale mentions constraints and risks

**Outcome**

* If plan score is below threshold: either

  * pick best among multiple candidates (preferred), or
  * do a bounded “repair” step (optional second call)

---

### 4) Single-call “multi-candidate” output (recommended)

Keep **one LLM call**, but request **2–3 alternative plans**.

LLM output schema should contain:

* `candidates[]` each with `groups[]`, `rationale`, `risk`, `confidence`
* Then your deterministic scorer selects best candidate.

This avoids loops and gives the model room to explore.

---

### 5) Optional bounded second pass (only if needed)

If you allow a second call:

* Call #1 returns candidates
* Scorer picks best and identifies top 1–2 violations
* Call #2 prompt: “Repair ONLY these violations; keep everything else identical.”

Hard rule: max 2 calls, no iterative loop.

---

## Data model (introduce / align types)

Create/extend these types (names can follow your Rust style):

* `PlanningContext`
* `PlanCandidate`
* `Plan`
* `PlanGroup`
* `PlanScore` (+ reasons)

`PlanGroup` should include:

* `title`
* `summary`
* `files[]` (or commits/hunks if you have them)
* `tags[]` (doc/refactor/format/test/feature)
* `risk` and `why`

---

## Prompt contract (must be strict)

Require **structured JSON** output (or JSON in fenced block) to parse reliably.
Must include:

* `candidates` array (2–3)
* each candidate: groups with file lists + rationale
* `assumptions` and `open_questions` empty unless needed
* never include prose-only answers

If parsing fails → fallback to heuristic-only grouping or a simpler LLM prompt.

---

## Implementation plan (steps)

1. Build `planning_context` builder
2. Implement `grouping_signals` heuristics:

   * whitespace/format-only detector
   * docs-only
   * rename-heavy
   * test coupling
3. Add deterministic `score_plan(candidate, context) -> score + reasons`
4. Update LLM prompt for `gitar plan`:

   * constraints + objective + context summary
   * request 2–3 candidates
5. Select best candidate by score
6. Add tests:

   * unit tests for detectors
   * unit tests for scorer
   * golden tests for prompt parsing
   * integration test: known repo fixture → stable grouping

---

## Testing strategy

Create fixtures (small git repos in tests) that represent:

* formatting-only change
* docs-only change
* rename-only refactor + small follow-up edits
* feature change + tests
* mixed changes across modules

Assertions:

* formatting-only gets isolated
* docs-only gets isolated
* renames not mixed with unrelated changes
* tests grouped with feature
* scorer rejects obviously mixed plans

---

## UX requirements

* CLI remains stable
* Output shows:

  * chosen plan score + top reasons
  * group list with tags and risk
* If plan is low confidence, show “suggested alternative grouping” (from other candidate), but still choose best automatically.

---

## Deliverables

* New module(s):

  * `src/command/plan/context.rs`
  * `src/command/plan/scoring.rs`
  * `src/command/plan/prompt.rs`
  * `src/command/plan/model.rs`
* Tests for detectors + scoring + parsing
* Minimal docs update for `gitar plan` explaining constraints and scoring

---

## Hard rules

* Default: **max 1 LLM call**
* Optional: bounded second pass only (max 2 calls)
* No “keep trying until good”
* Deterministic scorer decides final plan

