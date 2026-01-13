import subprocess
import os
import sys
import time
import json
from pathlib import Path
from openai import OpenAI, RateLimitError
import dotenv

dotenv.load_dotenv()

# =============================================================================
# PROMPTS
# =============================================================================

COMMIT_SYSTEM_PROMPT = """You are an expert software engineer who writes clear, informative Git commit messages.

## Commit Message Format
```
<Type>(<scope>):
<description line 1>
<description line 2 if needed>
<more lines for complex changes>
```

## Types
- Feat: New feature
- Fix: Bug fix
- Refactor: Code restructuring without behavior change
- Docs: Documentation changes
- Style: Formatting, whitespace (no code logic change)
- Test: Adding or modifying tests
- Chore: Build process, dependencies, config
- Perf: Performance improvement

## Rules
1. First line: Type(scope): only, capitalized (no description on this line)
2. Following lines: describe WHAT changed and WHY
3. Scale detail to complexity: simple changes get 1-2 lines, complex changes get more
4. Use imperative mood ("Add" not "Added")
5. Be specific about impact and reasoning

## Examples

Simple change:
Feat(docker):
Add 'll' alias for directory listing.

Medium change:
Fix(api):
Handle null response from payment gateway.
Prevents 500 errors when gateway times out during peak traffic.

Complex change:
Refactor(auth):
Extract token validation into dedicated middleware.
Centralizes JWT verification logic previously duplicated across 5 controllers.
Adds automatic token refresh for requests within 5 minutes of expiry.
Improves testability by isolating auth concerns."""

COMMIT_USER_PROMPT = """Generate a commit message for this diff.
First line: Type(scope): only (capitalized, nothing else on this line)
Following lines: describe what and why (1-5 lines depending on complexity)

**Original message (if any):** {original_message}

**Diff:**
```
{diff}
```

Respond with ONLY the commit message (no markdown, no extra explanation)."""

# -----------------------------------------------------------------------------

PR_SYSTEM_PROMPT = """You are a senior engineer writing a PR description for code review.

## Output Format
```
## Summary
Brief 1-2 sentence overview of the change.

## What Changed
- Bullet points of key changes
- Be specific about files/components affected

## Why
Motivation, context, or issue being solved.

## Risks & Considerations
- Potential issues or areas needing careful review
- Performance, security, backwards compatibility concerns
- "None identified" if truly low-risk

## Testing
- How this was tested
- Suggested manual testing steps
- Areas needing extra verification

## Rollout Notes
- Any deployment considerations
- Feature flags, migrations, dependencies
- "Standard deployment" if nothing special
```

Be concise but thorough. Flag anything reviewers should pay extra attention to."""

PR_USER_PROMPT = """Generate a PR description for this diff.

**Branch:** {branch}
**Commits:**
{commits}

**File stats:**
{stats}

**Diff:**
```
{diff}
```

Respond with the PR description in the format specified (no extra commentary)."""

# -----------------------------------------------------------------------------

CHANGELOG_SYSTEM_PROMPT = """You are a technical writer creating release notes from Git commits.

## Output Format
```
# Release Notes

## ✨ New Features
- Feature descriptions grouped logically

## 🐛 Bug Fixes
- Fix descriptions

## 🔧 Improvements
- Refactors, performance, DX improvements

## 📖 Documentation
- Doc changes

## ⚠️ Breaking Changes
- Any backwards-incompatible changes (highlight clearly)

## 🏗️ Infrastructure
- CI/CD, dependencies, config changes
```

Rules:
1. Group related changes together
2. Write for end-users/stakeholders, not devs
3. Skip trivial changes (typos, formatting)
4. Highlight breaking changes prominently
5. Omit empty sections"""

CHANGELOG_USER_PROMPT = """Generate release notes from these commits.

**Range:** {range}
**Commit count:** {count}

**Commits with messages:**
{commits}

Respond with release notes in the format specified."""

# -----------------------------------------------------------------------------

EXPLAIN_SYSTEM_PROMPT = """You are explaining code changes to a non-technical stakeholder (PM, designer, exec).

## Rules
1. NO jargon - translate technical terms
2. Focus on USER IMPACT - what changes for the product/users?
3. Be brief - 3-5 bullet points max
4. Call out anything visible to users
5. Mention if QA/testing is recommended
6. Use analogies if helpful

## Output Format
```
## What's Changing
Brief plain-English summary.

## User Impact
- How this affects the product/users
- Visible changes (if any)
- Performance/reliability changes (if any)

## Risk Level
Low / Medium / High + brief explanation

## Recommended Actions
- Any QA, communication, or documentation needed
```"""

