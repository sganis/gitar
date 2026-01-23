# Plan: Arrow-Key Navigable Interactive Menus

## Summary
Replace numbered text menus with modern arrow-key navigable selection using **dialoguer** crate, with fallback to text input for non-TTY environments.

## Library Choice: dialoguer

**Why dialoguer:**
- Lightweight (uses `console` crate, ~2 transitive deps)
- Simple API: `Select`, `Confirm`, `Input`
- Cross-platform (Windows, macOS, Linux)
- Aligns with project's minimal-dependency philosophy

## Files to Modify

### 1. New Files
| File | Purpose |
|------|---------|
| `src/prompt.rs` | Centralized prompt utilities wrapping dialoguer |

### 2. Existing Files (9 files)
| File | Changes |
|------|---------|
| `Cargo.toml` | Add `dialoguer = "0.11"` |
| `src/lib.rs` | Export `pub mod prompt;` |
| `src/command/init/mod.rs` | Provider/model/preset selection → `Select` |
| `src/command/commit/mod.rs` | Accept/regenerate/edit menu → `Select` |
| `src/command/plan/editor.rs` | Plan editing menu → `Select` |
| `src/command/plan/execute.rs` | Commit confirmation → `Select` |
| `src/command/rewrite/mod.rs` | Per-commit actions → `Select` |
| `src/command/resolve/mod.rs` | Yes/no prompts → `Confirm` |
| `src/command/resolve/llm.rs` | Region action menu → `Select` |
| `src/command/squash/mod.rs` | Squash confirmation → `Confirm` |
| `src/command/release/mod.rs` | Version input + confirmation → `Input` + `Confirm` |

## Implementation

### Step 1: Add dependency
```toml
# Cargo.toml
dialoguer = "0.11"
```

### Step 2: Create `src/prompt.rs`
```rust
// src/prompt.rs
use anyhow::Result;
use std::io::{self, IsTerminal, Write};
pub use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Arrow-key select with numbered fallback for non-TTY
pub fn select<T: ToString>(prompt: &str, items: &[T], default: usize) -> Result<usize> {
    if is_interactive() {
        Ok(Select::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()?)
    } else {
        fallback_select(prompt, items, default)
    }
}

/// Yes/no with arrow keys or y/n fallback
pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    if is_interactive() {
        Ok(Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default)
            .interact()?)
    } else {
        fallback_confirm(prompt, default)
    }
}

/// Text input with optional default
pub fn input(prompt: &str, default: Option<&str>) -> Result<String> {
    if is_interactive() {
        let mut b = Input::with_theme(&ColorfulTheme::default()).with_prompt(prompt);
        if let Some(d) = default { b = b.default(d.to_string()); }
        Ok(b.interact_text()?)
    } else {
        fallback_input(prompt, default)
    }
}

// Fallback implementations for piped/non-TTY environments...
```

### Step 3: Update commands (examples)

**Before (`init/mod.rs:167-196`):**
```rust
println!("\nAvailable providers:");
for (i, provider) in providers.iter().enumerate() {
    println!("  {}. {}", i + 1, provider);
}
let choice = prompt_with_default(&prompt, "1")?;
```

**After:**
```rust
use crate::prompt;
let idx = prompt::select("Select provider", &providers, default_idx)?;
Ok(providers[idx].to_string())
```

**Before (`commit/mod.rs:111-136`):**
```rust
println!("  [Enter] Accept | [g] Regenerate | [e] Edit | [other] Cancel");
let mut input = String::new();
io::stdin().read_line(&mut input)?;
match input.trim().to_lowercase().as_str() { ... }
```

**After:**
```rust
use crate::prompt;
let options = ["Accept", "Regenerate", "Edit message", "Cancel"];
match prompt::select("Action", &options, 0)? {
    0 => break msg,
    1 => { println!("Regenerating..."); continue; }
    2 => { let new = prompt::input("New message", Some(&msg))?; break new; }
    _ => { println!("Canceled."); return Ok(()); }
}
```

## Execution Order

1. **Foundation**: `Cargo.toml` + `src/prompt.rs` + `src/lib.rs`
2. **Simple confirmations**: `squash/mod.rs`, `release/mod.rs`, `resolve/mod.rs`
3. **Select menus**: `commit/mod.rs`, `init/mod.rs`, `rewrite/mod.rs`
4. **Complex menus**: `plan/editor.rs`, `plan/execute.rs`, `resolve/llm.rs`

## Verification

1. `cargo build` - ensure compilation
2. `cargo test` - run existing tests
3. Manual testing:
   - `gitar init` - test provider/model/preset selection with arrow keys
   - `gitar commit` - test accept/regenerate/edit menu
   - `gitar plan` - test plan editor menu
   - `echo "1" | gitar init` - verify fallback works for piped input
