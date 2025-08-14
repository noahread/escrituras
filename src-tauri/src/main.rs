use tauri::{command, State};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct ChatRequest {
    message: String,
    context: Vec<Scripture>,
    history: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Scripture {
    volume_id: i32,
    book_id: i32,
    chapter_id: i32,
    verse_id: i32,
    volume_title: String,
    book_title: String,
    volume_long_title: String,
    book_long_title: String,
    volume_subtitle: String,
    book_subtitle: String,
    volume_short_title: String,
    book_short_title: String,
    volume_lds_url: String,
    book_lds_url: String,
    chapter_number: i32,
    verse_number: i32,
    scripture_text: String,
    verse_title: String,
    verse_short_title: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    id: String,
    role: String,
    content: String,
    timestamp: String,
    context: Option<Vec<Scripture>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatResponse {
    response: String,
}

type LlmConfig = Mutex<HashMap<String, String>>;

#[command]
async fn chat_with_llm(
    request: ChatRequest,
    _config: State<'_, LlmConfig>,
) -> Result<ChatResponse, String> {
    
    // Build context from selected scriptures
    let context_text = if !request.context.is_empty() {
        let context_verses: Vec<String> = request.context
            .iter()
            .map(|s| format!("{}: {}", s.verse_title, s.scripture_text))
            .collect();
        format!("\n\nScripture Context:\n{}", context_verses.join("\n"))
    } else {
        String::new()
    };

    // For now, return a simple response
    // Later we'll integrate with actual LLM providers
    let response = format!(
        "Thank you for your question: \"{}\"\n\n{}\n\nThis is a placeholder response. LLM integration coming soon!",
        request.message,
        if !context_text.is_empty() {
            format!("I can see you've selected {} scripture(s) for context.", request.context.len())
        } else {
            "No scripture context selected.".to_string()
        }
    );

    Ok(ChatResponse { response })
}

#[command]
async fn set_llm_config(key: String, value: String, config: State<'_, LlmConfig>) -> Result<(), String> {
    let mut config = config.lock().unwrap();
    config.insert(key, value);
    Ok(())
}

#[command]
async fn get_llm_config(key: String, config: State<'_, LlmConfig>) -> Result<Option<String>, String> {
    let config = config.lock().unwrap();
    Ok(config.get(&key).cloned())
}

fn main() {
    tauri::Builder::default()
        .manage(LlmConfig::new(HashMap::new()))
        .invoke_handler(tauri::generate_handler![
            chat_with_llm,
            set_llm_config,
            get_llm_config
        ])
        .setup(|app| {
            // Setup code here if needed
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}