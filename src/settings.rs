/// Claude review settings configurable at runtime.
#[derive(Clone)]
pub struct Settings {
    pub model: ModelChoice,
    pub effort: EffortLevel,
    pub fast_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: ModelChoice::Default,
            effort: EffortLevel::Default,
            fast_mode: false,
        }
    }
}

impl Settings {
    /// Build extra CLI args for `claude -p` based on current settings.
    pub fn claude_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        match &self.model {
            ModelChoice::Default => {}
            ModelChoice::Named(name) => {
                args.push("--model".to_string());
                args.push(name.clone());
            }
        }

        match self.effort {
            EffortLevel::Default => {}
            EffortLevel::Low => {
                args.push("--effort".to_string());
                args.push("low".to_string());
            }
            EffortLevel::Medium => {
                args.push("--effort".to_string());
                args.push("medium".to_string());
            }
            EffortLevel::High => {
                args.push("--effort".to_string());
                args.push("high".to_string());
            }
            EffortLevel::Max => {
                args.push("--effort".to_string());
                args.push("max".to_string());
            }
        }

        args
    }
}

#[derive(Clone, PartialEq)]
pub enum ModelChoice {
    Default,
    Named(String),
}

impl ModelChoice {
    pub fn from_config(s: &str) -> Self {
        match s {
            "" => Self::Default,
            name => Self::Named(name.to_string()),
        }
    }

    pub fn display(&self) -> &str {
        match self {
            Self::Default => "(default)",
            Self::Named(name) => name,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Default => Self::Named("opus".to_string()),
            Self::Named(n) if n == "opus" => Self::Named("sonnet".to_string()),
            Self::Named(n) if n == "sonnet" => Self::Named("haiku".to_string()),
            Self::Named(n) if n == "haiku" => Self::Default,
            Self::Named(_) => Self::Default,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Default => Self::Named("haiku".to_string()),
            Self::Named(n) if n == "haiku" => Self::Named("sonnet".to_string()),
            Self::Named(n) if n == "sonnet" => Self::Named("opus".to_string()),
            Self::Named(n) if n == "opus" => Self::Default,
            Self::Named(_) => Self::Default,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum EffortLevel {
    Default,
    Low,
    Medium,
    High,
    Max,
}

impl EffortLevel {
    pub fn from_config(s: &str) -> Self {
        match s {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "max" => Self::Max,
            _ => Self::Default,
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Self::Default => "(default)",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Default,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Default => Self::Max,
            Self::Max => Self::High,
            Self::High => Self::Medium,
            Self::Medium => Self::Low,
            Self::Low => Self::Default,
        }
    }
}

/// Rows in the settings panel.
#[derive(Clone, Copy, PartialEq)]
pub enum SettingsRow {
    Model,
    Effort,
    FastMode,
    ViewPrompt,
    Version,
}

impl SettingsRow {
    pub const ALL: [Self; 5] = [
        Self::Model,
        Self::Effort,
        Self::FastMode,
        Self::ViewPrompt,
        Self::Version,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(Self::Model)
    }
}
