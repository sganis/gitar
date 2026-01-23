// src/command/release/version.rs - Version file detection and updates

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::color::success;

// =============================================================================
// VERSION FILE DETECTION
// =============================================================================

#[derive(Debug, Clone)]
pub struct VersionFile {
    pub path: PathBuf,
    pub file_type: VersionFileType,
    pub current_version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VersionFileType {
    Cargo,      // Cargo.toml
    PackageJson, // package.json
    PyProject,   // pyproject.toml
}

impl VersionFileType {
    #[allow(dead_code)]
    fn name(&self) -> &str {
        match self {
            Self::Cargo => "Cargo.toml",
            Self::PackageJson => "package.json",
            Self::PyProject => "pyproject.toml",
        }
    }
}

/// Detect version files in the current directory
pub fn detect_version_files() -> Result<Vec<VersionFile>> {
    let mut files = Vec::new();

    // Check for Cargo.toml
    if let Some(cargo_version) = detect_cargo_version()? {
        files.push(VersionFile {
            path: PathBuf::from("Cargo.toml"),
            file_type: VersionFileType::Cargo,
            current_version: cargo_version,
        });
    }

    // Check for package.json
    if let Some(pkg_version) = detect_package_json_version()? {
        files.push(VersionFile {
            path: PathBuf::from("package.json"),
            file_type: VersionFileType::PackageJson,
            current_version: pkg_version,
        });
    }

    // Check for pyproject.toml
    if let Some(py_version) = detect_pyproject_version()? {
        files.push(VersionFile {
            path: PathBuf::from("pyproject.toml"),
            file_type: VersionFileType::PyProject,
            current_version: py_version,
        });
    }

    Ok(files)
}

// =============================================================================
// VERSION EXTRACTION
// =============================================================================

fn detect_cargo_version() -> Result<Option<String>> {
    let path = Path::new("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") && trimmed.contains('=') {
            if let Some(version) = extract_quoted_value(trimmed) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

fn detect_package_json_version() -> Result<Option<String>> {
    let path = Path::new("package.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
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

fn detect_pyproject_version() -> Result<Option<String>> {
    let path = Path::new("pyproject.toml");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
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

    let updated = match file.file_type {
        VersionFileType::Cargo => update_cargo_version(&content, &file.current_version, new_version)?,
        VersionFileType::PackageJson => update_package_json_version(&content, &file.current_version, new_version)?,
        VersionFileType::PyProject => update_pyproject_version(&content, &file.current_version, new_version)?,
    };

    if dry_run {
        println!("Would update {} from {} to {}", file.path.display(), file.current_version, new_version);
    } else {
        fs::write(&file.path, updated)
            .context(format!("Failed to write {}", file.path.display()))?;
        success(format!("Updated {} from {} to {}", file.path.display(), file.current_version, new_version));
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

fn update_package_json_version(content: &str, old_version: &str, new_version: &str) -> Result<String> {
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
}
