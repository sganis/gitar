// src/preset.rs
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
    /// Parse from string (CLI or config)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" | "rs" => Some(Self::Rust),
            "javascript" | "js" => Some(Self::JavaScript),
            "python" | "py" => Some(Self::Python),
            "default" | "auto" => None, // None means auto-detect
            _ => None,
        }
    }

    /// Auto-detect from repository root
    pub fn detect(repo_root: &Path) -> Self {
        // Check in priority order (most specific first)
        if repo_root.join("Cargo.toml").exists() {
            Self::Rust
        } else if repo_root.join("package.json").exists() {
            Self::JavaScript
        } else if repo_root.join("pyproject.toml").exists()
            || repo_root.join("setup.py").exists()
            || repo_root.join("requirements.txt").exists()
        {
            Self::Python
        } else {
            Self::Default
        }
    }

    /// Resolve preset: explicit > config > auto-detect
    pub fn resolve(
        cli_preset: Option<&String>,
        config_preset: Option<&String>,
        repo_root: &Path,
    ) -> Self {
        // Try CLI first
        if let Some(s) = cli_preset {
            if let Some(p) = Self::from_str(s) {
                return p;
            }
        }
        // Try config
        if let Some(s) = config_preset {
            if let Some(p) = Self::from_str(s) {
                return p;
            }
        }
        // Auto-detect
        Self::detect(repo_root)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Default => "default",
        }
    }

    /// Get style hints to inject into prompts
    pub fn hints(&self) -> PresetHints {
        match self {
            Self::Rust => PresetHints {
                tone: "technical, precise, module-focused",
                nouns: &["crate", "module", "workspace", "feature", "trait", "impl"],
                examples: &[
                    "Add streaming support to providers module",
                    "Fix panic when diff is empty",
                    "Refactor cli args into separate module",
                    "Handle None case in config resolution",
                ],
                avoid: &[
                    "vague 'Update stuff'",
                    "overstating performance without evidence",
                    "calling everything 'refactor' when it's a fix",
                ],
            },
            Self::JavaScript => PresetHints {
                tone: "user-facing, product-aware, concise",
                nouns: &["component", "hook", "route", "handler", "store", "util"],
                examples: &[
                    "Add dark mode toggle to settings page",
                    "Fix form validation on submit",
                    "Refactor auth logic into custom hook",
                    "Update dependencies for security patch",
                ],
                avoid: &[
                    "'Fix bug' with no scenario",
                    "Rust-like language (borrow, panic)",
                    "listing every file in subject",
                ],
            },
            Self::Python => PresetHints {
                tone: "behavior-focused, clear, library-minded",
                nouns: &["module", "package", "endpoint", "model", "util", "cli"],
                examples: &[
                    "Add retry logic to API client",
                    "Fix edge case in date parsing",
                    "Improve type hints for core module",
                    "Handle empty response in fetch_data",
                ],
                avoid: &[
                    "JS-ish phrasing like 'Bump package-lock'",
                    "omitting key behavior changes",
                    "vague 'Update stuff'",
                ],
            },
            Self::Default => PresetHints {
                tone: "clear, informative, action-oriented",
                nouns: &["module", "feature", "component", "function"],
                examples: &[
                    "Add support for custom configuration",
                    "Fix error handling in main loop",
                    "Refactor validation logic for clarity",
                    "Update documentation for new API",
                ],
                avoid: &[
                    "vague messages like 'Update stuff'",
                    "listing files instead of describing changes",
                ],
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PresetHints {
    pub tone: &'static str,
    pub nouns: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub avoid: &'static [&'static str],
}

impl PresetHints {
    /// Format hints for injection into system prompt
    pub fn format(&self, preset_name: &str) -> String {
        let nouns = self.nouns.join(", ");
        let examples = self
            .examples
            .iter()
            .map(|e| format!("  - \"{}\"", e))
            .collect::<Vec<_>>()
            .join("\n");
        let avoid = self
            .avoid
            .iter()
            .map(|a| format!("  - {}", a))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"
## Style ({} project)
Tone: {}
Prefer nouns like: {}
Good examples:
{}
Avoid:
{}"#,
            preset_name, self.tone, nouns, examples, avoid
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
    fn from_str_rust() {
        assert_eq!(Preset::from_str("rust"), Some(Preset::Rust));
        assert_eq!(Preset::from_str("rs"), Some(Preset::Rust));
    }

    #[test]
    fn from_str_javascript() {
        assert_eq!(Preset::from_str("javascript"), Some(Preset::JavaScript));
        assert_eq!(Preset::from_str("js"), Some(Preset::JavaScript));
    }

    #[test]
    fn from_str_python() {
        assert_eq!(Preset::from_str("python"), Some(Preset::Python));
        assert_eq!(Preset::from_str("py"), Some(Preset::Python));
    }

    #[test]
    fn from_str_auto() {
        assert_eq!(Preset::from_str("auto"), None);
    }

    #[test]
    fn from_str_unknown() {
        assert_eq!(Preset::from_str("unknown"), None);
        assert_eq!(Preset::from_str("go"), None);
    }

    #[test]
    fn detect_rust() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Rust);
    }

    #[test]
    fn detect_javascript() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::JavaScript);
    }

    #[test]
    fn detect_python_pyproject() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Python);
    }

    #[test]
    fn detect_python_setup() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("setup.py"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Python);
    }

    #[test]
    fn detect_python_requirements() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Python);
    }

    #[test]
    fn detect_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Default);
    }

    #[test]
    fn detect_rust_over_js() {
        // If both exist, Rust wins (checked first)
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(Preset::detect(dir.path()), Preset::Rust);
    }

    #[test]
    fn resolve_cli_overrides_all() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap(); // Would detect Rust
        let cli = "python".to_string();
        let config = "javascript".to_string();
        let preset = Preset::resolve(Some(&cli), Some(&config), dir.path());
        assert_eq!(preset, Preset::Python);
    }

    #[test]
    fn resolve_config_overrides_detect() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let config = "javascript".to_string();
        let preset = Preset::resolve(None, Some(&config), dir.path());
        assert_eq!(preset, Preset::JavaScript);
    }

    #[test]
    fn resolve_falls_back_to_detect() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        let preset = Preset::resolve(None, None, dir.path());
        assert_eq!(preset, Preset::JavaScript);
    }

    #[test]
    fn hints_format_contains_sections() {
        let hints = Preset::Rust.hints();
        let formatted = hints.format("rust");
        assert!(formatted.contains("Style (rust project)"));
        assert!(formatted.contains("Tone:"));
        assert!(formatted.contains("Prefer nouns like:"));
        assert!(formatted.contains("Good examples:"));
        assert!(formatted.contains("Avoid:"));
    }

    #[test]
    fn hints_rust_contains_crate() {
        let hints = Preset::Rust.hints();
        assert!(hints.nouns.contains(&"crate"));
        assert!(hints.nouns.contains(&"module"));
    }

    #[test]
    fn hints_js_contains_component() {
        let hints = Preset::JavaScript.hints();
        assert!(hints.nouns.contains(&"component"));
        assert!(hints.nouns.contains(&"hook"));
    }

    #[test]
    fn hints_python_contains_endpoint() {
        let hints = Preset::Python.hints();
        assert!(hints.nouns.contains(&"endpoint"));
        assert!(hints.nouns.contains(&"model"));
    }

    #[test]
    fn preset_name() {
        assert_eq!(Preset::Rust.name(), "rust");
        assert_eq!(Preset::JavaScript.name(), "javascript");
        assert_eq!(Preset::Python.name(), "python");
        assert_eq!(Preset::Default.name(), "default");
    }
}