// src/context/preset.rs - Language-specific commit message styles

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    Rust,
    JavaScript,
    Python,
    #[default]
    Default,
}

impl Preset {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(Self::Rust),
            "javascript" | "js" => Some(Self::JavaScript),
            "python" | "py" => Some(Self::Python),
            "default" | "auto" => None,
            _ => None,
        }
    }

    pub fn detect(root: &Path) -> Self {
        if root.join("Cargo.toml").exists() {
            Self::Rust
        } else if root.join("package.json").exists() {
            Self::JavaScript
        } else if root.join("pyproject.toml").exists()
            || root.join("setup.py").exists()
            || root.join("requirements.txt").exists()
        {
            Self::Python
        } else {
            Self::Default
        }
    }

    pub fn resolve(cli: Option<&String>, config: Option<&String>, root: &Path) -> Self {
        if let Some(s) = cli {
            if let Some(p) = Self::from_str(s) {
                return p;
            }
        }
        if let Some(s) = config {
            if let Some(p) = Self::from_str(s) {
                return p;
            }
        }
        Self::detect(root)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Default => "default",
        }
    }

    pub fn hints(&self) -> Hints {
        match self {
            Self::Rust => Hints {
                tone: "technical, precise, module-focused",
                nouns: &["crate", "module", "workspace", "feature", "trait", "impl"],
                examples: &[
                    "Add streaming support to providers module",
                    "Fix panic when diff is empty",
                    "Refactor cli args into separate module",
                ],
                avoid: &[
                    "vague 'Update stuff'",
                    "overstating performance",
                    "calling everything 'refactor'",
                ],
            },
            Self::JavaScript => Hints {
                tone: "user-facing, product-aware, concise",
                nouns: &["component", "hook", "route", "handler", "store", "util"],
                examples: &[
                    "Add dark mode toggle to settings page",
                    "Fix form validation on submit",
                    "Refactor auth logic into custom hook",
                ],
                avoid: &[
                    "'Fix bug' with no context",
                    "Rust-like terms",
                    "listing files in subject",
                ],
            },
            Self::Python => Hints {
                tone: "behavior-focused, clear, library-minded",
                nouns: &["module", "package", "endpoint", "model", "util", "cli"],
                examples: &[
                    "Add retry logic to API client",
                    "Fix edge case in date parsing",
                    "Improve type hints for core module",
                ],
                avoid: &[
                    "JS phrasing like 'Bump package-lock'",
                    "omitting key behavior changes",
                ],
            },
            Self::Default => Hints {
                tone: "clear, informative, action-oriented",
                nouns: &["module", "feature", "component", "function"],
                examples: &[
                    "Add support for custom configuration",
                    "Fix error handling in main loop",
                    "Refactor validation logic for clarity",
                ],
                avoid: &[
                    "vague 'Update stuff'",
                    "listing files instead of describing changes",
                ],
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hints {
    pub tone: &'static str,
    pub nouns: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub avoid: &'static [&'static str],
}

impl Hints {
    pub fn format(&self, name: &str) -> String {
        format!(
            r#"
## Style ({} project)
Tone: {}
Prefer nouns: {}
Examples:
{}
Avoid:
{}"#,
            name,
            self.tone,
            self.nouns.join(", "),
            self.examples
                .iter()
                .map(|e| format!("  - \"{}\"", e))
                .collect::<Vec<_>>()
                .join("\n"),
            self.avoid
                .iter()
                .map(|a| format!("  - {}", a))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn from_str_all() {
        assert_eq!(Preset::from_str("rust"), Some(Preset::Rust));
        assert_eq!(Preset::from_str("rs"), Some(Preset::Rust));
        assert_eq!(Preset::from_str("javascript"), Some(Preset::JavaScript));
        assert_eq!(Preset::from_str("js"), Some(Preset::JavaScript));
        assert_eq!(Preset::from_str("python"), Some(Preset::Python));
        assert_eq!(Preset::from_str("py"), Some(Preset::Python));
        assert_eq!(Preset::from_str("auto"), None);
        assert_eq!(Preset::from_str("unknown"), None);
    }

    #[test]
    fn detect_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Rust);
    }

    #[test]
    fn detect_js() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::JavaScript);
    }

    #[test]
    fn detect_python() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Python);
    }

    #[test]
    fn detect_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Default);
    }

    #[test]
    fn resolve_cli_wins() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let cli = "python".to_string();
        assert_eq!(
            Preset::resolve(Some(&cli), None, dir.path()),
            Preset::Python
        );
    }

    #[test]
    fn hints_format() {
        let h = Preset::Rust.hints().format("rust");
        assert!(h.contains("Style (rust project)"));
        assert!(h.contains("crate"));
    }
}
