use crate::Category;

/// What happens to a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing suspicious: passed through unchanged.
    Allow,
    /// Suspicious: passed on with the risky parts removed, and flagged.
    Sanitize,
    /// Very likely an attack: must not reach the model.
    Block,
}

/// The result of screening one text.
#[derive(Debug, Clone, PartialEq)]
pub struct Screening {
    /// 0.0 (clean) to 1.0 (certain attack).
    pub score: f64,
    /// Distinct kinds found, sorted.
    pub categories: Vec<Category>,
    pub action: Action,
    /// The text to use: unchanged for `Allow`, cleaned for `Sanitize`, empty for `Block`.
    pub text: String,
}

impl Screening {
    pub fn is_clean(&self) -> bool {
        self.categories.is_empty()
    }
}
