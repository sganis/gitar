// src/color.rs
//! Cross-platform terminal color utilities
//!
//! Provides consistent, colorized output for success, warning, error, and info messages
//! that works across Windows, Linux, and macOS terminals.

use anstream::println as aprintln;
use anstyle::{AnsiColor, Color, Style};
use std::fmt;

/// Style for success messages (green)
pub fn success_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green)))
}

/// Style for warning messages (yellow)
pub fn warning_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
}

/// Style for error messages (red)
pub fn error_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Red)))
}

/// Style for info messages (cyan)
pub fn info_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
}

/// Style for dimmed text (gray)
pub fn dim_style() -> Style {
    Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlack)))
}

/// Style for bold text
pub fn bold_style() -> Style {
    Style::new().bold()
}

/// Format text with a style
pub struct Styled<T> {
    text: T,
    style: Style,
}

impl<T: fmt::Display> fmt::Display for Styled<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}", self.style.render(), self.text, self.style.render_reset())
    }
}

/// Create a styled text object
pub fn styled<T>(text: T, style: Style) -> Styled<T> {
    Styled { text, style }
}

/// Print a success message (green checkmark)
pub fn success(msg: impl fmt::Display) {
    aprintln!("{} {}", styled("[OK]", success_style()), msg);
}

/// Print a warning message (yellow warning)
pub fn warning(msg: impl fmt::Display) {
    aprintln!("{} {}", styled("[WARN]", warning_style()), msg);
}

/// Print an error message (red error)
pub fn error(msg: impl fmt::Display) {
    aprintln!("{} {}", styled("[ERROR]", error_style()), msg);
}

/// Print an info message (cyan info)
pub fn info(msg: impl fmt::Display) {
    aprintln!("{} {}", styled("[INFO]", info_style()), msg);
}

/// Print a dimmed message
pub fn dim(msg: impl fmt::Display) {
    aprintln!("{}", styled(msg, dim_style()));
}

/// Format an arrow for before/after comparisons
pub fn arrow() -> &'static str {
    "->"
}

/// Format a bullet point
pub fn bullet() -> &'static str {
    "*"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_styles_dont_panic() {
        let _ = success_style();
        let _ = warning_style();
        let _ = error_style();
        let _ = info_style();
        let _ = dim_style();
        let _ = bold_style();
    }

    #[test]
    fn test_styled_display() {
        let s = styled("test", success_style());
        let _ = format!("{}", s);
    }

    #[test]
    fn test_arrow_and_bullet() {
        assert_eq!(arrow(), "->");
        assert_eq!(bullet(), "*");
    }
}
