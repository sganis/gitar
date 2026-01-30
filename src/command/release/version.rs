// src/command/release/version.rs - Version file detection and updates

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

use crate::color::success;
use crate::config::{Config, ReleaseConfig, VersionFileConfig};

// =============================================================================
// VERSION FILE DETECTION
// =============================================================================

#[derive(Debug, Clone)]
pub struct VersionFile {
    pub path: PathBuf,
    pub file_type: VersionFileType,
    pub current_version: String,
}

#[derive(Debug, Clone)]
pub enum VersionFileType {
    Cargo,       // Cargo.toml
    PackageJson, // package.json
    PyProject,   // pyproject.toml
    /// Custom JSON file with dot-notation path
    Json { json_path: String },
    /// Custom file with regex pattern
    Pattern {
        pattern: String,
        template: String,
    },
}

impl VersionFileType {
    #[allow(dead_code)]
    fn name(&self) -> &str {
        match self {
            Self::Cargo => "Cargo.toml",
            Self::PackageJson => "package.json",
            Self::PyProject => "pyproject.toml",
            Self::Json { .. } => "JSON",
            Self::Pattern { .. } => "Custom",
        }
    }
}

/// Detect version files - checks config first, then falls back to auto-detection
pub fn detect_version_files(root: &Path) -> Result<Vec<VersionFile>> {
    // Load config to check for custom version file settings
    let config = Config::load();

    if let Some(ref release_config) = config.release {
        let custom_files = detect_custom_version_files(root, release_config)?;
        if !custom_files.is_empty() {
            return Ok(custom_files);
        }
    }

    // Fall back to auto-detection
    detect_standard_version_files(root)
}

/// Detect custom version files from config
fn detect_custom_version_files(root: &Path, config: &ReleaseConfig) -> Result<Vec<VersionFile>> {
    let mut files = Vec::new();

    // Check for multiple version files config
    if let Some(ref version_files) = config.version_files {
        for vf in version_files {
            if let Some(file) = detect_single_custom_file(root, vf)? {
                files.push(file);
            }
        }
        return Ok(files);
    }

    // Check for single version file config
    if let Some(ref version_file) = config.version_file {
        let vf_config = VersionFileConfig {
            file: version_file.clone(),
            json_path: config.version_json_path.clone(),
            pattern: config.version_pattern.clone(),
            template: config.version_template.clone(),
        };
        if let Some(file) = detect_single_custom_file(root, &vf_config)? {
            files.push(file);
        }
    }

    Ok(files)
}

/// Detect a single custom version file
fn detect_single_custom_file(root: &Path, config: &VersionFileConfig) -> Result<Option<VersionFile>> {
    let path = root.join(&config.file);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // Determine file type and extract version
    let (file_type, version) = if let Some(ref json_path) = config.json_path {
        // JSON file with path
        let version = extract_json_version(&content, json_path)?;
        (VersionFileType::Json { json_path: json_path.clone() }, version)
    } else if let Some(ref pattern) = config.pattern {
        // Custom pattern
        let template = config.template.clone()
            .unwrap_or_else(|| pattern.replace(r"(\d+\.\d+\.\d+)", "{version}"));
        let version = extract_pattern_version(&content, pattern)?;
        (VersionFileType::Pattern { pattern: pattern.clone(), template }, version)
    } else {
        // Try to auto-detect based on file extension
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "json" => {
                // Default to "version" path for JSON files
                let json_path = "version".to_string();
                let version = extract_json_version(&content, &json_path)?;
                (VersionFileType::Json { json_path }, version)
            }
            "toml" => {
                // Try standard TOML version extraction
                if let Some(version) = extract_toml_version(&content) {
                    (VersionFileType::Cargo, version) // Reuse Cargo type for TOML
                } else {
                    return Ok(None);
                }
            }
            _ => {
                // Try common patterns
                if let Some(version) = try_common_patterns(&content) {
                    let pattern = r#"version\s*=\s*"(\d+\.\d+\.\d+)""#.to_string();
                    let template = r#"version = "{version}""#.to_string();
                    (VersionFileType::Pattern { pattern, template }, version)
                } else {
                    return Ok(None);
                }
            }
        }
    };

    Ok(Some(VersionFile {
        path,
        file_type,
        current_version: version,
    }))
}

/// Detect standard version files (auto-detection)
fn detect_standard_version_files(root: &Path) -> Result<Vec<VersionFile>> {
    let mut files = Vec::new();

    // Check for Cargo.toml
    if let Some(cargo_version) = detect_cargo_version(root)? {
        files.push(VersionFile {
            path: root.join("Cargo.toml"),
            file_type: VersionFileType::Cargo,
            current_version: cargo_version,
        });
    }

    // Check for package.json
    if let Some(pkg_version) = detect_package_json_version(root)? {
        files.push(VersionFile {
            path: root.join("package.json"),
            file_type: VersionFileType::PackageJson,
            current_version: pkg_version,
        });
    }

    // Check for pyproject.toml
    if let Some(py_version) = detect_pyproject_version(root)? {
        files.push(VersionFile {
            path: root.join("pyproject.toml"),
            file_type: VersionFileType::PyProject,
            current_version: py_version,
        });
    }

    Ok(files)
}