EXPLAIN_USER_PROMPT = """Explain this code change for a non-technical person (PM/stakeholder).

**Diff stats:**
{stats}

**Diff:**
```
{diff}
```

Respond with a plain-English explanation (no code, no jargon)."""

# -----------------------------------------------------------------------------

VERSION_SYSTEM_PROMPT = """You analyze code changes to recommend semantic version bumps.

## Semantic Versioning Rules
- MAJOR (X.0.0): Breaking changes - removed/renamed APIs, changed behavior, schema migrations requiring data changes
- MINOR (0.X.0): New features, new endpoints, new optional parameters, deprecations
- PATCH (0.0.X): Bug fixes, performance improvements, internal refactors, documentation

## Output Format
```
Recommendation: MAJOR|MINOR|PATCH

Reasoning:
- Key point 1
- Key point 2

Breaking changes: Yes/No
- List if any
```

Be conservative - when in doubt, go higher."""

VERSION_USER_PROMPT = """Analyze this diff and recommend a semantic version bump.

**Current version:** {version}
**Diff:**
```
{diff}
```

Respond with your recommendation and reasoning."""

# =============================================================================
# GIT UTILITIES
# =============================================================================

# Files to exclude from diffs (noisy/generated)
EXCLUDE_PATTERNS = [
    ":(exclude)*.lock",
    ":(exclude)package-lock.json",
    ":(exclude)yarn.lock", 
    ":(exclude)pnpm-lock.yaml",
    ":(exclude)dist/*",
    ":(exclude)build/*",
    ":(exclude)*.min.js",
    ":(exclude)*.min.css",
    ":(exclude)*.map",
    ":(exclude).env*",
    ":(exclude)*.pyc",
    ":(exclude)__pycache__/*",
]


def get_openai_client() -> OpenAI:
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("Error: OPENAI_API_KEY not found in environment.", file=sys.stderr)
        sys.exit(1)
    return OpenAI(api_key=api_key)


def run_git(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git"] + args,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace"
    )


def is_git_repo() -> bool:
    return run_git(["rev-parse", "--git-dir"]).returncode == 0


def get_current_branch() -> str:
    result = run_git(["branch", "--show-current"])
    return result.stdout.strip() or "HEAD"


def get_default_branch() -> str:
    """Try to find main/master branch."""
    for branch in ["main", "master"]:
        if run_git(["rev-parse", "--verify", branch]).returncode == 0:
            return branch
    return "main"


def get_diff(target: str = None, staged: bool = False, max_chars: int = 15000) -> str:
    """Get diff with exclusions and smart truncation."""
    args = ["diff", "--unified=3"]
    
    if staged:
        args.append("--cached")
    elif target:
        args.append(target)
    
    args.append("--")
    args.append(".")
    args.extend(EXCLUDE_PATTERNS)
    
    result = run_git(args)
    diff = result.stdout
    
    if len(diff) > max_chars:
        truncated = diff[:max_chars]
        last_file = truncated.rfind("\ndiff --git")
        if last_file > max_chars // 2:
            truncated = truncated[:last_file]
        truncated += "\n\n[... diff truncated ...]"
        return truncated
    
    return diff


def get_diff_stats(target: str = None, staged: bool = False) -> str:
    """Get diff --stat summary."""
    args = ["diff", "--stat"]
    if staged:
        args.append("--cached")
    elif target:
        args.append(target)
    result = run_git(args)
    return result.stdout.strip()


def get_commit_logs(limit: int = None, since: str = None, range_spec: str = None) -> list[dict]:
    args = ["log", "--pretty=format:%H|%an|%ad|%s", "--date=iso"]
    
    if range_spec:
        args.append(range_spec)
    if limit:
        args.append(f"-n{limit}")
    if since:
        args.append(f"--since={since}")
    
    result = run_git(args)
    
    commits = []
    for line in result.stdout.strip().split("\n"):
        if not line:
            continue
        parts = line.split("|", 3)
        if len(parts) >= 4:
            commits.append({
                "hash": parts[0],
                "author": parts[1],
                "date": parts[2],
                "message": parts[3]
            })
    return commits


def get_commit_diff(commit_hash: str, max_chars: int = 12000) -> str | None:
    parent_check = run_git(["rev-parse", f"{commit_hash}^"])
    
    if parent_check.returncode != 0:
        args = ["diff-tree", "--patch", "--unified=3", "--root", commit_hash, "--"]
    else:
        args = ["diff", f"{commit_hash}^!", "--unified=3", "--"]
    
    args.append(".")
    args.extend(EXCLUDE_PATTERNS)
    
    result = run_git(args)
    
    if result.returncode != 0:
        return None
    
    diff = result.stdout
    if len(diff) > max_chars:
        truncated = diff[:max_chars]
        last_file = truncated.rfind("\ndiff --git")
        if last_file > max_chars // 2:
            truncated = truncated[:last_file]
        truncated += "\n\n[... diff truncated ...]"
        return truncated
    
    return diff


