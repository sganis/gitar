# 🎸 Gitar — Final CLI Redesign Handoff (2026-01)

## Product Philosophy

Gitar is an **AI-native interface to Git history**.

Not a bag of commands.
A **small, stable language** to:

> **plan, explain, fix, and release** history.

The CLI must be:

* Predictable
* Safe
* Boring (in a good way)
* Discoverable
* With **strong defaults** and **no magic mutations**

---

## Final Core Verbs (LOCKED)

```
gitar plan
gitar explain
gitar fix
gitar release

(later) gitar sync
```

Nothing else is a product-level verb.

---

## Global Safety Rules

* **Dry-run by default**
* **Nothing mutates without `--apply`**
* `--apply` has the same meaning everywhere: *execute the plan*

---

## Defaults

```
gitar           == gitar plan
gitar --apply   == gitar plan --apply
```

---

## Unified Scope Flags (Everywhere)

Replace “modes” with:

```
--working
--staged
--history <ref>
```

Inference:

* If `--history` → history
* Else if staged exists → staged
* Else → working

Examples:

```bash
gitar plan
gitar plan --history v1.0.0
gitar explain commit --staged
```

---

## `gitar plan` (Core Product)

Replaces old: `run`, `split`, multi-commit planner, history rewrite entrypoint.

```
gitar plan
gitar plan -i
gitar plan --apply
gitar plan --history v1.0.0
```

Does:

* Analyze changes or history
* Build multi-commit plan
* Show preview
* Allow interactive edit
* Execute only with `--apply`

---

## `gitar explain` (All Read-Only Narration)

This replaces the entire “narrate” layer.

Everything that **describes, summarizes, explains, or communicates** goes here.

```
gitar explain commit
gitar explain pr
gitar explain changelog v1.0.0
gitar explain history v1.0.0
gitar explain version
gitar explain report
```

### Important Rename

Old:

```
gitar explain    # plain English explanation of changes
```

New:

```
gitar explain report
```

`explain` is now the **category verb**, not a single feature.

---

## `gitar fix` (Replaces `resolve`)

```
gitar fix
gitar fix --apply
gitar fix --yes
```

Behavior:

* Detect conflicts
* Try heuristics
* Fall back to LLM (per-region, then full-file)
* Show preview
* Apply only with `--apply`

---

## `gitar release`

```
gitar release
gitar release --apply
gitar release --from v1.0.0
gitar release --skip-changelog
```

Does:

* Analyze commits
* Suggest version bump
* Generate changelog
* Update version files
* Create tag
* Never auto-push

---

## Utilities (Not Product Verbs)

Remain flat:

```
gitar init
gitar config
gitar models
gitar hook install
gitar diff
```

---

## Grammar Rule

Everything follows:

```
gitar <verb> [object] [scope] [--apply]
```

Examples:

```
gitar explain pr
gitar explain commit --staged
gitar plan --history v1.0.0
gitar fix --apply
```

---

## Mental Model

> Gitar manages Git history through four actions:
>
> * **plan** = create / reshape history
> * **explain** = understand / communicate history
> * **fix** = repair history
> * **release** = ship history
> * (**sync** later = move history)

---

## UX Goals

* Typing `gitar` should “just work”
* Few verbs
* Strong defaults
* No accidental mutations
* Feels like an AI copilot, not a git wrapper

---

## Migration Tasks

1. Rename `run` → `plan`
2. Rename `resolve` → `fix`
3. Move all narrate commands under `explain`
4. Rename old `explain` feature → `explain report`
5. Update README
6. Update Clap command tree
7. (Optionally) keep old commands as hidden aliases for transition

---

## One-line Product Statement

> **Gitar is an AI-native Git interface to plan, explain, fix, and release your history.**

---

## Non-Goals (For Now)

* No “cute” verbs (`go`, `ship`, `why`, `msg`, etc.)
* No magic commands
* No implicit mutation
* No exploding command surface

---

## Future

Add:

```
gitar sync
```

To cover pull/push/fetch/rebase as a **planned, safe operation**.

---

