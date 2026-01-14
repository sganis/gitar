# gitar
The Git AI in Rust

# Changelog
gitar changelog v1.0.0              # All commits since tag
gitar changelog HEAD~10             # Last 10 commits  
gitar changelog abc1234             # Since specific commit
gitar changelog                     # Recent 50 commits (default)
gitar changelog --since "1 week ago"
gitar changelog v1.0.0 --since "2024-01-01" --until "2024-06-01"
gitar changelog -n 20               # Limit to 20

# Commits (generate messages for history)
gitar commits v1.0.0                # All commits since tag
gitar commits -n 5                  # Last 5 commits
gitar commits --since "yesterday"

# PR
gitar pr                            # Against base_branch (from config)
gitar pr develop                    # Against develop
gitar pr v1.0.0                     # Against tag
gitar pr --staged                   # From staged changes only

# Explain
gitar explain                       # Current branch vs base
gitar explain v1.0.0                # Changes since tag
gitar explain HEAD~5                # Last 5 commits
gitar explain --staged              # Staged changes only

# Version
gitar version                       # Analyze vs base branch
gitar version v1.0.0                # Analyze since tag
gitar version --current 1.2.3       # Specify current version
