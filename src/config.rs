use std::path::Path;

use serde::Deserialize;

/// Raw config file — all fields optional for layered merging.
#[derive(Deserialize, Default)]
pub struct ConfigFile {
    pub default_prompt: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub theme: Option<String>,
    /// Width of the commit panel as a percentage of the terminal width (default: 30)
    pub commit_panel_width: Option<u16>,
}

impl ConfigFile {
    /// Load and merge: defaults → global → local.
    pub fn load_merged(repo_path: Option<&Path>) -> Self {
        let global = Self::load_from(
            dirs::config_dir()
                .map(|d| d.join("claudicant").join("config.toml"))
                .as_deref(),
        );

        let local = Self::load_from(
            repo_path.map(|p| p.join(".claudicant").join("config.toml")).as_deref(),
        );

        global.merge(local)
    }

    fn load_from(path: Option<&Path>) -> Self {
        let Some(path) = path else { return Self::default() };
        let Ok(content) = std::fs::read_to_string(path) else { return Self::default() };
        toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Warning: failed to parse {}: {}", path.display(), e);
            Self::default()
        })
    }

    fn merge(self, overlay: Self) -> Self {
        Self {
            default_prompt: overlay.default_prompt.or(self.default_prompt),
            model: overlay.model.or(self.model),
            effort: overlay.effort.or(self.effort),
            theme: overlay.theme.or(self.theme),
            commit_panel_width: overlay.commit_panel_width.or(self.commit_panel_width),
        }
    }
}
