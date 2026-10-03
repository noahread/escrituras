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

impl std::str::FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ollama" => Ok(Provider::Ollama),
            "claude" => Ok(Provider::Claude),
            "openai" => Ok(Provider::OpenAI),
            _ => Err(format!("unknown provider: {}", s)),
        }
    }
}
