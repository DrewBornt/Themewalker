//! Theme discovery: scans /usr/share/sddm/themes/ for installed SDDM themes
//! and reads per-theme metadata from metadata.desktop files.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub const THEMES_DIR: &str = "/usr/share/sddm/themes";

/// A discovered SDDM theme.
#[derive(Debug, Clone)]
pub struct SddmTheme {
    /// Directory name – this is the identifier SDDM uses in its config.
    pub name: String,
    /// Full path to the theme directory (available for callers that need it).
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Human-readable description from metadata.desktop (if present).
    pub description: Option<String>,
    /// Author field from metadata.desktop (if present).
    pub author: Option<String>,
}

impl SddmTheme {
    /// Try to build an `SddmTheme` from a directory path.
    /// Returns `None` when the path is not a directory or has no valid name.
    pub fn from_dir(path: PathBuf) -> Option<Self> {
        if !path.is_dir() {
            return None;
        }
        let name = path.file_name()?.to_string_lossy().into_owned();
        let (description, author) = parse_metadata(&path.join("metadata.desktop"));
        Some(Self { name, path, description, author })
    }

    /// One-line summary for display: "name — description" when a description exists.
    pub fn display_label(&self) -> String {
        match &self.description {
            Some(d) if !d.is_empty() => format!("{} — {}", self.name, d),
            _ => self.name.clone(),
        }
    }
}

/// Parse `Description=` and `Author=` from a `.desktop` file.
fn parse_metadata(path: &Path) -> (Option<String>, Option<String>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };
    let mut description = None;
    let mut author = None;
    for line in content.lines() {
        let t = line.trim();
        if description.is_none() {
            if let Some(v) = t.strip_prefix("Description=") {
                description = Some(v.to_string());
            }
        }
        if author.is_none() {
            if let Some(v) = t.strip_prefix("Author=") {
                author = Some(v.to_string());
            }
        }
        if description.is_some() && author.is_some() {
            break;
        }
    }
    (description, author)
}

/// Scan `THEMES_DIR` and return all installed themes, sorted alphabetically.
pub fn discover_themes() -> Result<Vec<SddmTheme>> {
    let dir = Path::new(THEMES_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut themes: Vec<SddmTheme> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter_map(SddmTheme::from_dir)
        .collect();

    themes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(themes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_file(path: &Path, content: &str) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_parse_metadata_reads_fields() {
        let dir = tempfile::tempdir().unwrap();
        let meta = dir.path().join("metadata.desktop");
        write_file(
            &meta,
            "[SddmGreeterTheme]\nName=Foo\nDescription=A test theme\nAuthor=Tester\n",
        );
        let (desc, auth) = parse_metadata(&meta);
        assert_eq!(desc.as_deref(), Some("A test theme"));
        assert_eq!(auth.as_deref(), Some("Tester"));
    }

    #[test]
    fn test_parse_metadata_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let (desc, auth) = parse_metadata(&dir.path().join("nonexistent.desktop"));
        assert!(desc.is_none());
        assert!(auth.is_none());
    }

    #[test]
    fn test_from_dir_skips_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notadir");
        write_file(&file, "content");
        assert!(SddmTheme::from_dir(file).is_none());
    }

    #[test]
    fn test_display_label_with_description() {
        let theme = SddmTheme {
            name: "breeze".to_string(),
            path: PathBuf::from("/tmp"),
            description: Some("KDE Breeze".to_string()),
            author: None,
        };
        assert_eq!(theme.display_label(), "breeze — KDE Breeze");
    }

    #[test]
    fn test_display_label_without_description() {
        let theme = SddmTheme {
            name: "breeze".to_string(),
            path: PathBuf::from("/tmp"),
            description: None,
            author: None,
        };
        assert_eq!(theme.display_label(), "breeze");
    }
}
