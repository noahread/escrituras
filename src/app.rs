use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::collections::HashSet;
use crate::embeddings::EmbeddingsDb;
use crate::scripture::{Scripture, ScriptureDb, ScriptureRange};
use crate::ollama::OllamaClient;
use crate::claude::ClaudeClient;
use crate::openai::OpenAIClient;
use crate::provider::Provider;
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Search,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavLevel {
    Volume,
    Book,
    Chapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Navigation,
    Content,
    References,
    Input,  // Query input box (AI mode only)
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFocus {
    #[default]
    Results,
    Preview,
}

/// Saved navigation state for returning to previous location
#[derive(Debug, Clone)]
pub struct NavigationState {
    pub volume_idx: Option<usize>,
    pub book_idx: Option<usize>,
    pub chapter_idx: Option<usize>,
    pub scroll: u16,
}

pub struct App {
    // Core state
    pub should_quit: bool,
    pub screen: Screen,
    pub input_mode: InputMode,
    pub focus: FocusPane,

    // Navigation state
    pub nav_level: NavLevel,
    pub volume_state: ListState,
    pub book_state: ListState,
    pub chapter_state: ListState,

    // Content state
    pub content_scroll: u16,
    pub content_height: u16,
    pub total_content_lines: u16,

    // Search state
    pub search_input: String,
    pub search_results: Vec<Scripture>,
    pub search_state: ListState,
    pub search_focus: SearchFocus,

    // AI Query state (chat history)
    pub query_input: String,
    pub query_cursor: usize, // cursor position in query_input
    pub chat_messages: Vec<ChatMessage>,
    pub query_loading: bool,
    pub query_scroll: u16,
    pub query_chat_height: u16, // Height of chat area for scroll calculations
    pub query_chat_width: u16,  // Width of chat area for wrap calculations
    pub query_task: Option<tokio::task::JoinHandle<anyhow::Result<String>>>,
    pub extracted_references: Vec<ScriptureRange>,
    pub references_state: ListState,

    // Navigation history (for returning after jumping to references)
    pub navigation_stack: Vec<NavigationState>,

    // Verse selection (for copy/context actions)
    pub selected_verse_idx: Option<usize>,
    // Range selection (when jumping from References - highlights multiple verses)
    pub selected_range: Option<ScriptureRange>,

    // Session context
    pub session_context: Vec<Scripture>,
    pub context_state: ListState,        // For navigating context list
    pub show_context_panel: bool,        // Toggle between scripture and context view

    // Browsed chapters (for AI context, lightweight tracking)
    pub browsed_chapters: Vec<(String, i32)>,  // (book_title, chapter_number)

    // Animation state
    pub animation_frame: u8, // 0-2 for ellipsis animation

    // Model picker state
    pub show_model_picker: bool,
    pub available_models: Vec<String>,
    pub model_picker_state: ListState,

    // Provider state
    pub current_provider: Provider,
    pub claude_client: Option<ClaudeClient>,
    pub openai_client: Option<OpenAIClient>,
    pub show_provider_picker: bool,
    pub provider_picker_state: ListState,

    // API key input state
    pub show_api_key_input: bool,
    pub api_key_input: String,
    pub api_key_input_cursor: usize,
    pub api_key_target_provider: Option<Provider>,

    // Panel areas for mouse hit-testing (updated during render)
    pub nav_area: Option<Rect>,
    pub content_area: Option<Rect>,
    pub refs_area: Option<Rect>,

    // Data
    pub scripture_db: ScriptureDb,
    pub embeddings_db: Option<EmbeddingsDb>,
    pub ollama: OllamaClient,
    pub selected_model: String,

    // Cached navigation data
    pub cached_volumes: Vec<String>,
    pub cached_books: Vec<String>,
    pub cached_chapters: Vec<i32>,
    pub cached_verses: Vec<Scripture>,
}

impl App {
    pub async fn new() -> anyhow::Result<Self> {
        let mut scripture_db = ScriptureDb::new();
        scripture_db.load_from_json("lds-scriptures-2020.12.08/json/lds-scriptures-json.txt").await?;

        let ollama = OllamaClient::new("http://localhost:11434");

        // Load config
        let config = Config::load().unwrap_or_else(|_| Config::new());

        // Load provider from config
        let current_provider = config.provider
            .as_ref()
            .and_then(|p| Provider::from_str(p))
            .unwrap_or(Provider::Ollama);

        // Initialize API clients - check env vars first, then config
        let claude_key = std::env::var("ANTHROPIC_API_KEY").ok()
            .or_else(|| config.claude_api_key.clone());
        let claude_client = claude_key.as_ref().map(|k| ClaudeClient::new(k));

        let openai_key = std::env::var("OPENAI_API_KEY").ok()
            .or_else(|| config.openai_api_key.clone());
        let openai_client = openai_key.as_ref().map(|k| OpenAIClient::new(k));

        // Load default model from config
        let selected_model = config.default_model
            .unwrap_or_else(|| "gemma3:latest".to_string());

        // Load embeddings if available (for semantic search)
        // Try local data/ directory first, then ~/.config/escrituras/data/
        let embeddings_db = {
            let local_path = std::path::Path::new("data");
            let config_path = dirs::config_dir()
                .map(|p| p.join("escrituras/data"));

            if local_path.join("scripture_embeddings.npy").exists() {
                EmbeddingsDb::load(local_path).ok()
            } else if let Some(ref cfg_path) = config_path {
                if cfg_path.join("scripture_embeddings.npy").exists() {
                    EmbeddingsDb::load(cfg_path).ok()
                } else {
                    None
                }
            } else {
                None
            }
        };

        let cached_volumes: Vec<String> = scripture_db.get_volumes().to_vec();

        let mut volume_state = ListState::default();
        volume_state.select(Some(0));

        Ok(Self {
            should_quit: false,
            screen: Screen::Browse,
            input_mode: InputMode::Normal,
            focus: FocusPane::Navigation,

            nav_level: NavLevel::Volume,
            volume_state,
            book_state: ListState::default(),
            chapter_state: ListState::default(),

            content_scroll: 0,
            content_height: 0,
            total_content_lines: 0,

            search_input: String::new(),
            search_results: Vec::new(),
            search_state: ListState::default(),
            search_focus: SearchFocus::default(),

            query_input: String::new(),
            query_cursor: 0,
            chat_messages: Vec::new(),
            query_loading: false,
            query_scroll: 0,
            query_chat_height: 0,
            query_chat_width: 0,
            query_task: None,
            extracted_references: Vec::new(),
            references_state: ListState::default(),

            navigation_stack: Vec::new(),
            selected_verse_idx: None,
            selected_range: None,

            session_context: Vec::new(),
            context_state: ListState::default(),
            show_context_panel: false,

            browsed_chapters: Vec::new(),

            animation_frame: 0,

            show_model_picker: false,
            available_models: Vec::new(),
            model_picker_state: ListState::default(),

            current_provider,
            claude_client,
            openai_client,
            show_provider_picker: false,
            provider_picker_state: ListState::default(),

            show_api_key_input: false,
            api_key_input: String::new(),
            api_key_input_cursor: 0,
            api_key_target_provider: None,

            nav_area: None,
            content_area: None,
            refs_area: None,

            scripture_db,
            embeddings_db,
            ollama,
            selected_model,

            cached_volumes,
            cached_books: Vec::new(),
            cached_chapters: Vec::new(),
            cached_verses: Vec::new(),
        })
    }

    // Navigation helpers
    pub fn selected_volume(&self) -> Option<&String> {
        self.volume_state.selected().and_then(|i| self.cached_volumes.get(i))
    }

    pub fn selected_book(&self) -> Option<&String> {
        self.book_state.selected().and_then(|i| self.cached_books.get(i))
    }

    pub fn selected_chapter(&self) -> Option<i32> {
        self.chapter_state.selected().and_then(|i| self.cached_chapters.get(i).copied())
    }

    // Navigation actions
    pub fn nav_down(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                let len = self.cached_volumes.len();
                if len > 0 {
                    let i = self.volume_state.selected().unwrap_or(0);
                    self.volume_state.select(Some((i + 1).min(len - 1)));
                }
            }
            NavLevel::Book => {
                let len = self.cached_books.len();
                if len > 0 {
                    let i = self.book_state.selected().unwrap_or(0);
                    self.book_state.select(Some((i + 1).min(len - 1)));
                }
            }
            NavLevel::Chapter => {
                let len = self.cached_chapters.len();
                if len > 0 {
                    let i = self.chapter_state.selected().unwrap_or(0);
                    let new_index = (i + 1).min(len - 1);
                    self.chapter_state.select(Some(new_index));
                    self.load_verses();
                }
            }
        }
    }

    pub fn nav_up(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                let i = self.volume_state.selected().unwrap_or(0);
                self.volume_state.select(Some(i.saturating_sub(1)));
            }
            NavLevel::Book => {
                let i = self.book_state.selected().unwrap_or(0);
                self.book_state.select(Some(i.saturating_sub(1)));
            }
            NavLevel::Chapter => {
                let i = self.chapter_state.selected().unwrap_or(0);
                self.chapter_state.select(Some(i.saturating_sub(1)));
                self.load_verses();
            }
        }
    }

    pub fn nav_enter(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                if let Some(volume) = self.selected_volume().cloned() {
                    self.cached_books = self.scripture_db.get_books_for_volume(&volume);
                    if !self.cached_books.is_empty() {
                        self.book_state.select(Some(0));
                        self.nav_level = NavLevel::Book;
                    }
                }
            }
            NavLevel::Book => {
                if let Some(book) = self.selected_book().cloned() {
                    self.cached_chapters = self.scripture_db.get_chapters_for_book(&book);
                    if !self.cached_chapters.is_empty() {
                        self.chapter_state.select(Some(0));
                        self.nav_level = NavLevel::Chapter;
                        self.load_verses();
                    }
                }
            }
            NavLevel::Chapter => {
                // At chapter level, Enter focuses the content pane
                self.focus = FocusPane::Content;
            }
        }
    }

    pub fn nav_back(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                // Already at top, do nothing
            }
            NavLevel::Book => {
                self.nav_level = NavLevel::Volume;
                self.cached_books.clear();
                self.book_state.select(None);
            }
            NavLevel::Chapter => {
                self.nav_level = NavLevel::Book;
                self.cached_chapters.clear();
                self.cached_verses.clear();
                self.chapter_state.select(None);
                self.content_scroll = 0;
            }
        }
    }

    pub fn nav_first(&mut self) {
        match self.nav_level {
            NavLevel::Volume => self.volume_state.select(Some(0)),
            NavLevel::Book => self.book_state.select(Some(0)),
            NavLevel::Chapter => {
                self.chapter_state.select(Some(0));
                self.load_verses();
            }
        }
    }

    pub fn nav_last(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                let len = self.cached_volumes.len();
                if len > 0 {
                    self.volume_state.select(Some(len - 1));
                }
            }
            NavLevel::Book => {
                let len = self.cached_books.len();
                if len > 0 {
                    self.book_state.select(Some(len - 1));
                }
            }
            NavLevel::Chapter => {
                let len = self.cached_chapters.len();
                if len > 0 {
                    self.chapter_state.select(Some(len - 1));
                    self.load_verses();
                }
            }
        }
    }

    fn load_verses(&mut self) {
        if let (Some(book), Some(chapter)) = (self.selected_book().cloned(), self.selected_chapter()) {
            let verses = self.scripture_db.get_verses_for_chapter(&book, chapter);
            self.cached_verses = verses.into_iter().cloned().collect();
            self.content_scroll = 0;

            // Track browsed chapter (lightweight, not individual verses)
            if !self.browsed_chapters.iter().any(|(b, c)| b == &book && *c == chapter) {
                self.browsed_chapters.push((book, chapter));
            }
        }
    }

    // Content scrolling
    pub fn scroll_down(&mut self) {
        if self.content_scroll < self.total_content_lines.saturating_sub(self.content_height) {
            self.content_scroll = self.content_scroll.saturating_add(1);
        }
    }

    pub fn scroll_up(&mut self) {
        self.content_scroll = self.content_scroll.saturating_sub(1);
    }

    pub fn scroll_half_page_down(&mut self) {
        let half_page = self.content_height / 2;
        let max_scroll = self.total_content_lines.saturating_sub(self.content_height);
        self.content_scroll = (self.content_scroll + half_page).min(max_scroll);
    }

    pub fn scroll_half_page_up(&mut self) {
        let half_page = self.content_height / 2;
        self.content_scroll = self.content_scroll.saturating_sub(half_page);
    }

    // Search - combines semantic (if available) and keyword results
    pub fn perform_search(&mut self) {
        if self.search_input.is_empty() {
            return;
        }

        let query = &self.search_input.clone();
        let limit = 50;
        let semantic_limit = 20; // Show up to 20 semantic results first
        let mut combined_results: Vec<Scripture> = Vec::new();
        let mut seen_titles: HashSet<String> = HashSet::new();

        // Try semantic search if embeddings are available (uses local ONNX model)
        if let Some(embeddings) = &mut self.embeddings_db {
            // Search embeddings for semantically similar verses (embeds query locally)
            if let Ok(semantic_matches) = embeddings.search(query, semantic_limit) {
                // Convert to Scripture objects
                for (verse_title, _score) in semantic_matches {
                    if let Some(scripture) = self.scripture_db.get_by_title(&verse_title) {
                        seen_titles.insert(verse_title);
                        combined_results.push(scripture.clone());
                    }
                }
            }
        }

        // Add keyword search results (deduped)
        let keyword_results = self.scripture_db.search(query, limit);
        for scripture in keyword_results {
            if !seen_titles.contains(&scripture.verse_title) {
                seen_titles.insert(scripture.verse_title.clone());
                combined_results.push(scripture.clone());
                if combined_results.len() >= limit {
                    break;
                }
            }
        }

        self.search_results = combined_results;
        if !self.search_results.is_empty() {
            self.search_state.select(Some(0));
        }
    }

    pub fn search_nav_down(&mut self) {
        let len = self.search_results.len();
        if len > 0 {
            let i = self.search_state.selected().unwrap_or(0);
            self.search_state.select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn search_nav_up(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        self.search_state.select(Some(i.saturating_sub(1)));
    }

    // Title helpers
    pub fn current_nav_title(&self) -> String {
        match self.nav_level {
            NavLevel::Volume => "Volumes".to_string(),
            NavLevel::Book => self.selected_volume().cloned().unwrap_or_default(),
            NavLevel::Chapter => self.selected_book().cloned().unwrap_or_default(),
        }
    }

    pub fn content_title(&self) -> String {
        if let (Some(book), Some(chapter)) = (self.selected_book(), self.selected_chapter()) {
            format!("{} {}", book, chapter)
        } else {
            "Select a chapter".to_string()
        }
    }

    pub fn session_context_count(&self) -> usize {
        self.session_context.len()
    }

    /// Save current navigation state to stack (before jumping to a reference)
    pub fn push_navigation_state(&mut self) {
        let state = NavigationState {
            volume_idx: self.volume_state.selected(),
            book_idx: self.book_state.selected(),
            chapter_idx: self.chapter_state.selected(),
            scroll: self.content_scroll,
        };
        self.navigation_stack.push(state);
    }

    /// Restore previous navigation state from stack
    pub fn pop_navigation_state(&mut self) -> bool {
        if let Some(state) = self.navigation_stack.pop() {
            // Restore volume selection
            if let Some(vol_idx) = state.volume_idx {
                self.volume_state.select(Some(vol_idx));
                if let Some(volume) = self.cached_volumes.get(vol_idx) {
                    self.cached_books = self.scripture_db.get_books_for_volume(volume);
                }
            }

            // Restore book selection
            if let Some(book_idx) = state.book_idx {
                self.book_state.select(Some(book_idx));
                if let Some(book) = self.cached_books.get(book_idx) {
                    self.cached_chapters = self.scripture_db.get_chapters_for_book(book);
                }
            }

            // Restore chapter selection and load verses
            if let Some(ch_idx) = state.chapter_idx {
                self.chapter_state.select(Some(ch_idx));
                if let (Some(book), Some(&chapter)) = (
                    self.book_state.selected().and_then(|i| self.cached_books.get(i)),
                    self.cached_chapters.get(ch_idx)
                ) {
                    let verses = self.scripture_db.get_verses_for_chapter(book, chapter);
                    self.cached_verses = verses.into_iter().cloned().collect();
                }
            }

            self.content_scroll = state.scroll;
            self.nav_level = NavLevel::Chapter;
            true
        } else {
            false
        }
    }

    /// Jump to a specific scripture range
    pub fn jump_to_scripture_range(&mut self, range: &ScriptureRange) {
        // Find the volume for this book
        for (vol_idx, volume) in self.cached_volumes.iter().enumerate() {
            let books = self.scripture_db.get_books_for_volume(volume);
            if let Some(book_idx) = books.iter().position(|b| *b == range.book_title) {
                // Set volume
                self.volume_state.select(Some(vol_idx));
                self.cached_books = books;

                // Set book
                self.book_state.select(Some(book_idx));
                let chapters = self.scripture_db.get_chapters_for_book(&range.book_title);
                self.cached_chapters = chapters;

                // Set chapter
                if let Some(ch_idx) = self.cached_chapters.iter().position(|&c| c == range.chapter_number) {
                    self.chapter_state.select(Some(ch_idx));

                    // Load verses
                    let verses = self.scripture_db.get_verses_for_chapter(
                        &range.book_title,
                        range.chapter_number,
                    );
                    self.cached_verses = verses.into_iter().cloned().collect();

                    // Store the range for highlighting multiple verses
                    self.selected_range = Some(range.clone());

                    // Find and select the first verse of the range, then scroll to it
                    for (verse_idx, verse) in self.cached_verses.iter().enumerate() {
                        if verse.verse_number == range.start_verse {
                            self.selected_verse_idx = Some(verse_idx);
                            self.scroll_to_selected_verse();
                            break;
                        }
                    }
                }

                self.nav_level = NavLevel::Chapter;
                break;
            }
        }
    }

    /// Jump to a single scripture (for search results)
    pub fn jump_to_scripture(&mut self, scripture: &Scripture) {
        // Clear any range selection
        self.selected_range = None;

        // Create a single-verse range and use the range function
        let range = ScriptureRange {
            book_title: scripture.book_title.clone(),
            book_short_title: scripture.book_short_title.clone(),
            chapter_number: scripture.chapter_number,
            start_verse: scripture.verse_number,
            end_verse: scripture.verse_number,
        };
        self.jump_to_scripture_range(&range);
    }

    /// Navigate references list
    pub fn references_nav_down(&mut self) {
        let len = self.extracted_references.len();
        if len > 0 {
            let i = self.references_state.selected().unwrap_or(0);
            self.references_state.select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn references_nav_up(&mut self) {
        let i = self.references_state.selected().unwrap_or(0);
        self.references_state.select(Some(i.saturating_sub(1)));
    }

    // Verse selection methods
    pub fn select_next_verse(&mut self) {
        let len = self.cached_verses.len();
        if len > 0 {
            let current = self.selected_verse_idx.unwrap_or(0);
            self.selected_verse_idx = Some((current + 1).min(len - 1));
            self.scroll_to_selected_verse();
        }
    }

    pub fn select_prev_verse(&mut self) {
        if let Some(current) = self.selected_verse_idx {
            self.selected_verse_idx = Some(current.saturating_sub(1));
            self.scroll_to_selected_verse();
        } else if !self.cached_verses.is_empty() {
            self.selected_verse_idx = Some(0);
        }
    }

    /// Clear the selected range (called when leaving AI mode or jumping to different reference)
    pub fn clear_selected_range(&mut self) {
        self.selected_range = None;
    }

    /// Tick animation frame (called by Tick event)
    pub fn tick_animation(&mut self) {
        if self.query_loading {
            self.animation_frame = (self.animation_frame + 1) % 3;
        }
    }

    /// Scroll chat to bottom so "Thinking..." is visible
    pub fn scroll_query_to_bottom(&mut self) {
        // Use actual chat width for wrap calculation, default to 50 if not set
        let wrap_width = if self.query_chat_width > 0 {
            self.query_chat_width as usize
        } else {
            50
        };

        let mut total_lines: u16 = 0;

        for msg in &self.chat_messages {
            total_lines += 1; // Role line ("You:" or "AI:")
            // Calculate wrapped lines for each line of content
            for line in msg.content.lines() {
                // Use character count, not byte length, for proper UTF-8 handling
                let char_count = line.chars().count();
                if char_count == 0 {
                    total_lines += 1; // Empty line still takes one line
                } else {
                    total_lines += ((char_count / wrap_width) + 1) as u16;
                }
            }
            total_lines += 1; // Blank line after message
        }

        // Add lines for "Thinking..." indicator
        total_lines += 2; // "AI:" + "Thinking..."

        let visible_height = if self.query_chat_height > 0 {
            self.query_chat_height
        } else {
            20
        };

        if total_lines > visible_height {
            self.query_scroll = total_lines.saturating_sub(visible_height);
        }
    }

    // Context panel navigation methods
    pub fn context_nav_down(&mut self) {
        let len = self.session_context.len();
        if len > 0 {
            let i = self.context_state.selected().unwrap_or(0);
            self.context_state.select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn context_nav_up(&mut self) {
        let i = self.context_state.selected().unwrap_or(0);
        self.context_state.select(Some(i.saturating_sub(1)));
    }

    pub fn remove_selected_context(&mut self) {
        if let Some(i) = self.context_state.selected() {
            if i < self.session_context.len() {
                self.session_context.remove(i);
                // Adjust selection
                if self.session_context.is_empty() {
                    self.context_state.select(None);
                } else if i >= self.session_context.len() {
                    self.context_state.select(Some(self.session_context.len() - 1));
                }
            }
        }
    }

    pub fn get_selected_verse(&self) -> Option<&Scripture> {
        self.selected_verse_idx.and_then(|idx| self.cached_verses.get(idx))
    }

    // Model picker methods
    pub fn model_picker_nav_down(&mut self) {
        let len = self.available_models.len();
        if len > 0 {
            let i = self.model_picker_state.selected().unwrap_or(0);
            self.model_picker_state.select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn model_picker_nav_up(&mut self) {
        let i = self.model_picker_state.selected().unwrap_or(0);
        self.model_picker_state.select(Some(i.saturating_sub(1)));
    }

    pub fn select_model(&mut self) {
        if let Some(i) = self.model_picker_state.selected() {
            if let Some(model) = self.available_models.get(i) {
                self.selected_model = model.clone();
                self.show_model_picker = false;
                // Save to config
                let _ = crate::config::Config::save_default_model(&self.selected_model);
            }
        }
    }

    // Provider picker methods
    pub fn provider_picker_nav_down(&mut self) {
        let providers = Provider::all();
        let len = providers.len();
        if len > 0 {
            let i = self.provider_picker_state.selected().unwrap_or(0);
            self.provider_picker_state.select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn provider_picker_nav_up(&mut self) {
        let i = self.provider_picker_state.selected().unwrap_or(0);
        self.provider_picker_state.select(Some(i.saturating_sub(1)));
    }

    pub fn get_models_for_provider(&self, provider: Provider) -> Vec<String> {
        match provider {
            Provider::Ollama => Vec::new(), // Will be fetched async
            Provider::Claude => ClaudeClient::list_models(),
            Provider::OpenAI => OpenAIClient::list_models(),
        }
    }

    /// Returns the source of the API key for a provider: "env", "config", or None
    pub fn get_key_source(&self, provider: Provider) -> Option<&'static str> {
        match provider {
            Provider::Ollama => Some("local"),
            Provider::Claude => {
                if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                    Some("env")
                } else if self.claude_client.is_some() {
                    Some("config")
                } else {
                    None
                }
            }
            Provider::OpenAI => {
                if std::env::var("OPENAI_API_KEY").is_ok() {
                    Some("env")
                } else if self.openai_client.is_some() {
                    Some("config")
                } else {
                    None
                }
            }
        }
    }

    fn scroll_to_selected_verse(&mut self) {
        if let Some(idx) = self.selected_verse_idx {
            let wrap_width = 40usize;
            let mut verse_start_line = 0u16;
            #[allow(unused_assignments)]
            let mut verse_end_line = 0u16;

            for (i, verse) in self.cached_verses.iter().enumerate() {
                // Use character count, not byte length, for proper UTF-8 handling
                let text_lines = (verse.scripture_text.chars().count() / wrap_width + 1) as u16;
                verse_end_line = verse_start_line + text_lines;

                if i == idx {
                    // Check if verse is above visible area
                    if verse_start_line < self.content_scroll {
                        self.content_scroll = verse_start_line;
                    }
                    // Check if verse is below visible area
                    else if verse_end_line > self.content_scroll + self.content_height {
                        // Scroll so verse bottom is at viewport bottom
                        self.content_scroll = verse_end_line.saturating_sub(self.content_height);
                    }
                    // Otherwise verse is visible, don't scroll
                    break;
                }

                verse_start_line = verse_end_line + 1; // +1 for blank line between verses
            }
        }
    }
}
