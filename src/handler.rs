use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use crate::app::{App, ChatMessage, ChatRole, FocusPane, InputMode, Screen, SearchFocus};
use crate::tui::AppEvent;
use crate::provider::Provider;
use crate::claude::ClaudeClient;
use crate::openai::OpenAIClient;
use crate::config::Config;

/// Convert a character index to a byte index for UTF-8 safe string operations
fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

pub async fn handle_event(app: &mut App, event: AppEvent) -> Result<()> {
    match event {
        AppEvent::Key(key) => handle_key(app, key).await?,
        AppEvent::Mouse(mouse) => handle_mouse(app, mouse),
        AppEvent::Resize(_, _) => {}
        AppEvent::Tick => {
            app.tick_animation();
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Global keys that work in any mode
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return Ok(());
    }

    match app.input_mode {
        InputMode::Normal => handle_normal_mode(app, key).await?,
        InputMode::Editing => handle_editing_mode(app, key).await?,
    }

    Ok(())
}

async fn handle_normal_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.screen {
        Screen::Browse => handle_browse_normal(app, key)?,
        Screen::Search => handle_search_normal(app, key),
        Screen::Query => handle_query_normal(app, key).await?,
    }
    Ok(())
}

fn handle_browse_normal(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,

        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            if app.focus == FocusPane::Navigation {
                app.nav_down();
            } else if app.show_context_panel {
                app.context_nav_down();
            } else {
                app.select_next_verse();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.focus == FocusPane::Navigation {
                app.nav_up();
            } else if app.show_context_panel {
                app.context_nav_up();
            } else {
                app.select_prev_verse();
            }
        }
        KeyCode::Char('g') => {
            if app.focus == FocusPane::Navigation {
                app.nav_first();
            } else {
                app.selected_verse_idx = Some(0);
                app.content_scroll = 0;
            }
        }
        KeyCode::Char('G') => {
            if app.focus == FocusPane::Navigation {
                app.nav_last();
            } else {
                let last = app.cached_verses.len().saturating_sub(1);
                app.selected_verse_idx = Some(last);
                app.content_scroll = app.total_content_lines.saturating_sub(app.content_height);
            }
        }

        // Enter/Select
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if app.focus == FocusPane::Navigation {
                app.nav_enter();
            }
        }

        // Back
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => {
            if app.focus == FocusPane::Content {
                app.focus = FocusPane::Navigation;
            } else {
                app.nav_back();
            }
        }

        // Tab to switch focus (Browse only has Nav and Content)
        KeyCode::Tab => {
            app.focus = match app.focus {
                FocusPane::Navigation => {
                    // Select first verse when entering content pane
                    if app.selected_verse_idx.is_none() && !app.cached_verses.is_empty() {
                        app.selected_verse_idx = Some(0);
                    }
                    FocusPane::Content
                }
                FocusPane::Content | FocusPane::References | FocusPane::Input => FocusPane::Navigation,
            };
        }

        // Half-page scroll
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_half_page_down();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_half_page_up();
        }

        // Verse actions (only when Content is focused)
        KeyCode::Char('c') => {
            if app.focus == FocusPane::Content {
                if let Some(verse) = app.get_selected_verse() {
                    let text = format!("{}\n{}", verse.verse_title, verse.scripture_text);
                    copy_to_clipboard(&text);
                }
            }
        }
        KeyCode::Char('x') => {
            if app.focus == FocusPane::Content && !app.show_context_panel {
                if let Some(verse) = app.get_selected_verse().cloned() {
                    if !app.session_context.iter().any(|v| v.verse_title == verse.verse_title) {
                        app.session_context.push(verse);
                    }
                }
            }
        }
        // Toggle saved scriptures panel
        KeyCode::Char('X') => {
            app.show_context_panel = !app.show_context_panel;
            if app.show_context_panel && app.context_state.selected().is_none() && !app.session_context.is_empty() {
                app.context_state.select(Some(0));
            }
        }
        // Remove from saved scriptures when panel is shown
        KeyCode::Char('d') if app.focus == FocusPane::Content && app.show_context_panel => {
            app.remove_selected_context();
        }
        KeyCode::Char('s') => {
            if app.focus == FocusPane::Content {
                if let Some(verse) = app.get_selected_verse() {
                    app.search_input = verse.verse_title.clone();
                    app.perform_search();
                    app.screen = Screen::Search;
                }
            }
        }

        // Screen switching
        KeyCode::Char('/') => {
            app.screen = Screen::Search;
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Char('a') => {
            app.screen = Screen::Query;
            app.input_mode = InputMode::Editing;
        }

        _ => {}
    }
    Ok(())
}

