#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Ollama,
    Claude,
    OpenAI,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Claude => "claude",
            Provider::OpenAI => "openai",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ollama" => Some(Provider::Ollama),
            "claude" => Some(Provider::Claude),
            "openai" => Some(Provider::OpenAI),
            _ => None,
        }
    }

    pub fn all() -> Vec<Provider> {
        vec![Provider::Ollama, Provider::Claude, Provider::OpenAI]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Ollama => "Ollama (Local)",
            Provider::Claude => "Claude (Anthropic)",
            Provider::OpenAI => "ChatGPT (OpenAI)",
        }
    }
}
