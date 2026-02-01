mod app;
mod claude;
mod config;
mod embeddings;
mod handler;
mod mcp;
mod ollama;
mod openai;
mod provider;
mod scripture;
mod tui;
mod ui;

use anyhow::Result;
use app::{ChatMessage, ChatRole};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Check for MCP server mode
    if args.iter().any(|a| a == "--mcp") {
        return run_mcp_server().await;
    }

    // Run TUI mode
    run_tui().await
}

async fn run_mcp_server() -> Result<()> {
    // Load scripture database (same path as TUI mode)
    let mut scripture_db = scripture::ScriptureDb::new();
    scripture_db.load_from_json("lds-scriptures-2020.12.08/json/lds-scriptures-json.txt").await?;

    // Load embeddings if available (for semantic search)
    // Try local data/ directory first, then ~/.config/escrituras/data/
    let embeddings_db = {
        let local_path = std::path::Path::new("data");
        let config_path = dirs::config_dir()
            .map(|p| p.join("escrituras/data"));

        if local_path.join("scripture_embeddings.npy").exists() {
            embeddings::EmbeddingsDb::load(local_path).ok()
        } else if let Some(ref cfg_path) = config_path {
            if cfg_path.join("scripture_embeddings.npy").exists() {
                embeddings::EmbeddingsDb::load(cfg_path).ok()
            } else {
                None
            }
        } else {
            None
        }
    };

    mcp::run_mcp_server(scripture_db, embeddings_db);
    Ok(())
}

async fn run_tui() -> Result<()> {
    // Install panic hook to restore terminal on crash
    tui::install_panic_hook();

    // Initialize terminal
    let mut terminal = tui::init()?;

    // Create app state
    let mut app = app::App::new().await?;

    // Create event handler
    let mut events = tui::EventHandler::new();

    // Main loop
    loop {
        // Draw UI
        terminal.draw(|frame| {
            ui::render(&mut app, frame);
        })?;

        // Check if AI query task completed
        if let Some(task) = &app.query_task {
            if task.is_finished() {
                let task = app.query_task.take().unwrap();
                match task.await {
                    Ok(Ok(response)) => {
                        // Extract scripture references from the response
                        let refs = app.scripture_db.extract_scripture_references(&response);
                        app.extracted_references = refs;
                        if !app.extracted_references.is_empty() {
                            app.references_state.select(Some(0));
                        }

                        app.chat_messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: response,
                        });
                    }
                    Ok(Err(e)) => {
                        app.extracted_references.clear();
                        app.chat_messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: format!("Error: {}", e),
                        });
                    }
                    Err(e) => {
                        app.extracted_references.clear();
                        app.chat_messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: format!("Task error: {}", e),
                        });
                    }
                }
                app.query_loading = false;
            }
        }

        // Handle events with timeout so we can poll task completion
        // Use select to either get an event or timeout after 100ms
        tokio::select! {
            event = events.next() => {
                if let Some(event) = event {
                    handler::handle_event(&mut app, event).await?;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Timeout - just continue to redraw and check task
            }
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    tui::restore()?;

    Ok(())
}
