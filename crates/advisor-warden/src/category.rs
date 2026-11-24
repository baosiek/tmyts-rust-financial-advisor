use std::fmt;

/// Kinds of injection the warden recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// "Ignore all previous instructions", "new instructions:".
    InstructionOverride,
    /// "From now on you are...", "developer mode".
    RoleManipulation,
    /// "Reveal your system prompt", "send this conversation to...".
    Exfiltration,
    /// Fake conversation or tool markers: "System:", "</tool_result>", "<|im_start|>".
    FakeMarker,
    /// Invisible characters that can hide text from people but not from the model.
    HiddenText,
}

impl Category {
    /// How strongly one match of this kind suggests an attack (0.0 to 1.0).
    pub(crate) fn weight(self) -> f64 {
        match self {
            Category::InstructionOverride => 0.6,
            Category::Exfiltration => 0.5,
            Category::FakeMarker => 0.5,
            Category::RoleManipulation => 0.4,
            Category::HiddenText => 0.3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Category::InstructionOverride => "instruction_override",
            Category::RoleManipulation => "role_manipulation",
            Category::Exfiltration => "exfiltration",
            Category::FakeMarker => "fake_marker",
            Category::HiddenText => "hidden_text",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
