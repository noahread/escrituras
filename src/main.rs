mod app;
mod claude;
mod config;
mod handler;
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
