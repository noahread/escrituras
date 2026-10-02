//! AI-assisted scripture study: prompt building and provider dispatch.

use crate::ai::{ClaudeClient, OllamaClient, OpenAIClient};
use crate::config::Config;
use crate::provider::Provider;
use crate::scripture::Scripture;
use crate::state::{ChatMessage, ChatRole};
use anyhow::{anyhow, Result};

const OLLAMA_URL: &str = "http://localhost:11434";
const ANTHROPIC_KEY_VAR: &str = "ANTHROPIC_API_KEY";
const OPENAI_KEY_VAR: &str = "OPENAI_API_KEY";

/// What the user is studying, included in the prompt to ground answers
#[derive(Debug, Default, Clone, Copy)]
pub struct StudyContext<'a> {
    /// The chapter or reference on screen, e.g. "Alma 32" or "Mosiah 4:19-21"
    pub current_reading: Option<&'a str>,
    /// Chapters viewed this session as (book_title, chapter_number)
    pub browsed_chapters: &'a [(String, i32)],
    /// Verses the user saved as context for questions
    pub saved_verses: &'a [Scripture],
}

/// Build a single prompt from the chat history and study context.
/// The last message in `chat_history` is the question being asked.
pub fn build_study_prompt(chat_history: &[ChatMessage], context: &StudyContext) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are helping with LDS (Latter-day Saint) scripture study. ");
    prompt.push_str("When answering, prioritize the Book of Mormon, Doctrine and Covenants, ");
    prompt.push_str(
        "and Pearl of Great Price alongside the Bible. Include specific verse citations.\n\n",
    );

    // Include what the user is currently reading
    if let Some(reading) = context.current_reading {
        prompt.push_str(&format!("The user is currently reading {}.\n\n", reading));
    }

    // Include recently browsed chapters (lightweight context)
    if !context.browsed_chapters.is_empty() {
        prompt.push_str("Recently viewed chapters: ");
        let chapters: Vec<String> = context
            .browsed_chapters
            .iter()
            .take(10) // Limit to last 10 chapters
            .map(|(book, ch)| format!("{} {}", book, ch))
            .collect();
        prompt.push_str(&chapters.join(", "));
        prompt.push_str("\n\n");
    }

    if !context.saved_verses.is_empty() {
        prompt.push_str("Scripture Context:\n");
        for verse in context.saved_verses.iter().take(20) {
            prompt.push_str(&format!(
                "{}: {}\n",
                verse.verse_title, verse.scripture_text
            ));
        }
        prompt.push('\n');
    }

    // Include chat history for context
    if chat_history.len() > 1 {
        prompt.push_str("Conversation so far:\n");
        for msg in chat_history
            .iter()
            .take(chat_history.len().saturating_sub(1))
        {
            match msg.role {
                ChatRole::User => prompt.push_str(&format!("User: {}\n", msg.content)),
                ChatRole::Assistant => prompt.push_str(&format!("Assistant: {}\n", msg.content)),
            }
        }
        prompt.push('\n');
    }

    // Add the current question
    if let Some(last_msg) = chat_history.last() {
        prompt.push_str("Current question: ");
        prompt.push_str(&last_msg.content);
    }

    prompt.push_str("\n\nPlease provide specific scripture references in your answer.");

    prompt
}

/// Where a provider's credentials come from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// No key needed (Ollama runs locally)
    Local,
    /// From an environment variable
    Env,
    /// From the config file or set during this session
    Config,
}

impl KeySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeySource::Local => "local",
            KeySource::Env => "env",
            KeySource::Config => "config",
        }
    }
}

/// Clients for every AI provider, created from environment variables and config
#[derive(Clone)]
pub struct Assistant {
    ollama: OllamaClient,
    claude: Option<ClaudeClient>,
    openai: Option<OpenAIClient>,
    claude_key_from_env: bool,
    openai_key_from_env: bool,
}

impl Assistant {
    /// API keys come from ANTHROPIC_API_KEY / OPENAI_API_KEY first, then the config file
    pub fn from_config(config: &Config) -> Self {
        let claude_env = std::env::var(ANTHROPIC_KEY_VAR).ok();
        let openai_env = std::env::var(OPENAI_KEY_VAR).ok();

        let claude_key = claude_env.clone().or_else(|| config.claude_api_key.clone());
        let openai_key = openai_env.clone().or_else(|| config.openai_api_key.clone());

        Self {
            ollama: OllamaClient::new(OLLAMA_URL),
            claude: claude_key.as_deref().map(ClaudeClient::new),
            openai: openai_key.as_deref().map(OpenAIClient::new),
            claude_key_from_env: claude_env.is_some(),
            openai_key_from_env: openai_env.is_some(),
        }
    }

    /// Use `key` for `provider` for the rest of this session (Ollama needs no key)
    pub fn set_api_key(&mut self, provider: Provider, key: &str) {
        match provider {
            Provider::Ollama => {}
            Provider::Claude => self.claude = Some(ClaudeClient::new(key)),
            Provider::OpenAI => self.openai = Some(OpenAIClient::new(key)),
        }
    }

