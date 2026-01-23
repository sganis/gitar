// src/prompt.rs
//! Cross-platform interactive prompts with arrow-key navigation
//!
//! Provides arrow-key navigable menus for TTY environments with automatic
//! fallback to text-based input for piped/non-TTY scenarios.

use anyhow::Result;
use std::io::{self, IsTerminal, Write};

pub use dialoguer::{theme::SimpleTheme, Confirm, Input, Select};

/// Check if stdin/stdout are interactive terminals
pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Arrow-key select with numbered fallback for non-TTY
///
/// In TTY: Arrow keys + Enter to select (ASCII-only, no unicode)
/// Non-TTY: Numbered list with text input
pub fn select<T: ToString + std::fmt::Display>(prompt: &str, items: &[T], default: usize) -> Result<usize> {
    if is_interactive() {
        let theme = SimpleTheme;
        Ok(Select::with_theme(&theme)
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()?)
    } else {
        fallback_select(prompt, items, default)
    }
}

/// Yes/no confirmation with arrow keys or y/n fallback
///
/// In TTY: Arrow keys to toggle Yes/No + Enter (ASCII-only, no unicode)
/// Non-TTY: y/n text input
pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    if is_interactive() {
        let theme = SimpleTheme;
        Ok(Confirm::with_theme(&theme)
            .with_prompt(prompt)
            .default(default)
            .interact()?)
    } else {
        fallback_confirm(prompt, default)
    }
}

/// Text input with optional default value
///
/// In TTY: Editable input line with default pre-filled (ASCII-only, no unicode)
/// Non-TTY: Plain text input (default shown but not pre-filled)
pub fn input(prompt: &str, default: Option<&str>) -> Result<String> {
    if is_interactive() {
        let theme = SimpleTheme;
        let mut builder = Input::with_theme(&theme).with_prompt(prompt);
        if let Some(d) = default {
            builder = builder.default(d.to_string());
        }
        Ok(builder.interact_text()?)
    } else {
        fallback_input(prompt, default)
    }
}

// =============================================================================
// FALLBACK IMPLEMENTATIONS FOR NON-TTY ENVIRONMENTS
// =============================================================================

fn fallback_select<T: ToString>(prompt: &str, items: &[T], default: usize) -> Result<usize> {
    eprintln!("\n{}", prompt);
    for (i, item) in items.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, item.to_string());
    }
    eprint!("Enter choice [1-{}] (default: {}): ", items.len(), default + 1);
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        return Ok(default);
    }

    match input.parse::<usize>() {
        Ok(n) if n > 0 && n <= items.len() => Ok(n - 1),
        _ => {
            eprintln!("Invalid choice. Using default: {}", default + 1);
            Ok(default)
        }
    }
}

fn fallback_confirm(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    eprint!("{} [{}]: ", prompt, default_str);
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input.is_empty() {
        return Ok(default);
    }

    match input.as_str() {
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => {
            eprintln!("Invalid input. Using default: {}", if default { "yes" } else { "no" });
            Ok(default)
        }
    }
}

fn fallback_input(prompt: &str, default: Option<&str>) -> Result<String> {
    if let Some(d) = default {
        eprint!("{} [{}]: ", prompt, d);
    } else {
        eprint!("{}: ", prompt);
    }
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        if let Some(d) = default {
            return Ok(d.to_string());
        }
    }

    Ok(input.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_interactive() {
        // Just verify it doesn't panic
        let _ = is_interactive();
    }

    #[test]
    fn test_fallback_functions_exist() {
        // Ensure fallback functions compile
        let items = vec!["a", "b", "c"];
        let _ = fallback_select::<&str>;
        let _ = fallback_confirm;
        let _ = fallback_input;
    }
}
