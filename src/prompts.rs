use std::path::{Path, PathBuf};

/// A user prompt snippet that can be appended to the base review prompt.
#[derive(Clone)]
pub struct PromptEntry {
    pub name: String,
    pub source: PromptSource,
    pub path: Option<String>,
    pub content: String,
}

#[derive(Clone, PartialEq)]
pub enum PromptSource {
    BuiltIn,
    Global,
    Local,
}


/// Load all available prompt snippets from global and local directories.
/// Local prompts with the same name shadow global ones.
/// The built-in "(none)" entry is always first.
pub fn load_prompts(repo_path: Option<&Path>) -> Vec<PromptEntry> {
    let mut prompts = vec![PromptEntry {
        name: "(none)".to_string(),
        source: PromptSource::BuiltIn,
        path: None,
        content: String::new(),
    }];

    // Global: ~/.config/claudicant/prompts/
    if let Some(dir) = global_prompts_dir() {
        load_from_dir(&dir, PromptSource::Global, &mut prompts);
    }

    // Local: <repo>/.claudicant/prompts/
    if let Some(repo) = repo_path {
        let local_dir = repo.join(".claudicant").join("prompts");
        load_from_dir(&local_dir, PromptSource::Local, &mut prompts);
    }

    prompts
}

fn global_prompts_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("claudicant").join("prompts"))
}

fn load_from_dir(dir: &Path, source: PromptSource, prompts: &mut Vec<PromptEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        // Local shadows global with the same name
        if source == PromptSource::Local {
            prompts.retain(|p| !(p.name == name && p.source == PromptSource::Global));
        }

        prompts.push(PromptEntry {
            name: name.to_string(),
            source: source.clone(),
            path: Some(path.to_string_lossy().to_string()),
            content: content.trim().to_string(),
        });
    }
}
