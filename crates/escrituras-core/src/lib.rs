pub mod ai;
pub mod config;
pub mod embeddings;
pub mod mcp;
pub mod provider;
pub mod scripture;
pub mod state;

// Re-export main types for convenience
pub use ai::{ClaudeClient, OllamaClient, OpenAIClient};
pub use config::Config;
pub use embeddings::{download_embedding_model, EmbeddingsDb};
pub use provider::Provider;
pub use scripture::{Scripture, ScriptureDb, ScriptureRange};
pub use state::{ChatMessage, ChatRole};
