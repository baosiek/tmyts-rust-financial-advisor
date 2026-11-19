#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage, // real token count from the API
}
