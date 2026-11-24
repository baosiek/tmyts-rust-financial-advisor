//! The pattern rules. Written to match attack phrasing, not ordinary financial
//! language: "investors ignored the guidance" or "act as a catalyst" must pass.

use std::sync::LazyLock;

use regex::Regex;

use crate::Category;

pub(crate) struct Rule {
    pub category: Category,
    pub pattern: Regex,
}

fn rule(category: Category, pattern: &str) -> Rule {
    Rule {
        category,
        pattern: Regex::new(pattern).expect("built-in warden pattern is valid"),
    }
}

pub(crate) static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    use Category::*;
    vec![
        // --- instruction override ---
        rule(
            InstructionOverride,
            r"(?i)\b(ignore|disregard|forget|skip)\s+(all\s+|any\s+|the\s+|your\s+)*(previous|prior|above|earlier|preceding|system)\s+(instructions?|prompts?|rules|messages|directions|context)\b",
        ),
        rule(
            InstructionOverride,
            r"(?i)\b(override|bypass)\s+(your|the|all|any)\s+(instructions|rules|guidelines|system\s+prompt|safety)\b",
        ),
        rule(
            InstructionOverride,
            r"(?i)\b(new|updated|real)\s+instructions\s*:",
        ),
        // --- role manipulation ---
        rule(
            RoleManipulation,
            r"(?i)\bfrom\s+now\s+on,?\s+(you|the\s+assistant|the\s+ai)\s+(are|will|must|should)\b",
        ),
        rule(
            RoleManipulation,
            r"(?i)\byou\s+are\s+now\s+(a|an|in|no\s+longer)\b",
        ),
        rule(RoleManipulation, r"(?i)\bpretend\s+(to\s+be|you\s+are)\b"),
        rule(
            RoleManipulation,
            r"(?i)\b(developer|jailbreak|god|dan)\s+mode\b",
        ),
        // --- exfiltration / prompt probing ---
        rule(
            Exfiltration,
            // "the instructions" alone is ordinary English; only "your instructions"
            // or a named prompt counts.
            r"(?i)\b(reveal|print|show|repeat|output|disclose)\s+(me\s+)?((your|the)\s+(system|hidden|initial)\s+prompt|your\s+(instructions|prompt|rules))\b",
        ),
        rule(
            Exfiltration,
            r"(?i)\b(send|post|forward|upload)\s+(this|the|all|your)\s+(conversation|chat|data|information|history|api\s+key)\s+to\b",
        ),
        // --- fake markers (also removed by the sanitizer) ---
        rule(FakeMarker, r"(?im)^\s*(system|assistant|human|user)\s*:"),
        rule(
            FakeMarker,
            r"(?i)</?\s*(system|assistant|user|human|tool_result|tool_use|instructions?)\s*>",
        ),
        rule(
            FakeMarker,
            r"(?i)<\|\s*(im_start|im_end|system|endoftext)\s*\|>",
        ),
        rule(FakeMarker, r"(?i)\[/?\s*(INST|SYS)\s*\]"),
    ]
});

/// Zero-width, bidirectional-override and Unicode tag characters.
pub(crate) fn is_hidden_char(c: char) -> bool {
    matches!(c,
        '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}'
        | '\u{E0000}'..='\u{E007F}')
}
