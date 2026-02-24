//! SDDM configuration: reading the current theme from the INI-style config
//! file and writing a new selection back (with sudo escalation when the file
//! is not writable by the current user).
//!
//! SDDM config locations checked, in order:
//!   1. /etc/sddm.conf          (legacy single-file)
//!   2. /etc/sddm.conf.d/*.conf (drop-in directory, modern)
//!
//! The theme identifier lives in the [Theme] section under the key `Current`.

use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

const SDDM_CONF: &str = "/etc/sddm.conf";
const SDDM_CONF_D: &str = "/etc/sddm.conf.d";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Loaded SDDM configuration, ready for reading and writing.
pub struct SddmConfig {
    /// Path we will write changes to.
    pub path: PathBuf,
    /// The `Current=` value found in `[Theme]`, if any.
    pub current_theme: Option<String>,
    /// Raw file content (may be empty for a brand-new file).
    raw_content: String,
}

impl SddmConfig {
    /// Load config from disk.  Succeeds even when the config file does not
    /// exist yet (returns an empty config targeting `/etc/sddm.conf`).
    pub fn load() -> Result<Self> {
        let path = resolve_config_path();

        let raw_content = if path.exists() {
            fs::read_to_string(&path)
                .with_context(|| format!("Failed to read SDDM config at {}", path.display()))?
        } else {
            String::new()
        };

        let current_theme = parse_current_theme(&raw_content);

        Ok(Self { path, current_theme, raw_content })
    }

    /// Return a minimal in-memory config (no disk I/O), used as a fallback.
    pub fn empty() -> Self {
        Self {
            path: PathBuf::from(SDDM_CONF),
            current_theme: None,
            raw_content: String::new(),
        }
    }

    /// Patch the `Current=` key in `[Theme]` and write the file back.
    /// Tries a direct write first; falls back to `sudo tee` on EPERM/EACCES.
    pub fn write_theme(&self, theme_name: &str) -> Result<()> {
        let new_content = apply_theme_to_content(&self.raw_content, theme_name);
        write_to_path(&self.path, &new_content)
    }
}

// ---------------------------------------------------------------------------
// Config file resolution
// ---------------------------------------------------------------------------

/// Walk the known locations and return the path that contains [Theme]/Current=,
/// or the best default path to create.
fn resolve_config_path() -> PathBuf {
    // Prefer an existing file that already holds [Theme]
    let main = Path::new(SDDM_CONF);
    if main.exists() {
        if let Ok(c) = fs::read_to_string(main) {
            if has_theme_section(&c) {
                return main.to_path_buf();
            }
        }
    }

    // Check drop-in directory for any file that has [Theme] / Current=
    let conf_d = Path::new(SDDM_CONF_D);
    if conf_d.is_dir() {
        if let Ok(entries) = fs::read_dir(conf_d) {
            let mut candidates: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |x| x == "conf"))
                .collect();
            candidates.sort(); // deterministic order
            for path in candidates {
                if let Ok(c) = fs::read_to_string(&path) {
                    if has_theme_section(&c) {
                        return path;
                    }
                }
            }
        }
        // No existing file with [Theme] – create a new drop-in
        return conf_d.join("theme.conf");
    }

    // Fall back to the legacy path (may not exist yet)
    main.to_path_buf()
}

