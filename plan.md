# 🎸 Gitar — CLI Redesign Handoff (Final Short Syntax)

## Product Philosophy

Gitar is an **AI-native interface to Git history**.

It exposes a **small, stable command language**:

> **explain, fix, plan, release**

Everything is:

* Predictable
* Dry-run by default
* **Nothing mutates without `--apply`**
* No “magic” behavior
* No hidden mode switches

---

## Final Top-Level Verbs (LOCKED)

```
gitar plan
gitar explain
gitar fix
gitar release

(later) gitar sync
```

No other product-level verbs.

---

## Global Syntax Rule

> **No 3rd positional “object” word**

All commands follow:

```
gitar <verb> [--options] [REF]
```

Behavior is selected via **flags**, not subcommands.

---

## Global Defaults

```
gitar           == gitar plan        (dry-run)
gitar --apply   == gitar plan --apply
```

---

## Global Safety Rules

* Dry-run by default
* **Only `--apply` mutates**
* Same meaning everywhere

---

## Unified Scope Flags (Everywhere)

```
--working        # working tree
--staged         # index
--history <REF>  # committed history
```

Inference:

* If `--history` is given → history
* Else if staged exists → staged
* Else → working

**Do NOT use `--unstaged`.**
Use `--working` (matches Git’s real model).

---

# 🚀 `gitar plan`

Core product: analyze changes or history and build a multi-commit plan.

```
gitar plan
gitar plan -i
gitar plan --apply

gitar plan --working
gitar plan --staged
gitar plan --history REF

gitar plan --history REF -i --apply
```

Does:

* Analyze
* Propose multi-commit plan
* Show preview
* Interactive edit
* Execute only with `--apply`

---

# 📝 `gitar explain`

All **read-only understanding & communication**.

### Selector flags (exactly one, default = `--report`):

```
--commit
--pr
--changelog
--history
--version
--report    (old "explain changes in plain English")
```

### Syntax

```
gitar explain [--selector] [REF] [--working|--staged] [--preset X] [--algo N]
```

### Examples

```bash
gitar explain
gitar explain --report
gitar explain --report --staged

gitar explain --commit
gitar explain --commit --staged
gitar explain --commit --preset rust

gitar explain --pr
gitar explain --pr main

gitar explain --changelog v1.0.0
gitar explain --history v1.0.0

gitar explain --version
gitar explain --version v1.0.0
```

Algorithms and presets apply normally:

```bash
gitar explain --commit --algo 4
gitar explain --changelog v1.0.0 --algo 2
```

---

# 🩹 `gitar fix`

Replaces `resolve`.

```
gitar fix
gitar fix --apply
gitar fix --apply --yes
```

Behavior:

* Detect conflicts
* Heuristics first
* Per-region LLM
* Full-file LLM fallback
* Always preview
* Apply only with `--apply`

---

# 🧰 `gitar release`

```
gitar release
gitar release --apply
gitar release --from REF
gitar release --skip-changelog
gitar release --from REF --skip-changelog --apply
```

Does:

* Analyze commits
* Suggest version bump
* Generate changelog
* Update version files
* Create commit + tag
* Never auto-push

---

# ⚙️ Utilities (Not Product Verbs)

Remain flat:

```
gitar init
gitar config
gitar models
gitar hook --install
gitar hook --uninstall
gitar diff
```

`gitar diff` supports:

```
--algo N
--compare
--max-chars N
--stats
```

---

# 🧠 Mental Model

> Gitar manages Git history via:
>
> * **plan** = create / reshape history
> * **explain** = understand / communicate history
> * **fix** = repair history
> * **release** = ship history
> * (**sync** later = move history)

---

# 🔁 Compatibility Aliases (Optional but Recommended)

For migration / muscle memory:

```
gitar run        -> gitar plan
gitar resolve    -> gitar fix
gitar commit     -> gitar explain --commit
gitar pr         -> gitar explain --pr
gitar changelog  -> gitar explain --changelog
gitar history    -> gitar explain --history
gitar version    -> gitar explain --version
```

These should be **thin wrappers**, not real commands.

---

# ❗ Explicit Non-Goals

* No 3-word commands like: `gitar explain commit`
* No `--unstaged`
* No implicit mutation
* No cute verbs (`go`, `ship`, `why`, `msg`, etc.)
* No “magic mode switching”

---

# 📋 Migration Tasks

1. Rename:

   * `run` → `plan`
   * `resolve` → `fix`
2. Collapse narrate layer → `explain --...`
3. Rename old `explain` feature → `explain --report`
4. Implement scope flags: `--working`, `--staged`, `--history`
5. Enforce `--apply` safety everywhere
6. Update README and `--help`
7. (Optional) Add alias layer

---

# 🏁 One-line Product Statement

> **Gitar is an AI-native Git interface to plan, explain, fix, and release your history.**

