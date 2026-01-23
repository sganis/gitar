// src/prompt.rs
//! Cross-platform interactive prompts with arrow-key navigation
//!
//! Provides arrow-key navigable menus for TTY environments with automatic
//! fallback to text-based input for piped/non-TTY scenarios.

use anyhow::Result;
use std::io::{self, IsTerminal, Write};

pub use dialoguer::{Confirm, Input, Select};
use dialoguer::theme::Theme;
use dialoguer::console::style;

/// Custom theme with ASCII characters and cyan highlighting for selected items
pub struct GitarTheme;

impl Theme for GitarTheme {
    fn format_prompt(&self, f: &mut dyn std::fmt::Write, prompt: &str) -> std::fmt::Result {
        write!(f, "{}", style(prompt).cyan().bold())
    }

    fn format_error(&self, f: &mut dyn std::fmt::Write, err: &str) -> std::fmt::Result {
        write!(f, "{}", style(err).red())
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> std::fmt::Result {
        write!(f, "{} ", style(prompt).cyan().bold())?;
        match default {
            None => write!(f, "[y/n] "),
            Some(true) => write!(f, "[Y/n] "),
            Some(false) => write!(f, "[y/N] "),
        }
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> std::fmt::Result {
        write!(f, "{} ", prompt)?;
        match selection {
            Some(true) => write!(f, "{}", style("yes").green()),
            Some(false) => write!(f, "{}", style("no").red()),
            None => write!(f, ""),
        }
    }

    fn format_input_prompt(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> std::fmt::Result {
        write!(f, "{}", style(prompt).cyan().bold())?;
        if let Some(d) = default {
            write!(f, " [{}]", style(d).dim())?;
        }
        write!(f, ": ")
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> std::fmt::Result {
        write!(f, "{}: {}", prompt, style(sel).cyan())
    }

    fn format_select_prompt(&self, f: &mut dyn std::fmt::Write, prompt: &str) -> std::fmt::Result {
        write!(f, "{}", style(prompt).cyan().bold())
    }

    fn format_select_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> std::fmt::Result {
        write!(f, "{}: {}", prompt, style(sel).cyan())
    }

    fn format_multi_select_prompt(&self, f: &mut dyn std::fmt::Write, prompt: &str) -> std::fmt::Result {
        write!(f, "{}", style(prompt).cyan().bold())
    }

    fn format_multi_select_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        selections: &[&str],
    ) -> std::fmt::Result {
        write!(f, "{}: ", prompt)?;
        for (idx, sel) in selections.iter().enumerate() {
            if idx > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", style(sel).cyan())?;
        }
        Ok(())
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn std::fmt::Write,
        text: &str,
        active: bool,
    ) -> std::fmt::Result {
        if active {
            write!(f, "{} {}", style(">").cyan().bold(), style(text).cyan().bold())
        } else {
            write!(f, "  {}", text)
        }
    }

    fn format_multi_select_prompt_item(
        &self,
        f: &mut dyn std::fmt::Write,
        text: &str,
        checked: bool,
        active: bool,
    ) -> std::fmt::Result {
        let checkbox = if checked { "[X]" } else { "[ ]" };

        if active {
            write!(
                f,
                "{} {} {}",
                style(">").cyan().bold(),
                style(checkbox).cyan().bold(),
                style(text).cyan().bold()
            )
        } else if checked {
            write!(f, "  {} {}", style(checkbox).green(), text)
        } else {
            write!(f, "  {} {}", checkbox, text)
        }
    }

    fn format_sort_prompt_item(
        &self,
        f: &mut dyn std::fmt::Write,
        text: &str,
        picked: bool,
        active: bool,
    ) -> std::fmt::Result {
        if picked {
            write!(f, "  [{}] {}", style("x").dim(), style(text).dim())
        } else if active {
            write!(f, "{} [ ] {}", style(">").cyan().bold(), style(text).cyan().bold())
        } else {
            write!(f, "  [ ] {}", text)
        }
    }
}

/// Check if stdin/stdout are interactive terminals
pub fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Arrow-key select with numbered fallback for non-TTY
///
/// In TTY: Arrow keys + Enter to select (ASCII-only with cyan highlighting)
/// Non-TTY: Numbered list with text input
pub fn select<T: ToString + std::fmt::Display>(prompt: &str, items: &[T], default: usize) -> Result<usize> {
    if is_interactive() {
        let theme = GitarTheme;
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
/// In TTY: Arrow keys to toggle Yes/No + Enter (ASCII-only with cyan highlighting)
/// Non-TTY: y/n text input
pub fn confirm(prompt: &str, default: bool) -> Result<bool> {
    if is_interactive() {
        let theme = GitarTheme;
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
/// In TTY: Editable input line with default pre-filled (ASCII-only with cyan highlighting)
/// Non-TTY: Plain text input (default shown but not pre-filled)
pub fn input(prompt: &str, default: Option<&str>) -> Result<String> {
    if is_interactive() {
        let theme = GitarTheme;
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
