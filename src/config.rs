//! User configuration, stored as TOML.
//!
//! Location: `$SLOTH_CONFIG` if set, otherwise `<config dir>/sloth/config.toml`
//! (`~/.config` on Linux, `~/Library/Application Support` on macOS,
//! `%APPDATA%` on Windows).

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Theme name, as shown in the header.
    pub theme: Option<String>,
    /// Directory scanned when `--path` is not given.
    pub default_path: Option<String>,
    /// Directory names skipped while scanning for repositories.
    pub scan_exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            default_path: None,
            scan_exclude: ["node_modules", "target", ".venv", "vendor"]
                .map(String::from)
                .to_vec(),
        }
    }
}

const TEMPLATE: &str = r#"# Sloth configuration — https://github.com/AymericChaverot/sloth

# Directory scanned when --path is not given.
# default_path = "~/Projects"

# Directory names skipped while scanning for repositories.
scan_exclude = ["node_modules", "target", ".venv", "vendor"]
"#;

impl Config {
    pub fn path() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("SLOTH_CONFIG") {
            return Some(PathBuf::from(p));
        }
        dirs::config_dir().map(|d| d.join("sloth").join("config.toml"))
    }

    /// Loads the configuration, creating a commented default file on first run.
    /// Falls back to the defaults, with a warning, when the file cannot be used.
    pub fn load() -> (Self, Option<String>) {
        let Some(path) = Self::path() else {
            return (Self::default(), None);
        };
        if !path.exists() {
            let config = Self {
                theme: migrate_legacy_theme(),
                ..Self::default()
            };
            let _ = write_template(&path, config.theme.as_deref());
            return (config, None);
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => match Self::parse(&text) {
                Ok(config) => (config, None),
                Err(e) => (
                    Self::default(),
                    Some(format!("Invalid config {}: {}", path.display(), e)),
                ),
            },
            Err(e) => (
                Self::default(),
                Some(format!("Cannot read {}: {}", path.display(), e)),
            ),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.message().to_string())
    }

    /// Persists the theme without touching the rest of the file (comments included).
    pub fn save_theme(&mut self, name: &str) {
        self.theme = Some(name.to_string());
        if let Some(path) = Self::path() {
            let _ = update_key(&path, "theme", name);
        }
    }

    /// Directory to scan: the CLI argument, then `default_path`, then the current one.
    pub fn scan_root(&self, cli_path: Option<&str>) -> PathBuf {
        match cli_path.or(self.default_path.as_deref()) {
            Some(p) => expand_tilde(p),
            None => PathBuf::from("."),
        }
    }
}

fn write_template(path: &Path, theme: Option<&str>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = TEMPLATE.to_string();
    if let Some(theme) = theme {
        text.push_str(&format!("\ntheme = {:?}\n", theme));
    }
    std::fs::write(path, text)
}

fn update_key(path: &Path, key: &str, value: &str) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| TEMPLATE.to_string());
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    doc[key] = toml_edit::value(value);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, doc.to_string())
}

/// v0.1 stored the theme index in `~/.sloth_theme`.
fn migrate_legacy_theme() -> Option<String> {
    let legacy = dirs::home_dir()?.join(".sloth_theme");
    let index: usize = std::fs::read_to_string(&legacy).ok()?.trim().parse().ok()?;
    let _ = std::fs::remove_file(&legacy);
    crate::ui::theme::THEMES
        .get(index)
        .map(|t| t.name.to_string())
}

/// Expands a leading `~` to the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    match (path.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses_to_defaults() {
        assert_eq!(Config::parse(TEMPLATE).unwrap(), Config::default());
    }

    #[test]
    fn partial_config_keeps_defaults() {
        let config = Config::parse("theme = \"Nord\"").unwrap();
        assert_eq!(config.theme.as_deref(), Some("Nord"));
        assert_eq!(config.scan_exclude, Config::default().scan_exclude);
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(Config::parse("themes = \"Nord\"").is_err());
    }

    #[test]
    fn cli_path_wins_over_default_path() {
        let config = Config {
            default_path: Some("/projects".into()),
            ..Config::default()
        };
        assert_eq!(config.scan_root(Some("/other")), PathBuf::from("/other"));
        assert_eq!(config.scan_root(None), PathBuf::from("/projects"));
        assert_eq!(Config::default().scan_root(None), PathBuf::from("."));
    }

    #[test]
    fn updates_theme_preserving_comments() {
        let path = std::env::temp_dir().join(format!("sloth-cfg-{}.toml", std::process::id()));
        std::fs::write(&path, TEMPLATE).unwrap();
        update_key(&path, "theme", "Dracula").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("# Sloth configuration"));
        assert_eq!(
            Config::parse(&text).unwrap().theme.as_deref(),
            Some("Dracula")
        );
    }
}