fn handle_search_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        // Back to browse
        KeyCode::Esc => {
            app.screen = Screen::Browse;
            app.search_input.clear();
            app.search_results.clear();
            app.search_focus = SearchFocus::Results;
        }

        // Tab cycles focus: Results -> Preview -> Results
        KeyCode::Tab => {
            app.search_focus = match app.search_focus {
                SearchFocus::Results => {
                    // When entering saved scriptures panel, select first item
                    if app.show_context_panel && app.context_state.selected().is_none()
                        && !app.session_context.is_empty()
                    {
                        app.context_state.select(Some(0));
                    }
                    SearchFocus::Preview
                }
                SearchFocus::Preview => SearchFocus::Results,
            };
        }

        // Navigation - depends on focus and panel
        KeyCode::Char('j') | KeyCode::Down => {
            if app.search_focus == SearchFocus::Preview && app.show_context_panel {
                app.context_nav_down();
            } else {
                app.search_nav_down();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.search_focus == SearchFocus::Preview && app.show_context_panel {
                app.context_nav_up();
            } else {
                app.search_nav_up();
            }
        }

        // Toggle saved scriptures panel
        KeyCode::Char('X') => {
            app.show_context_panel = !app.show_context_panel;
            if app.show_context_panel && app.context_state.selected().is_none()
                && !app.session_context.is_empty()
            {
                app.context_state.select(Some(0));
            }
        }

        // Save scripture (when Preview focused, not showing saved panel)
        KeyCode::Char('x') => {
            if app.search_focus == SearchFocus::Preview && !app.show_context_panel {
                if let Some(i) = app.search_state.selected() {
                    if let Some(scripture) = app.search_results.get(i).cloned() {
                        if !app.session_context.iter().any(|v| v.verse_title == scripture.verse_title) {
                            app.session_context.push(scripture);
                        }
                    }
                }
            }
        }

        // Remove from saved (when Saved panel focused)
        KeyCode::Char('d') => {
            if app.search_focus == SearchFocus::Preview && app.show_context_panel {
                app.remove_selected_context();
            }
        }

        // Copy scripture (when Preview focused)
        KeyCode::Char('c') => {
            if app.search_focus == SearchFocus::Preview && !app.show_context_panel {
                if let Some(i) = app.search_state.selected() {
                    if let Some(scripture) = app.search_results.get(i) {
                        let text = format!("{}\n{}", scripture.verse_title, scripture.scripture_text);
                        copy_to_clipboard(&text);
                    }
                }
            }
        }

        // Edit search
        KeyCode::Char('i') | KeyCode::Char('/') => {
            app.input_mode = InputMode::Editing;
        }

        // View selected result (go to that chapter)
        KeyCode::Enter => {
            if app.search_focus == SearchFocus::Results {
                if let Some(i) = app.search_state.selected() {
                    if let Some(scripture) = app.search_results.get(i).cloned() {
                        app.jump_to_scripture(&scripture);
                        app.screen = Screen::Browse;
                        app.focus = FocusPane::Content;
                    }
                }
            }
        }

        _ => {}
    }
}