// =============================================================================
// VERSION EXTRACTION
// =============================================================================

/// Extract version from JSON content using dot-notation path
fn extract_json_version(content: &str, json_path: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(content)
        .context("Failed to parse JSON")?;

    let parts: Vec<&str> = json_path.split('.').collect();
    let mut current = &value;

    for part in parts {
        current = current.get(part)
            .ok_or_else(|| anyhow::anyhow!("JSON path '{}' not found at '{}'", json_path, part))?;
    }

    current.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Version at path '{}' is not a string", json_path))
}

/// Extract version using regex pattern
fn extract_pattern_version(content: &str, pattern: &str) -> Result<String> {
    let re = Regex::new(pattern)
        .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

    let caps = re.captures(content)
        .ok_or_else(|| anyhow::anyhow!("Pattern '{}' not found in file", pattern))?;

    caps.get(1)
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("Pattern '{}' has no capture group", pattern))
}

/// Extract version from TOML content (generic)
fn extract_toml_version(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") && trimmed.contains('=') {
            if let Some(version) = extract_quoted_value(trimmed) {
                return Some(version);
            }
        }
    }
    None
}

/// Try common version patterns
fn try_common_patterns(content: &str) -> Option<String> {
    // Try version = "x.y.z"
    let toml_re = Regex::new(r#"version\s*=\s*"(\d+\.\d+\.\d+)""#).ok()?;
    if let Some(caps) = toml_re.captures(content) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // Try "version": "x.y.z"
    let json_re = Regex::new(r#""version"\s*:\s*"(\d+\.\d+\.\d+)""#).ok()?;
    if let Some(caps) = json_re.captures(content) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // Try VERSION = x.y.z
    let const_re = Regex::new(r#"VERSION\s*=\s*['"]*(\d+\.\d+\.\d+)['"]*"#).ok()?;
    if let Some(caps) = const_re.captures(content) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}

fn detect_cargo_version(root: &Path) -> Result<Option<String>> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    Ok(extract_toml_version(&content))
}

fn detect_package_json_version(root: &Path) -> Result<Option<String>> {
    let path = root.join("package.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"version\"") {
            if let Some(version) = extract_quoted_value(trimmed) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

fn detect_pyproject_version(root: &Path) -> Result<Option<String>> {
    let path = root.join("pyproject.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    let mut in_project_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            in_project_section = trimmed.contains("[project]") || trimmed.contains("[tool.poetry]");
        }

        if in_project_section && trimmed.starts_with("version") {
            if let Some(version) = extract_quoted_value(trimmed) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

fn extract_quoted_value(line: &str) -> Option<String> {
    // Extract value between quotes: version = "1.2.3" or "version": "1.2.3"
    if let Some(eq_pos) = line.find('=').or_else(|| line.find(':')) {
        let after_eq = &line[eq_pos + 1..];
        if let Some(start) = after_eq.find('"') {
            if let Some(end) = after_eq[start + 1..].find('"') {
                return Some(after_eq[start + 1..start + 1 + end].to_string());
            }
        }
    }
    None
}

// =============================================================================
// VERSION UPDATES
// =============================================================================

/// Update version in a file
pub fn update_version_file(file: &VersionFile, new_version: &str, dry_run: bool) -> Result<()> {
    let content = fs::read_to_string(&file.path)
        .context(format!("Failed to read {}", file.path.display()))?;

    let updated = match &file.file_type {
        VersionFileType::Cargo => {
            update_cargo_version(&content, &file.current_version, new_version)?
        }
        VersionFileType::PackageJson => {
            update_package_json_version(&content, &file.current_version, new_version)?
        }
        VersionFileType::PyProject => {
            update_pyproject_version(&content, &file.current_version, new_version)?
        }
        VersionFileType::Json { json_path } => {
            update_json_version(&content, json_path, new_version)?
        }
        VersionFileType::Pattern { pattern, template } => {
            update_pattern_version(&content, pattern, template, new_version)?
        }
    };

    if dry_run {
        println!(
            "Would update {} from {} to {}",
            file.path.display(),
            file.current_version,
            new_version
        );
    } else {
        fs::write(&file.path, updated)
            .context(format!("Failed to write {}", file.path.display()))?;
        success(format!(
            "Updated {} from {} to {}",
            file.path.display(),
            file.current_version,
            new_version
        ));
    }

    Ok(())
}

fn update_cargo_version(content: &str, old_version: &str, new_version: &str) -> Result<String> {
    let old_line = format!("version = \"{}\"", old_version);
    let new_line = format!("version = \"{}\"", new_version);

    if !content.contains(&old_line) {
        bail!("Could not find version line in Cargo.toml");
    }

    Ok(content.replace(&old_line, &new_line))
}

fn update_package_json_version(
    content: &str,
    old_version: &str,
    new_version: &str,
) -> Result<String> {
    let old_line = format!("\"version\": \"{}\"", old_version);
    let new_line = format!("\"version\": \"{}\"", new_version);

    if !content.contains(&old_line) {
        bail!("Could not find version line in package.json");
    }

    Ok(content.replace(&old_line, &new_line))
}

fn update_pyproject_version(content: &str, old_version: &str, new_version: &str) -> Result<String> {
    let old_line = format!("version = \"{}\"", old_version);
    let new_line = format!("version = \"{}\"", new_version);

    if !content.contains(&old_line) {
        bail!("Could not find version line in pyproject.toml");
    }

    Ok(content.replace(&old_line, &new_line))
}

/// Update version in JSON file using json_path
fn update_json_version(content: &str, json_path: &str, new_version: &str) -> Result<String> {
    let mut value: serde_json::Value = serde_json::from_str(content)
        .context("Failed to parse JSON")?;

    let parts: Vec<&str> = json_path.split('.').collect();
    let mut current = &mut value;

    // Navigate to parent of the target field
    for part in &parts[..parts.len() - 1] {
        current = current.get_mut(*part)
            .ok_or_else(|| anyhow::anyhow!("JSON path '{}' not found", json_path))?;
    }

    // Update the target field
    let last_part = parts.last()
        .ok_or_else(|| anyhow::anyhow!("Empty JSON path"))?;

    if let Some(obj) = current.as_object_mut() {
        obj.insert(last_part.to_string(), serde_json::Value::String(new_version.to_string()));
    } else {
        bail!("Cannot update non-object JSON value");
    }

    // Serialize back with pretty printing
    serde_json::to_string_pretty(&value)
        .context("Failed to serialize JSON")
}

/// Update version using regex pattern and template
fn update_pattern_version(
    content: &str,
    pattern: &str,
    template: &str,
    new_version: &str,
) -> Result<String> {
    let re = Regex::new(pattern)
        .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

    if !re.is_match(content) {
        bail!("Pattern '{}' not found in file", pattern);
    }

    let replacement = template.replace("{version}", new_version);
    Ok(re.replace(content, replacement.as_str()).to_string())
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_cargo_version() {
        let line = "version = \"1.2.3\"";
        assert_eq!(extract_quoted_value(line), Some("1.2.3".to_string()));
    }

    #[test]
    fn extract_package_json_version() {
        let line = "  \"version\": \"2.0.0\",";
        assert_eq!(extract_quoted_value(line), Some("2.0.0".to_string()));
    }

    #[test]
    fn update_cargo_content() {
        let content = "[package]\nname = \"test\"\nversion = \"1.0.0\"\n";
        let result = update_cargo_version(content, "1.0.0", "1.1.0").unwrap();
        assert!(result.contains("version = \"1.1.0\""));
    }

    #[test]
    fn extract_json_version_simple() {
        let content = r#"{"version": "1.2.3", "name": "test"}"#;
        let result = extract_json_version(content, "version").unwrap();
        assert_eq!(result, "1.2.3");
    }

    #[test]
    fn extract_json_version_nested() {
        let content = r#"{"package": {"version": "2.0.0"}, "name": "test"}"#;
        let result = extract_json_version(content, "package.version").unwrap();
        assert_eq!(result, "2.0.0");
    }

    #[test]
    fn extract_pattern_version_toml() {
        let content = r#"[package]\nversion = "3.1.4""#;
        let pattern = r#"version\s*=\s*"(\d+\.\d+\.\d+)""#;
        let result = extract_pattern_version(content, pattern).unwrap();
        assert_eq!(result, "3.1.4");
    }

    #[test]
    fn update_json_version_simple() {
        let content = r#"{"version": "1.0.0", "name": "test"}"#;
        let result = update_json_version(content, "version", "2.0.0").unwrap();
        assert!(result.contains("\"version\": \"2.0.0\""));
    }

    #[test]
    fn update_pattern_version_works() {
        let content = "VERSION = \"1.0.0\"";
        let pattern = r#"VERSION = "(\d+\.\d+\.\d+)""#;
        let template = "VERSION = \"{version}\"";
        let result = update_pattern_version(content, pattern, template, "2.0.0").unwrap();
        assert_eq!(result, "VERSION = \"2.0.0\"");
    }

    #[test]
    fn try_common_patterns_toml() {
        let content = "version = \"1.2.3\"";
        let result = try_common_patterns(content);
        assert_eq!(result, Some("1.2.3".to_string()));
    }

    #[test]
    fn try_common_patterns_json() {
        let content = "\"version\": \"4.5.6\"";
        let result = try_common_patterns(content);
        assert_eq!(result, Some("4.5.6".to_string()));
    }
}
