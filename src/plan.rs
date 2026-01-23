// src/plan.rs
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Plan {
    pub title: String,
    pub steps: Vec<Action>,
}

impl Plan {
    #[allow(dead_code)]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            steps: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, step: Action) {
        self.steps.push(step);
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Action {
    /// A high-level “do this next” step (no side effects yet).
    Suggest {
        label: String,
        detail: Option<String>,
    },

    /// A git command that would be executed (execution engine will handle it later).
    Git {
        args: Vec<String>,
        label: Option<String>,
    },
}

impl Action {
    #[allow(dead_code)]
    pub fn suggest(label: impl Into<String>) -> Self {
        Self::Suggest {
            label: label.into(),
            detail: None,
        }
    }

    #[allow(dead_code)]
    pub fn suggest_with_detail(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Suggest {
            label: label.into(),
            detail: Some(detail.into()),
        }
    }

    #[allow(dead_code)]
    pub fn git(args: &[&str]) -> Self {
        Self::Git {
            args: args.iter().map(|s| s.to_string()).collect(),
            label: None,
        }
    }

    #[allow(dead_code)]
    pub fn git_labeled(label: impl Into<String>, args: &[&str]) -> Self {
        Self::Git {
            args: args.iter().map(|s| s.to_string()).collect(),
            label: Some(label.into()),
        }
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.title)?;
        writeln!(f, "{}", "-".repeat(self.title.len()))?;

        for (i, step) in self.steps.iter().enumerate() {
            match step {
                Action::Suggest { label, detail } => {
                    writeln!(f, "{}. {}", i + 1, label)?;
                    if let Some(d) = detail {
                        writeln!(f, "   {}", d)?;
                    }
                }
                Action::Git { args, label } => {
                    if let Some(l) = label {
                        writeln!(f, "{}. {}:", i + 1, l)?;
                        writeln!(f, "   git {}", args.join(" "))?;
                    } else {
                        writeln!(f, "{}. git {}", i + 1, args.join(" "))?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_display_is_stable() {
        let mut p = Plan::new("Plan");
        p.push(Action::suggest("Resolve conflicts"));
        p.push(Action::git_labeled("Stage all", &["add", "-A"]));

        let s = p.to_string();
        assert!(s.contains("Plan"));
        assert!(s.contains("1. Resolve conflicts"));
        assert!(s.contains("2. Stage all:"));
        assert!(s.contains("git add -A"));
    }
}