async fn handle_query_normal(app: &mut App, key: KeyEvent) -> Result<()> {
    // Handle API key input if it's open
    if app.show_api_key_input {
        match key.code {
            KeyCode::Esc => {
                app.show_api_key_input = false;
                app.api_key_input.clear();
                app.api_key_target_provider = None;
            }
            KeyCode::Enter => {
                if !app.api_key_input.is_empty() {
                    if let Some(provider) = app.api_key_target_provider {
                        let mut config = Config::load().unwrap_or_else(|_| Config::new());
                        match provider {
                            Provider::Claude => {
                                config.claude_api_key = Some(app.api_key_input.clone());
                                app.claude_client = Some(ClaudeClient::new(&app.api_key_input));
                            }
                            Provider::OpenAI => {
                                config.openai_api_key = Some(app.api_key_input.clone());
                                app.openai_client = Some(OpenAIClient::new(&app.api_key_input));
                            }
                            Provider::Ollama => {}
                        }
                        config.provider = Some(provider.as_str().to_string());
                        let _ = config.save();
                        app.current_provider = provider;
                        // Set default model for the new provider
                        let models = app.get_models_for_provider(provider);
                        if let Some(model) = models.first() {
                            app.selected_model = model.clone();
                        }
                    }
                }
                app.show_api_key_input = false;
                app.api_key_input.clear();
                app.api_key_target_provider = None;
            }
            KeyCode::Backspace => {
                if app.api_key_input_cursor > 0 {
                    app.api_key_input_cursor -= 1;
                    let byte_pos = char_to_byte_index(&app.api_key_input, app.api_key_input_cursor);
                    app.api_key_input.remove(byte_pos);
                }
            }
            KeyCode::Char(c) => {
                let byte_pos = char_to_byte_index(&app.api_key_input, app.api_key_input_cursor);
                app.api_key_input.insert(byte_pos, c);
                app.api_key_input_cursor += 1;
            }
            KeyCode::Left => {
                app.api_key_input_cursor = app.api_key_input_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let char_count = app.api_key_input.chars().count();
                app.api_key_input_cursor = (app.api_key_input_cursor + 1).min(char_count);
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle provider picker if it's open
    if app.show_provider_picker {
        match key.code {
            KeyCode::Esc => {
                app.show_provider_picker = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.provider_picker_nav_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.provider_picker_nav_up();
            }
            KeyCode::Enter => {
                if let Some(i) = app.provider_picker_state.selected() {
                    let providers = Provider::all();
                    if let Some(&provider) = providers.get(i) {
                        // Check if API key is needed (client not initialized)
                        let needs_key = app.get_key_source(provider).is_none();
                        if needs_key {
                            app.api_key_target_provider = Some(provider);
                            app.show_api_key_input = true;
                            app.api_key_input.clear();
                            app.api_key_input_cursor = 0;
                        } else {
                            app.current_provider = provider;
                            // Save provider to config
                            let mut config = Config::load().unwrap_or_else(|_| Config::new());
                            config.provider = Some(provider.as_str().to_string());
                            let _ = config.save();
                            // Set model for the new provider
                            match provider {
                                Provider::Ollama => {
                                    // Fetch Ollama models
                                    if let Ok(models) = app.ollama.list_models().await {
                                        if let Some(model) = models.first() {
                                            app.selected_model = model.clone();
                                        }
                                    }
                                }
                                _ => {
                                    let models = app.get_models_for_provider(provider);
                                    if let Some(model) = models.first() {
                                        app.selected_model = model.clone();
                                    }
                                }
                            }
                        }
                        app.show_provider_picker = false;
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle model picker if it's open
    if app.show_model_picker {
        match key.code {
            KeyCode::Esc => {
                app.show_model_picker = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.model_picker_nav_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.model_picker_nav_up();
            }
            KeyCode::Enter => {
                app.select_model();
            }
            _ => {}
        }
        return Ok(());
    }

    match key.code {
        // Back to browse (or exit input, or pop navigation stack)
        KeyCode::Esc => {
            if app.focus == FocusPane::Input {
                // Exit input mode, return to chat
                app.input_mode = InputMode::Normal;
                app.focus = FocusPane::Navigation;
            } else if !app.pop_navigation_state() {
                app.screen = Screen::Browse;
                app.clear_selected_range(); // Clear range highlight when leaving AI mode
            }
        }

        // Go back in navigation stack
        KeyCode::Char('b') | KeyCode::Backspace => {
            if app.focus != FocusPane::Input {
                app.pop_navigation_state();
            }
        }

        // Tab cycles: Navigation -> Input -> Content -> References -> Navigation
        KeyCode::Tab => {
            app.focus = match app.focus {
                FocusPane::Navigation => FocusPane::Input,
                FocusPane::Input => {
                    // Exit editing when leaving input
                    app.input_mode = InputMode::Normal;
                    if app.show_context_panel {
                        if app.context_state.selected().is_none() && !app.session_context.is_empty() {
                            app.context_state.select(Some(0));
                        }
                    } else if app.selected_verse_idx.is_none() && !app.cached_verses.is_empty() {
                        app.selected_verse_idx = Some(0);
                    }
                    FocusPane::Content
                }
                FocusPane::Content => {
                    if !app.extracted_references.is_empty() {
                        FocusPane::References
                    } else {
                        FocusPane::Navigation
                    }
                }
                FocusPane::References => FocusPane::Navigation,
            };

            // Auto-enter editing mode when focusing input
            if app.focus == FocusPane::Input {
                app.input_mode = InputMode::Editing;
                // Cursor at end of existing text
                app.query_cursor = app.query_input.chars().count();
            }
        }

        // Toggle context panel view
        KeyCode::Char('X') => {
            app.show_context_panel = !app.show_context_panel;
            // When entering context view, select first item if any
            if app.show_context_panel && app.context_state.selected().is_none() && !app.session_context.is_empty() {
                app.context_state.select(Some(0));
            }
        }

        // Enter to jump to selected reference (when References focused)
        KeyCode::Enter => {
            if app.focus == FocusPane::References {
                if let Some(idx) = app.references_state.selected() {
                    if let Some(range) = app.extracted_references.get(idx).cloned() {
                        app.push_navigation_state();
                        app.jump_to_scripture_range(&range);
                        // Focus the content pane so user can see the selected verse
                        app.focus = FocusPane::Content;
                    }
                }
            }
        }

        // Scroll/navigate based on focus
        KeyCode::Char('j') | KeyCode::Down => {
            match app.focus {
                FocusPane::Navigation => app.query_scroll = app.query_scroll.saturating_add(1),
                FocusPane::Content => {
                    if app.show_context_panel {
                        app.context_nav_down();
                    } else {
                        app.select_next_verse();
                    }
                }
                FocusPane::References => app.references_nav_down(),
                FocusPane::Input => {} // Handled by editing mode
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            match app.focus {
                FocusPane::Navigation => app.query_scroll = app.query_scroll.saturating_sub(1),
                FocusPane::Content => {
                    if app.show_context_panel {
                        app.context_nav_up();
                    } else {
                        app.select_prev_verse();
                    }
                }
                FocusPane::References => app.references_nav_up(),
                FocusPane::Input => {} // Handled by editing mode
            }
        }

        // Half-page scroll for content (must be before plain 'd' to match first)
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.focus == FocusPane::Content {
                app.scroll_half_page_down();
            }
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.focus == FocusPane::Content {
                app.scroll_half_page_up();
            }
        }

        // Remove from context when context panel is focused
        KeyCode::Char('d') => {
            if app.focus == FocusPane::Content && app.show_context_panel {
                app.remove_selected_context();
            }
        }

        // Jump to top/bottom of content
        KeyCode::Char('g') => {
            if app.focus == FocusPane::Content {
                app.selected_verse_idx = Some(0);
                app.content_scroll = 0;
            } else {
                app.query_scroll = 0;
            }
        }
        KeyCode::Char('G') => {
            if app.focus == FocusPane::Content {
                let last = app.cached_verses.len().saturating_sub(1);
                app.selected_verse_idx = Some(last);
                app.content_scroll = app.total_content_lines.saturating_sub(app.content_height);
            }
        }

        // Verse actions (only when Content is focused)
        KeyCode::Char('c') => {
            if app.focus == FocusPane::Content {
                if let Some(verse) = app.get_selected_verse() {
                    let text = format!("{}\n{}", verse.verse_title, verse.scripture_text);
                    copy_to_clipboard(&text);
                }
            }
        }
        KeyCode::Char('x') => {
            if app.focus == FocusPane::Content {
                if let Some(verse) = app.get_selected_verse().cloned() {
                    if !app.session_context.iter().any(|v| v.verse_title == verse.verse_title) {
                        app.session_context.push(verse);
                    }
                }
            }
        }

        // Open model picker
        KeyCode::Char('M') => {
            // Fetch available models based on current provider
            let models = match app.current_provider {
                Provider::Ollama => {
                    app.ollama.list_models().await.unwrap_or_default()
                }
                Provider::Claude => ClaudeClient::list_models(),
                Provider::OpenAI => OpenAIClient::list_models(),
            };
            app.available_models = models;
            if !app.available_models.is_empty() {
                // Select current model if in list, otherwise first
                let current_idx = app.available_models
                    .iter()
                    .position(|m| m == &app.selected_model)
                    .unwrap_or(0);
                app.model_picker_state.select(Some(current_idx));
                app.show_model_picker = true;
            }
        }

        // Open provider picker
        KeyCode::Char('P') => {
            let current_idx = Provider::all()
                .iter()
                .position(|p| *p == app.current_provider)
                .unwrap_or(0);
            app.provider_picker_state.select(Some(current_idx));
            app.show_provider_picker = true;
        }

        _ => {}
    }
    Ok(())
}

async fn handle_editing_mode(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.screen {
        Screen::Search => handle_search_editing(app, key),
        Screen::Query => handle_query_editing(app, key).await?,
        _ => {}
    }
    Ok(())
}

fn handle_search_editing(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.perform_search();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_input.pop();
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
        }
        _ => {}
    }
}

async fn handle_query_editing(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if !app.query_input.is_empty() && app.query_task.is_none() {
                // Add user message to chat history
                let user_message = app.query_input.clone();
                app.chat_messages.push(ChatMessage {
                    role: ChatRole::User,
                    content: user_message,
                });

                // Build prompt with chat history, session context, and browsed chapters
                let prompt = build_query_prompt(&app.chat_messages, &app.session_context, &app.browsed_chapters);

                app.query_input.clear();
                app.query_cursor = 0;
                app.query_loading = true;
                app.input_mode = InputMode::Normal;

                // Scroll to bottom so "Thinking..." is visible
                app.scroll_query_to_bottom();

                // Spawn background task to query the AI provider
                let model = app.selected_model.clone();
                let provider = app.current_provider;

                match provider {
                    Provider::Ollama => {
                        let ollama = app.ollama.clone();
                        app.query_task = Some(tokio::spawn(async move {
                            ollama.query(&model, &prompt).await
                        }));
                    }
                    Provider::Claude => {
                        if let Some(client) = app.claude_client.clone() {
                            app.query_task = Some(tokio::spawn(async move {
                                client.query(&model, &prompt).await
                            }));
                        } else {
                            app.query_loading = false;
                            app.chat_messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content: "Error: Claude API key not configured. Press 'P' to set up.".to_string(),
                            });
                        }
                    }
                    Provider::OpenAI => {
                        if let Some(client) = app.openai_client.clone() {
                            app.query_task = Some(tokio::spawn(async move {
                                client.query(&model, &prompt).await
                            }));
                        } else {
                            app.query_loading = false;
                            app.chat_messages.push(ChatMessage {
                                role: ChatRole::Assistant,
                                content: "Error: OpenAI API key not configured. Press 'P' to set up.".to_string(),
                            });
                        }
                    }
                }
            }
        }
        KeyCode::Backspace => {
            if app.query_cursor > 0 {
                app.query_cursor -= 1;
                let byte_pos = char_to_byte_index(&app.query_input, app.query_cursor);
                app.query_input.remove(byte_pos);
            }
        }
        KeyCode::Delete => {
            let char_count = app.query_input.chars().count();
            if app.query_cursor < char_count {
                let byte_pos = char_to_byte_index(&app.query_input, app.query_cursor);
                app.query_input.remove(byte_pos);
            }
        }
        KeyCode::Left => {
            app.query_cursor = app.query_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            let char_count = app.query_input.chars().count();
            app.query_cursor = (app.query_cursor + 1).min(char_count);
        }
        KeyCode::Home => {
            app.query_cursor = 0;
        }
        KeyCode::End => {
            app.query_cursor = app.query_input.chars().count();
        }
        KeyCode::Char(c) => {
            let byte_pos = char_to_byte_index(&app.query_input, app.query_cursor);
            app.query_input.insert(byte_pos, c);
            app.query_cursor += 1;
        }
        _ => {}
    }
    Ok(())
}

fn build_query_prompt(
    chat_history: &[ChatMessage],
    context: &[crate::scripture::Scripture],
    browsed_chapters: &[(String, i32)],
) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are helping with LDS (Latter-day Saint) scripture study. ");
    prompt.push_str("When answering, prioritize the Book of Mormon, Doctrine and Covenants, ");
    prompt.push_str("and Pearl of Great Price alongside the Bible. Include specific verse citations.\n\n");

    // Include recently browsed chapters (lightweight context)
    if !browsed_chapters.is_empty() {
        prompt.push_str("Recently viewed chapters: ");
        let chapters: Vec<String> = browsed_chapters.iter()
            .take(10)  // Limit to last 10 chapters
            .map(|(book, ch)| format!("{} {}", book, ch))
            .collect();
        prompt.push_str(&chapters.join(", "));
        prompt.push_str("\n\n");
    }

    if !context.is_empty() {
        prompt.push_str("Scripture Context:\n");
        for verse in context.iter().take(20) {
            prompt.push_str(&format!("{}: {}\n", verse.verse_title, verse.scripture_text));
        }
        prompt.push('\n');
    }

    // Include chat history for context
    if chat_history.len() > 1 {
        prompt.push_str("Conversation so far:\n");
        for msg in chat_history.iter().take(chat_history.len().saturating_sub(1)) {
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

/// Check if a point is within a rectangle
fn point_in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    let x = mouse.column;
    let y = mouse.row;

    // Determine which area the mouse is in (position-based scrolling)
    let in_nav = app.nav_area.map(|r| point_in_rect(x, y, r)).unwrap_or(false);
    let in_content = app.content_area.map(|r| point_in_rect(x, y, r)).unwrap_or(false);
    let in_refs = app.refs_area.map(|r| point_in_rect(x, y, r)).unwrap_or(false);

    match mouse.kind {
        MouseEventKind::ScrollDown => {
            match app.screen {
                Screen::Browse => {
                    if in_content {
                        app.scroll_down();
                        app.scroll_down();
                        app.scroll_down();
                    } else if in_nav {
                        app.nav_down();
                    }
                }
                Screen::Query => {
                    if in_nav {
                        app.query_scroll = app.query_scroll.saturating_add(3);
                    } else if in_content {
                        app.scroll_down();
                        app.scroll_down();
                        app.scroll_down();
                    } else if in_refs {
                        app.references_nav_down();
                    }
                }
                Screen::Search => {
                    app.search_nav_down();
                }
            }
        }
        MouseEventKind::ScrollUp => {
            match app.screen {
                Screen::Browse => {
                    if in_content {
                        app.scroll_up();
                        app.scroll_up();
                        app.scroll_up();
                    } else if in_nav {
                        app.nav_up();
                    }
                }
                Screen::Query => {
                    if in_nav {
                        app.query_scroll = app.query_scroll.saturating_sub(3);
                    } else if in_content {
                        app.scroll_up();
                        app.scroll_up();
                        app.scroll_up();
                    } else if in_refs {
                        app.references_nav_up();
                    }
                }
                Screen::Search => {
                    app.search_nav_up();
                }
            }
        }
        _ => {}
    }
}

fn copy_to_clipboard(text: &str) {
    use std::process::{Command, Stdio};
    use std::io::Write;

    if let Ok(mut child) = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
    }
}
