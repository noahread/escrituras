mod app;
mod handler;
mod tui;
mod ui;

use anyhow::Result;
use escrituras_core::{download_embedding_model, mcp, ChatMessage, ChatRole, DataPaths};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Check for MCP server mode
    if args.iter().any(|a| a == "--mcp") {
        return run_mcp_server().await;
    }

    // Check for model download mode (used by install.sh)
    if args.iter().any(|a| a == "--download-model") {
        return download_embedding_model();
    }

    // Run TUI mode
    run_tui().await
}

async fn run_mcp_server() -> Result<()> {
    let paths = DataPaths::discover()?;
    let scripture_db = paths.load_scriptures().await?;
    let embeddings_db = paths.load_embeddings();

    mcp::run_mcp_server(scripture_db, embeddings_db);
    Ok(())
}

async fn run_tui() -> Result<()> {
    // Install panic hook to restore terminal on crash
    tui::install_panic_hook();

    // Initialize terminal
    let mut terminal = tui::init()?;

    // Create app state - restore terminal on failure
    let mut app = match app::App::new().await {
        Ok(app) => app,
        Err(e) => {
            tui::restore()?;
            return Err(e);
        }
    };

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
