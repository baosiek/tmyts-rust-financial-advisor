use std::collections::BTreeSet;

use crate::rules::{RULES, is_hidden_char};
use crate::{Action, Category, Screening, WardenConfig};

/// Screens untrusted text. Cheap to share: holds only the thresholds; the
/// compiled patterns are global and built once.
#[derive(Debug, Clone)]
pub struct PromptWarden {
    config: WardenConfig,
}

impl PromptWarden {
    pub fn new(config: WardenConfig) -> Self {
        Self { config }
    }

    pub fn screen(&self, text: &str) -> Screening {
        let mut categories: BTreeSet<Category> = RULES
            .iter()
            .filter(|rule| rule.pattern.is_match(text))
            .map(|rule| rule.category)
            .collect();
        if text.chars().any(is_hidden_char) {
            categories.insert(Category::HiddenText);
        }

        // Independent signals combine like probabilities: two weak ones are
        // stronger than either, but the score never exceeds 1.
        let score = 1.0 - categories.iter().map(|c| 1.0 - c.weight()).product::<f64>();

        let action = if score >= self.config.block_at() {
            Action::Block
        } else if score >= self.config.sanitize_at() {
            Action::Sanitize
        } else {
            Action::Allow
        };

        let text = match action {
            Action::Allow => text.to_string(),
            Action::Sanitize => sanitize(text),
            Action::Block => String::new(),
        };

        Screening {
            score: (score * 100.0).round() / 100.0,
            categories: categories.into_iter().collect(),
            action,
            text,
        }
    }
}

/// Removes invisible characters and replaces every matched pattern with a marker,
/// so the model sees that something was removed but not the instruction itself.
fn sanitize(text: &str) -> String {
    let mut cleaned: String = text.chars().filter(|c| !is_hidden_char(*c)).collect();
    for rule in RULES.iter() {
        cleaned = rule.pattern.replace_all(&cleaned, "[removed]").into_owned();
    }
    cleaned
}