def get_file_tree(max_depth: int = 2) -> str:
    """Get a simple file tree for context."""
    result = run_git(["ls-tree", "-r", "--name-only", "HEAD"])
    files = result.stdout.strip().split("\n")[:50]
    return "\n".join(files)


def get_current_version() -> str:
    """Try to get current version from tags."""
    result = run_git(["describe", "--tags", "--abbrev=0"])
    if result.returncode == 0:
        return result.stdout.strip()
    return "0.0.0"


# =============================================================================
# AI UTILITIES
# =============================================================================

def call_ai(
    client: OpenAI,
    system_prompt: str,
    user_prompt: str,
    model: str = "gpt-4o",
    max_tokens: int = 500,
    max_retries: int = 3
) -> str | None:
    for attempt in range(max_retries):
        try:
            response = client.chat.completions.create(
                model=model,
                messages=[
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_prompt}
                ],
                max_tokens=max_tokens,
                temperature=0.3
            )
            return response.choices[0].message.content.strip()
        except RateLimitError:
            wait_time = 2 ** attempt * 5
            print(f"Rate limited, waiting {wait_time}s...")
            time.sleep(wait_time)
        except Exception as e:
            print(f"API error: {e}", file=sys.stderr)
            return None
    return None


# =============================================================================
# COMMANDS
# =============================================================================

