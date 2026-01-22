// src/executor.rs
use anyhow::Result;

use crate::git::run_git;
use crate::plan::{Action, Plan};

pub struct Executor;

impl Executor {
    pub fn execute(plan: &Plan, dry_run: bool) -> Result<()> {
        for step in &plan.steps {
            match step {
                Action::Suggest { .. } => {
                    // No side effects.
                }
                Action::Git { args, .. } => {
                    if dry_run {
                        continue;
                    }
                    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                    let _out = run_git(&refs)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{Action, Plan};

    #[test]
    fn executor_dry_run_does_not_fail() {
        let mut p = Plan::new("x");
        p.push(Action::git(&["--version"])); // would work, but dry_run must not execute
        assert!(Executor::execute(&p, true).is_ok());
    }
}
