// src/context/secret.rs - Secret detection and redaction
//
// Scans diffs for sensitive data before sending to LLMs.
// Detects: API keys, tokens, private keys, passwords, connection strings.

use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

// =============================================================================
// PUBLIC TYPES
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SecretAction {
    #[default]
    Redact,
    Warn,
    Block,
}

impl SecretAction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "redact" => Some(Self::Redact),
            "warn" => Some(Self::Warn),
            "block" => Some(Self::Block),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct SecretMatch {
    pub pattern_name: String,
    pub severity: Severity,
    pub line_number: usize,
    pub line_preview: String,
    pub match_start: usize,
    pub match_end: usize,
}

impl SecretMatch {
    pub fn masked_preview(&self) -> String {
        let line = &self.line_preview;
        if self.match_start >= line.len() {
            return line.clone();
        }
        let end = self.match_end.min(line.len());
        let secret = &line[self.match_start..end];
        format!(
            "{}{}{}",
            &line[..self.match_start],
            mask(secret),
            &line[end..]
        )
    }
}

#[derive(Debug, Default)]
pub struct SecretScanResult {
    pub matches: Vec<SecretMatch>,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
}

impl SecretScanResult {
    pub fn from_matches(matches: Vec<SecretMatch>) -> Self {
        let high_count = matches
            .iter()
            .filter(|m| m.severity == Severity::High)
            .count();
        let medium_count = matches
            .iter()
            .filter(|m| m.severity == Severity::Medium)
            .count();
        let low_count = matches
            .iter()
            .filter(|m| m.severity == Severity::Low)
            .count();
        Self {
            matches,
            high_count,
            medium_count,
            low_count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
    pub fn total(&self) -> usize {
        self.matches.len()
    }

    pub fn format_warning(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut msg = format!(
            "\n\x1b[33m⚠️  Found {} potential secret(s):\x1b[0m\n",
            self.total()
        );

        if self.high_count > 0 {
            msg.push_str(&format!("   \x1b[31m● {} HIGH\x1b[0m\n", self.high_count));
        }
        if self.medium_count > 0 {
            msg.push_str(&format!(
                "   \x1b[33m● {} MEDIUM\x1b[0m\n",
                self.medium_count
            ));
        }
        if self.low_count > 0 {
            msg.push_str(&format!("   \x1b[90m● {} LOW\x1b[0m\n", self.low_count));
        }

        msg.push('\n');
        for (i, m) in self.matches.iter().take(5).enumerate() {
            let color = match m.severity {
                Severity::High => "\x1b[31m",
                Severity::Medium => "\x1b[33m",
                Severity::Low => "\x1b[90m",
            };
            msg.push_str(&format!(
                "   {}. [{}{}:L{}\x1b[0m] {}\n",
                i + 1,
                color,
                m.pattern_name,
                m.line_number,
                m.masked_preview().chars().take(70).collect::<String>()
            ));
        }

        if self.matches.len() > 5 {
            msg.push_str(&format!("   ... and {} more\n", self.matches.len() - 5));
        }

        msg
    }
}

// =============================================================================
// PATTERNS
// =============================================================================

struct CompiledPatterns {
    patterns: Vec<(Regex, &'static str, Severity)>,
}

impl CompiledPatterns {
    fn new() -> Self {
        let patterns = vec![
            // AI APIs
            // Anthropic: sk-ant- prefix (check this BEFORE generic patterns)
            (Regex::new(r"sk-ant-(?:api\d+-)?[A-Za-z0-9_-]{20,}").unwrap(), "Anthropic Key", Severity::High),
            // OpenAI: sk-proj- prefix (modern project keys)
            (Regex::new(r"sk-proj-[A-Za-z0-9_-]{20,}").unwrap(), "OpenAI Key", Severity::High),
            (Regex::new(r"AIza[A-Za-z0-9_-]{35}").unwrap(), "Google API Key", Severity::High),

            // Version Control
            (Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}").unwrap(), "GitHub Token", Severity::High),
            (Regex::new(r"ghu_[A-Za-z0-9_]{36,}").unwrap(), "GitHub App Token", Severity::High),
            (Regex::new(r"glpat-[A-Za-z0-9_-]{20,}").unwrap(), "GitLab Token", Severity::High),

            // Cloud
            (Regex::new(r"(?:A3T[A-Z0-9]|AKIA|ABIA|ACCA|ASIA)[A-Z0-9]{16}").unwrap(), "AWS Key ID", Severity::High),
            (Regex::new(r"(?i)aws_secret(?:_access)?_key\s*[=:]\s*[A-Za-z0-9/+=]{40}").unwrap(), "AWS Secret", Severity::High),

            // Payment
            (Regex::new(r"(?:sk|pk)_(?:live|test)_[A-Za-z0-9]{24,}").unwrap(), "Stripe Key", Severity::High),

            // Communication
            (Regex::new(r"xox[bpars]-[A-Za-z0-9-]{10,}").unwrap(), "Slack Token", Severity::High),
            (Regex::new(r"[MN][A-Za-z0-9_-]{23,}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,}").unwrap(), "Discord Token", Severity::High),
            (Regex::new(r"SK[a-f0-9]{32}").unwrap(), "Twilio Key", Severity::High),

            // Infrastructure
            (Regex::new(r"SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}").unwrap(), "SendGrid Key", Severity::High),
            (Regex::new(r"key-[A-Za-z0-9]{32}").unwrap(), "Mailgun Key", Severity::High),
            (Regex::new(r"npm_[A-Za-z0-9]{36}").unwrap(), "npm Token", Severity::High),
            (Regex::new(r"pypi-[A-Za-z0-9_-]{50,}").unwrap(), "PyPI Token", Severity::High),

            // Keys & Certs
            (Regex::new(r"-----BEGIN\s+(?:RSA\s+|DSA\s+|EC\s+|OPENSSH\s+|PGP\s+)?PRIVATE\s+KEY").unwrap(), "Private Key", Severity::High),

            // Database
            (Regex::new(r"(?i)(?:mongodb(?:\+srv)?|postgres(?:ql)?|mysql|redis|amqp|mssql)://\S+").unwrap(), "DB Connection", Severity::High),

            // Generic (medium severity)
            (Regex::new(r"(?i)(?:password|passwd|pwd)\s*[=:]\s*\S{8,}").unwrap(), "Password", Severity::Medium),
            (Regex::new(r"(?i)(?:secret|token|api_key|apikey|auth_token|access_token)\s*[=:]\s*[A-Za-z0-9_/+=.-]{16,}").unwrap(), "Secret/Token", Severity::Medium),
            (Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*").unwrap(), "JWT", Severity::Medium),
            (Regex::new(r"(?i)bearer\s+[A-Za-z0-9_.-]{20,}").unwrap(), "Bearer Token", Severity::Medium),
        ];
        Self { patterns }
    }
}

static PATTERNS: LazyLock<CompiledPatterns> = LazyLock::new(CompiledPatterns::new);

// =============================================================================
// SCANNING
// =============================================================================

pub fn scan(text: &str) -> Vec<SecretMatch> {
    let mut matches = Vec::new();
    let patterns = &PATTERNS.patterns;

    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("@@") || trimmed.starts_with("diff --git") {
            continue;
        }

        for (regex, name, severity) in patterns {
            for m in regex.find_iter(line) {
                matches.push(SecretMatch {
                    pattern_name: name.to_string(),
                    severity: *severity,
                    line_number: line_num + 1,
                    line_preview: line.to_string(),
                    match_start: m.start(),
                    match_end: m.end(),
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.line_number.cmp(&b.line_number))
    });
    matches
}

pub fn has_secrets(text: &str) -> bool {
    let patterns = &PATTERNS.patterns;

    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("@@") || t.starts_with("diff --git") {
            continue;
        }
        for (regex, _, _) in patterns {
            if regex.is_match(line) {
                return true;
            }
        }
    }
    false
}

pub fn redact(text: &str) -> Cow<'_, str> {
    if !has_secrets(text) {
        return Cow::Borrowed(text);
    }

    let patterns = &PATTERNS.patterns;
    let mut result = text.to_string();

    for (regex, name, _) in patterns {
        result = regex
            .replace_all(&result, |caps: &regex::Captures| {
                let m = caps.get(0).map(|x| x.as_str()).unwrap_or("");
                format!("[REDACTED:{}:{}ch]", short_name(name), m.len())
            })
            .to_string();
    }
    Cow::Owned(result)
}

/// Process secrets based on action
pub fn process_secrets(text: &str, action: SecretAction) -> Result<Cow<'_, str>, SecretScanResult> {
    let matches = scan(text);
    if matches.is_empty() {
        return Ok(Cow::Borrowed(text));
    }

    let result = SecretScanResult::from_matches(matches);

    match action {
        SecretAction::Block => Err(result),
        SecretAction::Warn => {
            eprintln!("{}", result.format_warning());
            Ok(Cow::Borrowed(text))
        }
        SecretAction::Redact => {
            eprintln!("{}", result.format_warning());
            eprintln!("\x1b[32m✓ Secrets redacted before sending to LLM\x1b[0m\n");
            Ok(redact(text))
        }
    }
}

// =============================================================================
// HELPERS
// =============================================================================

fn mask(s: &str) -> String {
    let len = s.len();
    if len <= 8 {
        return "[REDACTED]".to_string();
    }
    let show = 4.min(len / 4);
    format!(
        "{}...{}[REDACTED]",
        &s[..show],
        &s[len.saturating_sub(show)..]
    )
}

fn short_name(name: &str) -> &'static str {
    match name {
        "OpenAI Key" => "OPENAI",
        "Anthropic Key" => "ANTHROPIC",
        "GitHub Token" | "GitHub App Token" => "GITHUB",
        "GitLab Token" => "GITLAB",
        "AWS Key ID" => "AWS_ID",
        "AWS Secret" => "AWS_SECRET",
        "Stripe Key" => "STRIPE",
        "Slack Token" => "SLACK",
        "Discord Token" => "DISCORD",
        "Google API Key" => "GOOGLE",
        "Private Key" => "PRIVATE_KEY",
        "Password" => "PASSWORD",
        "Secret/Token" => "SECRET",
        "DB Connection" => "DB_URL",
        "JWT" => "JWT",
        "Bearer Token" => "BEARER",
        _ => "SECRET",
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // OpenAI detection tests
    // ==========================================================================

    #[test]
    fn detect_openai_proj_key() {
        let m = scan("OPENAI_API_KEY=sk-proj-abc123def456ghi789jkl012");
        assert!(!m.is_empty());
        assert_eq!(m[0].pattern_name, "OpenAI Key");
    }

    // ==========================================================================
    // Anthropic detection tests
    // ==========================================================================

    #[test]
    fn detect_anthropic_new_format() {
        let m = scan("key=sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789");
        assert!(!m.is_empty(), "Should detect new Anthropic key format");
        assert_eq!(m[0].pattern_name, "Anthropic Key");
    }

    #[test]
    fn detect_anthropic_old_format() {
        let m = scan("key=sk-ant-abcdefghijklmnopqrstuvwxyz");
        assert!(!m.is_empty(), "Should detect old Anthropic key format");
        assert_eq!(m[0].pattern_name, "Anthropic Key");
    }

    // ==========================================================================
    // Other provider detection tests
    // ==========================================================================

    #[test]
    fn detect_github() {
        let m = scan("token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx1234");
        assert!(!m.is_empty());
        assert_eq!(m[0].pattern_name, "GitHub Token");
    }

    #[test]
    fn detect_aws() {
        let m = scan("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE");
        assert!(!m.is_empty());
        assert_eq!(m[0].pattern_name, "AWS Key ID");
    }

    #[test]
    fn detect_private_key() {
        let m = scan("-----BEGIN RSA PRIVATE KEY-----");
        assert!(!m.is_empty());
        assert_eq!(m[0].pattern_name, "Private Key");
    }

    #[test]
    fn detect_db_url() {
        let m = scan("DATABASE_URL=postgres://user:pass@localhost/db");
        assert!(!m.is_empty());
        assert_eq!(m[0].pattern_name, "DB Connection");
    }

    // ==========================================================================
    // Redaction tests
    // ==========================================================================

    #[test]
    fn redacts_secrets() {
        let r = redact("key=sk-proj-abc123def456ghi789jkl012mno345");
        assert!(r.contains("[REDACTED"));
        assert!(!r.contains("sk-proj-"));
    }

    #[test]
    fn redact_preserves_diff_structure() {
        let diff = "diff --git a/config.rs b/config.rs\n+API_KEY=sk-proj-abc123def456ghi789jkl012mno345\n unchanged line";
        let redacted = redact(diff);
        assert!(redacted.starts_with("diff --git"));
        assert!(redacted.contains("[REDACTED:OPENAI:"));
        assert!(redacted.contains("unchanged line"));
    }

    // ==========================================================================
    // False positive tests
    // ==========================================================================

    #[test]
    fn no_false_positive_on_normal_code() {
        let m = scan("fn main() { let x = 42; }");
        assert!(m.is_empty());
    }

    #[test]
    fn skips_diff_headers() {
        let diff = "diff --git a/sk-proj-secret.rs b/sk-proj-secret.rs\n@@ -1,3 +1,4 @@ sk-proj-test\n+real secret sk-proj-abc123def456ghi789jkl012";
        let m = scan(diff);
        // Should only detect the secret in the actual diff line, not headers
        assert_eq!(m.len(), 1);
        assert!(m[0].line_preview.starts_with("+real"));
    }

    // ==========================================================================
    // SecretAction tests
    // ==========================================================================

    #[test]
    fn action_from_str() {
        assert_eq!(SecretAction::from_str("redact"), Some(SecretAction::Redact));
        assert_eq!(SecretAction::from_str("WARN"), Some(SecretAction::Warn));
        assert_eq!(SecretAction::from_str("Block"), Some(SecretAction::Block));
        assert_eq!(SecretAction::from_str("invalid"), None);
    }

    #[test]
    fn process_block_returns_error() {
        let r = process_secrets(
            "key=sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123",
            SecretAction::Block,
        );
        assert!(r.is_err());
        let scan_result = r.unwrap_err();
        assert!(scan_result.total() > 0);
    }

    #[test]
    fn process_clean_returns_ok() {
        let r = process_secrets("let x = 42;", SecretAction::Redact);
        assert!(r.is_ok());
    }

    // ==========================================================================
    // SecretScanResult tests
    // ==========================================================================

    #[test]
    fn scan_result_counts_severity() {
        let matches = vec![
            SecretMatch {
                pattern_name: "Test".into(),
                severity: Severity::High,
                line_number: 1,
                line_preview: "test".into(),
                match_start: 0,
                match_end: 4,
            },
            SecretMatch {
                pattern_name: "Test2".into(),
                severity: Severity::Medium,
                line_number: 2,
                line_preview: "test2".into(),
                match_start: 0,
                match_end: 5,
            },
        ];
        let result = SecretScanResult::from_matches(matches);
        assert_eq!(result.high_count, 1);
        assert_eq!(result.medium_count, 1);
        assert_eq!(result.total(), 2);
    }
}