    /// Where the provider's key comes from, or `None` if it has no key yet
    pub fn key_source(&self, provider: Provider) -> Option<KeySource> {
        let (configured, from_env) = match provider {
            Provider::Ollama => return Some(KeySource::Local),
            Provider::Claude => (self.claude.is_some(), self.claude_key_from_env),
            Provider::OpenAI => (self.openai.is_some(), self.openai_key_from_env),
        };
        match (configured, from_env) {
            (_, true) => Some(KeySource::Env),
            (true, false) => Some(KeySource::Config),
            (false, false) => None,
        }
    }

    /// Whether `provider` can be queried (has a key, or needs none)
    pub fn is_configured(&self, provider: Provider) -> bool {
        self.key_source(provider).is_some()
    }

    /// Models available for `provider`. For Ollama this asks the local
    /// server and returns an empty list if it isn't running.
    pub async fn list_models(&self, provider: Provider) -> Vec<String> {
        match provider {
            Provider::Ollama => self.ollama.list_models().await.unwrap_or_default(),
            Provider::Claude => ClaudeClient::list_models(),
            Provider::OpenAI => OpenAIClient::list_models(),
        }
    }

    /// Send `prompt` to `model` on `provider` and return the reply
    pub async fn ask(&self, provider: Provider, model: &str, prompt: &str) -> Result<String> {
        match provider {
            Provider::Ollama => self.ollama.query(model, prompt).await,
            Provider::Claude => match &self.claude {
                Some(client) => client.query(model, prompt).await,
                None => Err(anyhow!("Claude API key not configured")),
            },
            Provider::OpenAI => match &self.openai {
                Some(client) => client.query(model, prompt).await,
                None => Err(anyhow!("OpenAI API key not configured")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: ChatRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
        }
    }

    fn verse(title: &str, text: &str) -> Scripture {
        Scripture {
            volume_title: "Book of Mormon".to_string(),
            book_title: "Alma".to_string(),
            book_short_title: "Alma".to_string(),
            chapter_number: 32,
            verse_number: 21,
            verse_title: title.to_string(),
            verse_short_title: title.to_string(),
            scripture_text: text.to_string(),
        }
    }

    #[test]
    fn test_prompt_with_question_only() {
        let history = [message(ChatRole::User, "What is faith?")];
        let prompt = build_study_prompt(&history, &StudyContext::default());

        assert!(prompt.starts_with("You are helping with LDS"));
        assert!(prompt.contains("Current question: What is faith?"));
        assert!(!prompt.contains("Conversation so far"));
        assert!(!prompt.contains("Scripture Context"));
        assert!(!prompt.contains("currently reading"));
        assert!(prompt.ends_with("Please provide specific scripture references in your answer."));
    }

    #[test]
    fn test_prompt_includes_context_and_history() {
        let history = [
            message(ChatRole::User, "What is faith?"),
            message(ChatRole::Assistant, "Alma 32:21 explains it."),
            message(ChatRole::User, "How do I grow it?"),
        ];
        let browsed = [("Alma".to_string(), 32), ("Ether".to_string(), 12)];
        let saved = [verse(
            "Alma 32:21",
            "faith is not to have a perfect knowledge",
        )];
        let context = StudyContext {
            current_reading: Some("Alma 32"),
            browsed_chapters: &browsed,
            saved_verses: &saved,
        };
        let prompt = build_study_prompt(&history, &context);

        assert!(prompt.contains("The user is currently reading Alma 32."));
        assert!(prompt.contains("Recently viewed chapters: Alma 32, Ether 12"));
        assert!(prompt.contains("Alma 32:21: faith is not to have a perfect knowledge"));
        assert!(prompt.contains(
            "Conversation so far:\nUser: What is faith?\nAssistant: Alma 32:21 explains it.\n"
        ));
        assert!(!prompt.contains("User: How do I grow it?"));
        assert!(prompt.contains("Current question: How do I grow it?"));
    }

    #[test]
    fn test_prompt_limits_browsed_chapters() {
        let browsed: Vec<(String, i32)> = (1..=12).map(|c| ("Alma".to_string(), c)).collect();
        let context = StudyContext {
            browsed_chapters: &browsed,
            ..Default::default()
        };
        let prompt = build_study_prompt(&[message(ChatRole::User, "q")], &context);

        assert!(prompt.contains("Alma 10\n"));
        assert!(!prompt.contains("Alma 11"));
    }

    #[test]
    fn test_key_source_and_set_api_key() {
        // Built directly so the test doesn't depend on the environment
        let mut assistant = Assistant {
            ollama: OllamaClient::new(OLLAMA_URL),
            claude: None,
            openai: None,
            claude_key_from_env: false,
            openai_key_from_env: false,
        };

        assert_eq!(
            assistant.key_source(Provider::Ollama),
            Some(KeySource::Local)
        );
        assert_eq!(assistant.key_source(Provider::Claude), None);
        assert!(!assistant.is_configured(Provider::OpenAI));

        assistant.set_api_key(Provider::Claude, "key");
        assert_eq!(
            assistant.key_source(Provider::Claude),
            Some(KeySource::Config)
        );
        assert!(assistant.is_configured(Provider::Claude));
        assert!(!assistant.is_configured(Provider::OpenAI));
    }

    #[tokio::test]
    async fn test_ask_without_key_is_an_error() {
        let assistant = Assistant {
            ollama: OllamaClient::new(OLLAMA_URL),
            claude: None,
            openai: None,
            claude_key_from_env: false,
            openai_key_from_env: false,
        };
        let err = assistant
            .ask(Provider::Claude, "model", "prompt")
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "Claude API key not configured");
    }
}