fn has_theme_section(content: &str) -> bool {
    let mut in_theme = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_theme = t == "[Theme]";
        } else if in_theme && t.starts_with("Current=") {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// INI parsing
// ---------------------------------------------------------------------------

/// Extract the value of `Current=` from the `[Theme]` section.
pub fn parse_current_theme(content: &str) -> Option<String> {
    let mut in_theme = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_theme = t == "[Theme]";
            continue;
        }
        if in_theme {
            if let Some(val) = t.strip_prefix("Current=") {
                let v = val.trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// INI writing
// ---------------------------------------------------------------------------

/// Return a new copy of `content` with `Current=<theme_name>` set inside
/// `[Theme]`.  Handles four cases:
///   A. `[Theme]` + `Current=` exist  → replace the value in-place.
///   B. `[Theme]` exists but no `Current=` → insert before next section.
///   C. No `[Theme]` at all            → append `[Theme]\nCurrent=…` at EOF.
pub fn apply_theme_to_content(content: &str, theme_name: &str) -> String {
    let new_line = format!("Current={}", theme_name);

    let mut result = String::with_capacity(content.len() + 64);
    let mut in_theme = false;
    let mut found_section = false;
    let mut found_key = false;

    for line in content.lines() {
        let t = line.trim();

        if t.starts_with('[') {
            // Leaving a [Theme] section that had no Current= yet → inject key
            if in_theme && !found_key {
                result.push_str(&new_line);
                result.push('\n');
                found_key = true;
            }
            in_theme = t == "[Theme]";
            if in_theme {
                found_section = true;
            }
            result.push_str(line);
            result.push('\n');
        } else if in_theme && t.starts_with("Current=") {
            result.push_str(&new_line);
            result.push('\n');
            found_key = true;
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    // End-of-file: still inside [Theme] with no Current= written yet
    if in_theme && !found_key {
        result.push_str(&new_line);
        result.push('\n');
        found_key = true;
    }

    // [Theme] section was never found at all → append it
    if !found_section || !found_key {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str("\n[Theme]\n");
        result.push_str(&new_line);
        result.push('\n');
    }

    result
}

// ---------------------------------------------------------------------------
// Writing (direct or via sudo)
// ---------------------------------------------------------------------------

fn write_to_path(path: &Path, content: &str) -> Result<()> {
    // Ensure parent directory exists (e.g. /etc/sddm.conf.d/)
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            // Try to create it with sudo if we can't do it directly
            if fs::create_dir_all(parent).is_err() {
                sudo_mkdir(parent)?;
            }
        }
    }

    // Attempt unprivileged write first
    if try_direct_write(path, content).is_ok() {
        return Ok(());
    }

    // Escalate to sudo tee
    sudo_tee(path, content)
}

fn try_direct_write(path: &Path, content: &str) -> Result<()> {
    let mut file = fs::File::create(path)
        .with_context(|| format!("Cannot open {} for writing", path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write to {}", path.display()))?;
    Ok(())
}

/// `echo <content> | sudo tee <path>`
///
/// stdout from tee is suppressed; stderr (sudo password prompt) is inherited
/// so the user sees it in the terminal after the TUI exits.
fn sudo_tee(path: &Path, content: &str) -> Result<()> {
    let path_str = path.to_string_lossy();
    let mut child = Command::new("sudo")
        .args(["tee", path_str.as_ref()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn `sudo tee`. Ensure sudo is installed and configured.")?;

    // Write config content to tee's stdin
    {
        let stdin = child.stdin.as_mut().context("Failed to open sudo tee stdin")?;
        stdin
            .write_all(content.as_bytes())
            .context("Failed to write config to sudo tee")?;
    }

    let status = child.wait().context("Failed to wait for `sudo tee`")?;
    if !status.success() {
        bail!("`sudo tee {}` exited with status {}", path.display(), status);
    }
    Ok(())
}

fn sudo_mkdir(dir: &Path) -> Result<()> {
    let status = Command::new("sudo")
        .args(["mkdir", "-p", &dir.to_string_lossy()])
        .status()
        .context("Failed to run `sudo mkdir`")?;
    if !status.success() {
        bail!("`sudo mkdir -p {}` failed", dir.display());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_current_theme ---

    #[test]
    fn parses_theme_in_section() {
        let cfg = "[General]\nFoo=bar\n\n[Theme]\nCurrent=breeze\n";
        assert_eq!(parse_current_theme(cfg).as_deref(), Some("breeze"));
    }

    #[test]
    fn parses_theme_first_section() {
        let cfg = "[Theme]\nCurrent=maya\n\n[Users]\nMinimumUid=1000\n";
        assert_eq!(parse_current_theme(cfg).as_deref(), Some("maya"));
    }

    #[test]
    fn returns_none_when_no_theme_section() {
        let cfg = "[General]\nFoo=bar\n";
        assert!(parse_current_theme(cfg).is_none());
    }

    #[test]
    fn returns_none_when_current_missing() {
        let cfg = "[Theme]\nFontSize=12\n";
        assert!(parse_current_theme(cfg).is_none());
    }

    #[test]
    fn ignores_current_outside_theme_section() {
        let cfg = "[General]\nCurrent=breeze\n[Theme]\n";
        assert!(parse_current_theme(cfg).is_none());
    }

    // --- apply_theme_to_content ---

    #[test]
    fn replaces_existing_current() {
        let cfg = "[Theme]\nCurrent=old\n";
        let out = apply_theme_to_content(cfg, "new");
        assert!(out.contains("Current=new"));
        assert!(!out.contains("Current=old"));
    }

    #[test]
    fn inserts_current_into_existing_theme_section() {
        let cfg = "[Theme]\nFontSize=12\n\n[General]\nFoo=bar\n";
        let out = apply_theme_to_content(cfg, "breeze");
        assert!(out.contains("Current=breeze"));
        // Key must appear before [General]
        let pos_key = out.find("Current=breeze").unwrap();
        let pos_general = out.find("[General]").unwrap();
        assert!(pos_key < pos_general);
    }

    #[test]
    fn appends_theme_section_when_absent() {
        let cfg = "[General]\nHaltCommand=/usr/bin/systemctl poweroff\n";
        let out = apply_theme_to_content(cfg, "sugar-candy");
        assert!(out.contains("[Theme]"));
        assert!(out.contains("Current=sugar-candy"));
    }

    #[test]
    fn handles_empty_content() {
        let out = apply_theme_to_content("", "aerial");
        assert!(out.contains("[Theme]"));
        assert!(out.contains("Current=aerial"));
    }

    #[test]
    fn roundtrip_preserves_other_sections() {
        let cfg = "[General]\nNumlock=on\n\n[Theme]\nCurrent=breeze\n\n[Users]\nMinimumUid=1000\n";
        let out = apply_theme_to_content(cfg, "maya");
        assert!(out.contains("Numlock=on"));
        assert!(out.contains("MinimumUid=1000"));
        assert!(out.contains("Current=maya"));
        assert!(!out.contains("Current=breeze"));
    }
}
