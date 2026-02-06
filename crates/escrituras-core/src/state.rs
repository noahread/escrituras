//! UI-agnostic application state types
//!
//! This module contains data structures that are shared between different UIs
//! (TUI, Tauri desktop app, etc.) and don't depend on any specific UI framework.

use serde::{Deserialize, Serialize};

/// A chat message in the AI conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// The role of a chat message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
}
