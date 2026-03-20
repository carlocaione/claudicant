use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: String,
    cache: HashMap<(String, String), Vec<(Style, String)>>,
}

impl Highlighter {
    pub fn new(theme_name: &str) -> Self {
        let theme_set = ThemeSet::load_defaults();

        let effective_name = if theme_name.is_empty() || !theme_set.themes.contains_key(theme_name) {
            String::new() // no highlighting
        } else {
            theme_name.to_string()
        };

        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set,
            theme_name: effective_name,
            cache: HashMap::new(),
        }
    }

    pub fn highlight_line(
        &mut self,
        content: &str,
        extension: &str,
    ) -> Vec<Span<'static>> {
        if self.theme_name.is_empty() {
            return vec![Span::raw(content.to_string())];
        }

        let cache_key = (extension.to_string(), content.to_string());

        let cached = self.cache.entry(cache_key).or_insert_with(|| {
            let syntax = self
                .syntax_set
                .find_syntax_by_extension(extension)
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

            let theme = &self.theme_set.themes[&self.theme_name];
            let mut h = HighlightLines::new(syntax, theme);
            let line_with_nl = format!("{content}\n");
            let segments = match h.highlight_line(&line_with_nl, &self.syntax_set) {
                Ok(segments) => segments,
                Err(_) => return vec![(Style::default(), content.to_string())],
            };

            segments
                .into_iter()
                .filter_map(|(style, text)| {
                    let text = text.trim_end_matches('\n').to_string();
                    if text.is_empty() {
                        return None;
                    }
                    Some((syntect_to_ratatui_style(style), text))
                })
                .collect()
        });

        cached
            .iter()
            .map(|(style, text)| {
                let mut s = *style;
                s.bg = None;
                Span::styled(text.clone(), s)
            })
            .collect()
    }
}

fn syntect_to_ratatui_style(style: syntect::highlighting::Style) -> Style {
    let fg = if style.foreground.a > 0 {
        Some(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ))
    } else {
        None
    };

    let mut ratatui_style = Style::default();
    if let Some(fg) = fg {
        ratatui_style = ratatui_style.fg(fg);
    }

    let fs = style.font_style;
    if fs.contains(FontStyle::BOLD) {
        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
    }
    if fs.contains(FontStyle::ITALIC) {
        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
    }
    if fs.contains(FontStyle::UNDERLINE) {
        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
    }

    ratatui_style
}

/// Extract file extension from a path like "src/foo/bar.rs" → "rs"
pub fn extension_from_path(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or("")
}