def cmd_commits(args, client: OpenAI):
    """Generate commit messages for history."""
    print("Fetching commit history...")
    commits = get_commit_logs(limit=args.limit, since=args.since)
    
    if not commits:
        print("No commits found.")
        return
    
    print(f"Processing {len(commits)} commits...\n")
    results = []
    
    for i, commit in enumerate(commits, 1):
        short_hash = commit["hash"][:8]
        date_short = commit["date"][:10]
        
        print(f"[{i}/{len(commits)}] {short_hash} | {date_short} | {commit['author'][:15]:<15} | {commit['message'][:40]}")
        
        diff = get_commit_diff(commit["hash"])
        
        if not diff or not diff.strip():
            print("  ⚠ No diff available, skipping")
            continue
        
        prompt = COMMIT_USER_PROMPT.format(
            original_message=commit["message"] or "(none)",
            diff=diff
        )
        
        new_message = call_ai(client, COMMIT_SYSTEM_PROMPT, prompt, model=args.model, max_tokens=300)
        
        if new_message:
            msg_lines = new_message.strip().split("\n")
            print(f"  ✓ {msg_lines[0]}")
            for line in msg_lines[1:]:
                if line.strip():
                    print(f"    {line}")
            results.append({
                "hash": commit["hash"],
                "original": commit["message"],
                "suggested": new_message
            })
        else:
            print("  ✗ Failed to generate message")
        
        if i < len(commits):
            time.sleep(args.delay)
    
    if args.output:
        with open(args.output, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to: {args.output}")


def cmd_pr(args, client: OpenAI):
    """Generate PR description."""
    branch = get_current_branch()
    base = args.base or get_default_branch()
    
    print(f"Generating PR description: {branch} → {base}\n")
    
    # Get diff
    diff_target = f"{base}...{branch}" if branch != base else None
    
    if args.staged:
        diff = get_diff(staged=True)
        stats = get_diff_stats(staged=True)
        commits_text = "(staged changes)"
    else:
        diff = get_diff(diff_target)
        stats = get_diff_stats(diff_target)
        commits = get_commit_logs(range_spec=f"{base}..{branch}")
        commits_text = "\n".join([f"- {c['message']}" for c in commits[:20]]) or "(no commits)"
    
    if not diff.strip():
        print("No changes detected.")
        return
    
    prompt = PR_USER_PROMPT.format(
        branch=branch,
        commits=commits_text,
        stats=stats,
        diff=diff
    )
    
    result = call_ai(client, PR_SYSTEM_PROMPT, prompt, model=args.model, max_tokens=1000)
    
    if result:
        print(result)
        if args.output:
            with open(args.output, "w") as f:
                f.write(result)
            print(f"\n(Saved to {args.output})")
    else:
        print("Failed to generate PR description.")


def cmd_changelog(args, client: OpenAI):
    """Generate changelog / release notes."""
    if args.since_tag:
        range_spec = f"{args.since_tag}..HEAD"
        range_display = f"{args.since_tag} → HEAD"
    elif args.since:
        range_spec = None
        range_display = f"since {args.since}"
    else:
        range_spec = None
        range_display = "recent commits"
    
    print(f"Generating changelog for {range_display}...\n")
    
    commits = get_commit_logs(limit=args.limit, since=args.since, range_spec=range_spec)
    
    if not commits:
        print("No commits found.")
        return
    
    commits_text = "\n".join([
        f"- [{c['hash'][:8]}] {c['message']}" for c in commits
    ])
    
    prompt = CHANGELOG_USER_PROMPT.format(
        range=range_display,
        count=len(commits),
        commits=commits_text
    )
    
    result = call_ai(client, CHANGELOG_SYSTEM_PROMPT, prompt, model=args.model, max_tokens=1500)
    
    if result:
        print(result)
        if args.output:
            with open(args.output, "w") as f:
                f.write(result)
            print(f"\n(Saved to {args.output})")
    else:
        print("Failed to generate changelog.")


def cmd_explain(args, client: OpenAI):
    """Explain changes for non-technical stakeholders."""
    base = args.base or get_default_branch()
    branch = get_current_branch()
    
    print(f"Generating PM-friendly explanation...\n")
    
    if args.staged:
        diff = get_diff(staged=True)
        stats = get_diff_stats(staged=True)
    else:
        diff_target = f"{base}...{branch}" if branch != base else None
        diff = get_diff(diff_target)
        stats = get_diff_stats(diff_target)
    
    if not diff.strip():
        print("No changes detected.")
        return
    
    prompt = EXPLAIN_USER_PROMPT.format(stats=stats, diff=diff)
    
    result = call_ai(client, EXPLAIN_SYSTEM_PROMPT, prompt, model=args.model, max_tokens=800)
    
    if result:
        print(result)
    else:
        print("Failed to generate explanation.")


def cmd_version(args, client: OpenAI):
    """Suggest semantic version bump."""
    base = args.base or get_default_branch()
    branch = get_current_branch()
    current = args.current or get_current_version()
    
    print(f"Analyzing changes for version bump (current: {current})...\n")
    
    diff_target = f"{base}...{branch}" if branch != base else None
    diff = get_diff(diff_target)
    
    if not diff.strip():
        print("No changes detected.")
        return
    
    prompt = VERSION_USER_PROMPT.format(version=current, diff=diff)
    
    result = call_ai(client, VERSION_SYSTEM_PROMPT, prompt, model=args.model, max_tokens=400)
    
    if result:
        print(result)
    else:
        print("Failed to analyze version.")


# =============================================================================
# MAIN
# =============================================================================

def main():
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Git AI Assistant - generate commit messages, PR descriptions, changelogs, and more"
    )
    parser.add_argument("--model", default="gpt-4o", help="OpenAI model (default: gpt-4o)")
    
    subparsers = parser.add_subparsers(dest="command", help="Command to run")
    
    # commits
    p_commits = subparsers.add_parser("commits", help="Generate commit messages for history")
    p_commits.add_argument("-n", "--limit", type=int, help="Number of commits")
    p_commits.add_argument("--since", help="Since date (e.g., '2024-01-01')")
    p_commits.add_argument("-o", "--output", help="Save to JSON file")
    p_commits.add_argument("--delay", type=float, default=0.5, help="Delay between API calls")
    
    # pr
    p_pr = subparsers.add_parser("pr", help="Generate PR description")
    p_pr.add_argument("--base", help="Base branch (default: main/master)")
    p_pr.add_argument("--staged", action="store_true", help="Use staged changes instead")
    p_pr.add_argument("-o", "--output", help="Save to file")
    
    # changelog
    p_changelog = subparsers.add_parser("changelog", help="Generate release notes")
    p_changelog.add_argument("--since-tag", help="Since tag (e.g., 'v1.0.0')")
    p_changelog.add_argument("--since", help="Since date")
    p_changelog.add_argument("-n", "--limit", type=int, default=50, help="Max commits")
    p_changelog.add_argument("-o", "--output", help="Save to file")
    
    # explain
    p_explain = subparsers.add_parser("explain", help="Explain changes for PM/stakeholders")
    p_explain.add_argument("--base", help="Base branch")
    p_explain.add_argument("--staged", action="store_true", help="Use staged changes")
    
    # version
    p_version = subparsers.add_parser("version", help="Suggest semantic version bump")
    p_version.add_argument("--base", help="Base branch")
    p_version.add_argument("--current", help="Current version (default: from tags)")
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return
    
    if not is_git_repo():
        print("Error: Not a git repository", file=sys.stderr)
        sys.exit(1)
    
    client = get_openai_client()
    
    commands = {
        "commits": cmd_commits,
        "pr": cmd_pr,
        "changelog": cmd_changelog,
        "explain": cmd_explain,
        "version": cmd_version,
    }
    
    commands[args.command](args, client)


if __name__ == "__main__":
    main()