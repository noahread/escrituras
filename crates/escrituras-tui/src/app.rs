use escrituras_core::{
    combined_search, Assistant, ChatMessage, Config, DataPaths, EmbeddingsDb, KeySource, Provider,
    Scripture, ScriptureDb, ScriptureRange, StudyStore,
};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Browse,
    Search,
    Query,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusSubMode {
    #[default]
    Reading,
    Memorize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemorizeMode {
    #[default]
    Progressive,
    Flashcard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlashcardPhase {
    #[default]
    Hidden, // Shows reference only, prompt to type or reveal
    Typing,   // User is typing their attempt (editing mode)
    Revealed, // Shows actual text with diff highlighting
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
    Input, // Query input box (AI mode only)
}

// ChatMessage and ChatRole are re-exported from escrituras_core

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFocus {
    #[default]
    Results,
    Preview,
    Input, // Search input field
}

/// Direction of last scroll movement (for verse positioning in view)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    #[default]
    Down,
    Up,
}

/// State for Focus Mode - immersive single-verse study
#[derive(Debug, Clone)]
pub struct FocusState {
    /// Current scripture being displayed
    pub current_verse: Scripture,
    /// Current sub-mode (reading or memorize)
    pub sub_mode: FocusSubMode,
    /// Memorization style
    pub memorize_mode: MemorizeMode,
    /// Memorization difficulty level (0-5)
    pub memorize_level: u8,
    /// Whether answer is revealed (flashcard mode) - deprecated, use flashcard_phase
    pub memorize_revealed: bool,
    /// All verses in the volume for navigation
    pub volume_verses: Vec<Scripture>,
    /// Current index into volume_verses
    pub current_index: usize,
    /// Screen we came from (for returning)
    pub previous_screen: Screen,
    /// Flashcard phase (hidden, typing, revealed)
    pub flashcard_phase: FlashcardPhase,
    /// User's typed attempt in flashcard mode
    pub flashcard_input: String,
    /// Cursor position in flashcard input
    pub flashcard_input_cursor: usize,
}

/// Saved navigation state for returning to previous location
#[derive(Debug, Clone)]
pub struct NavigationState {
    pub volume_idx: Option<usize>,
    pub book_idx: Option<usize>,
    pub chapter_idx: Option<usize>,
    pub line_scroll: usize,
    pub verse_line_offset: usize,
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
    // Manual scroll tracking (bypasses ratatui's internal scroll logic)
    pub volume_scroll: usize,
    pub book_scroll: usize,
    pub chapter_scroll: usize,
    // Stored visible heights for scroll management (updated during render)
    pub nav_visible_height: usize,
    pub search_visible_height: usize,
    pub refs_visible_height: usize,
    pub context_visible_height: usize,

    // Content state - line-based scrolling
    pub line_scroll: usize, // Which line is at top of view (line-based scroll)
    pub verse_line_offset: usize, // Sub-verse offset for verses taller than view
    pub last_scroll_direction: ScrollDirection, // For verse positioning (top vs bottom)
    pub content_height: u16, // Height of content area in lines
    pub content_width: usize, // Width of content area for text wrapping
    pub total_content_lines: u16, // Total lines in chapter (for reference)

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
    pub query_deltas: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    pub streaming_reply: String, // Reply received so far while query_loading
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
    pub context_state: ListState, // For navigating context list
    pub show_context_panel: bool, // Toggle between scripture and context view

    // Persistent study data (None if the database couldn't be opened, in
    // which case saved verses and notes only last for this session)
    pub study: Option<StudyStore>,
    pub notes: HashMap<String, String>, // verse_title -> note text

    // Note editor state
    pub show_note_input: bool,
    pub note_input: String,
    pub note_cursor: usize,
    pub note_target: Option<String>, // verse_title being edited

    // Browsed chapters (for AI context, lightweight tracking)
    pub browsed_chapters: Vec<(String, i32)>, // (book_title, chapter_number)

    // Animation state
    pub animation_frame: u8, // 0-2 for ellipsis animation

    // Model picker state
    pub show_model_picker: bool,
    pub available_models: Vec<String>,
    pub model_picker_state: ListState,

    // Provider state
    pub current_provider: Provider,
    pub assistant: Assistant,
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

    // Focus mode state
    pub focus_state: Option<FocusState>,

    // Data
    pub scripture_db: ScriptureDb,
    pub embeddings_db: Option<EmbeddingsDb>,
    pub selected_model: String,

    // Cached navigation data
    pub cached_volumes: Vec<String>,
    pub cached_books: Vec<String>,
    pub cached_chapters: Vec<i32>,
    pub cached_verses: Vec<Scripture>,
}

impl App {
    pub async fn new() -> anyhow::Result<Self> {
        let paths = DataPaths::discover()?;
        let scripture_db = paths.load_scriptures().await?;

        // Load config
        let config = Config::load().unwrap_or_else(|_| Config::new());

        // Load provider from config
        let current_provider = config
            .provider
            .as_ref()
            .and_then(|p| p.parse::<Provider>().ok())
            .unwrap_or(Provider::Ollama);

        // Initialize API clients - env vars first, then config
        let assistant = Assistant::from_config(&config);

        // Load default model from config
        let selected_model = config
            .default_model
            .unwrap_or_else(|| "gemma3:latest".to_string());

        // Load embeddings if available (for semantic search)
        let embeddings_db = paths.load_embeddings();

        // Saved verses, notes and the last chapter read come from the study database
        let study = StudyStore::open_default().ok();
        let session_context: Vec<Scripture> = study
            .as_ref()
            .and_then(|s| s.saved_verses().ok())
            .unwrap_or_default()
            .iter()
            .filter_map(|title| scripture_db.get_by_title(title).cloned())
            .collect();
        let notes: HashMap<String, String> = study
            .as_ref()
            .and_then(|s| s.notes().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|note| (note.verse_title, note.body))
            .collect();
        let last_position = study
            .as_ref()
            .and_then(|s| s.last_position().ok().flatten());

        let cached_volumes: Vec<String> = scripture_db.get_volumes().to_vec();

        let mut volume_state = ListState::default();
        volume_state.select(Some(0));

        let mut app = Self {
            should_quit: false,
            screen: Screen::Browse,
            input_mode: InputMode::Normal,
            focus: FocusPane::Navigation,

            nav_level: NavLevel::Volume,
            volume_state,
            book_state: ListState::default(),
            chapter_state: ListState::default(),
            volume_scroll: 0,
            book_scroll: 0,
            chapter_scroll: 0,
            nav_visible_height: 20, // Reasonable default, updated during render
            search_visible_height: 20,
            refs_visible_height: 10,
            context_visible_height: 10,

            line_scroll: 0,
            verse_line_offset: 0,
            last_scroll_direction: ScrollDirection::Down,
            content_height: 0,
            content_width: 80, // Default, updated during render
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
            query_deltas: None,
            streaming_reply: String::new(),
            extracted_references: Vec::new(),
            references_state: ListState::default(),

            navigation_stack: Vec::new(),
            selected_verse_idx: None,
            selected_range: None,

            session_context,
            context_state: ListState::default(),
            show_context_panel: false,

            study,
            notes,
            show_note_input: false,
            note_input: String::new(),
            note_cursor: 0,
            note_target: None,

            browsed_chapters: Vec::new(),

            animation_frame: 0,

            show_model_picker: false,
            available_models: Vec::new(),
            model_picker_state: ListState::default(),

            current_provider,
            assistant,
            show_provider_picker: false,
            provider_picker_state: ListState::default(),

            show_api_key_input: false,
            api_key_input: String::new(),
            api_key_input_cursor: 0,
            api_key_target_provider: None,

            nav_area: None,
            content_area: None,
            refs_area: None,

            focus_state: None,

            scripture_db,
            embeddings_db,
            selected_model,

            cached_volumes,
            cached_books: Vec::new(),
            cached_chapters: Vec::new(),
            cached_verses: Vec::new(),
        };

        // Resume where the user left off
        if let Some((book, chapter)) = last_position {
            app.open_chapter(&book, chapter);
        }

        Ok(app)
    }

    /// Show a chapter, selecting its volume, book and chapter in the
    /// navigation lists. Returns false if the chapter doesn't exist.
    pub fn open_chapter(&mut self, book: &str, chapter: i32) -> bool {
        let Some(volume) = self
            .scripture_db
            .get_volume_for_book(book)
            .map(str::to_string)
        else {
            return false;
        };
        let Some(volume_idx) = self.cached_volumes.iter().position(|v| *v == volume) else {
            return false;
        };
        let books = self.scripture_db.get_books_for_volume(&volume);
        let Some(book_idx) = books.iter().position(|b| b == book) else {
            return false;
        };
        let chapters = self.scripture_db.get_chapters_for_book(book);
        let Some(chapter_idx) = chapters.iter().position(|&c| c == chapter) else {
            return false;
        };

        if !self.load_verses_for(book, chapter) {
            return false;
        }
        self.volume_state.select(Some(volume_idx));
        self.cached_books = books;
        self.book_state.select(Some(book_idx));
        self.cached_chapters = chapters;
        self.chapter_state.select(Some(chapter_idx));
        self.nav_level = NavLevel::Chapter;
        true
    }

    /// Remember the chapter being read so the next session resumes there
    fn record_reading(&self, book: &str, chapter: i32) {
        if let Some(study) = &self.study {
            let _ = study.record_reading(book, chapter);
        }
    }

    /// Add a verse to the saved scriptures (ignored if already saved)
    pub fn save_verse(&mut self, verse: Scripture) {
        if self
            .session_context
            .iter()
            .any(|v| v.verse_title == verse.verse_title)
        {
            return;
        }
        if let Some(study) = &self.study {
            let _ = study.save_verse(&verse.verse_title);
        }
        self.session_context.push(verse);
    }

    /// Open the note editor for the selected verse, pre-filled with its note
    pub fn start_editing_note(&mut self) {
        if let Some(title) = self.get_selected_verse().map(|v| v.verse_title.clone()) {
            self.note_input = self.notes.get(&title).cloned().unwrap_or_default();
            self.note_cursor = self.note_input.chars().count();
            self.note_target = Some(title);
            self.show_note_input = true;
        }
    }

    /// Save the note being edited (blank text deletes the note) and close the editor
    pub fn finish_editing_note(&mut self) {
        if let Some(title) = self.note_target.take() {
            let body = self.note_input.trim().to_string();
            if let Some(study) = &self.study {
                let _ = study.set_note(&title, &body);
            }
            if body.is_empty() {
                self.notes.remove(&title);
            } else {
                self.notes.insert(title, body);
            }
        }
        self.cancel_editing_note();
    }

    /// Close the note editor without saving
    pub fn cancel_editing_note(&mut self) {
        self.show_note_input = false;
        self.note_input.clear();
        self.note_cursor = 0;
        self.note_target = None;
    }

    // Navigation helpers
    pub fn selected_volume(&self) -> Option<&String> {
        self.volume_state
            .selected()
            .and_then(|i| self.cached_volumes.get(i))
    }

    pub fn selected_book(&self) -> Option<&String> {
        self.book_state
            .selected()
            .and_then(|i| self.cached_books.get(i))
    }

    pub fn selected_chapter(&self) -> Option<i32> {
        self.chapter_state
            .selected()
            .and_then(|i| self.cached_chapters.get(i).copied())
    }

    /// Adjust a ListState's offset to ensure the selected item is visible
    /// (Used for lists that don't have manual scroll tracking)
    fn adjust_list_offset(state: &mut ListState, visible_height: usize) {
        let visible_height = visible_height.max(1);
        if let Some(selected) = state.selected() {
            let min_offset = selected.saturating_sub(visible_height - 1);
            let max_offset = selected;
            let current = state.offset();
            if current < min_offset {
                *state.offset_mut() = min_offset;
            } else if current > max_offset {
                *state.offset_mut() = max_offset;
            }
        }
    }

    // Navigation actions
    // Note: scroll adjustment happens in render, not here, because only render knows the actual visible height
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
                    self.chapter_state.select(Some((i + 1).min(len - 1)));
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
                        self.book_scroll = 0;

                        // For single-book volumes (like D&C), skip directly to chapters
                        if self.is_single_book_volume(&volume) {
                            // The book is auto-selected, now load chapters
                            self.cached_chapters = self.scripture_db.get_chapters_for_book(&volume);
                            if !self.cached_chapters.is_empty() {
                                self.chapter_state.select(Some(0));
                                self.chapter_scroll = 0;
                                self.nav_level = NavLevel::Chapter;
                                self.load_verses();
                            }
                        } else {
                            self.nav_level = NavLevel::Book;
                        }
                    }
                }
            }
            NavLevel::Book => {
                if let Some(book) = self.selected_book().cloned() {
                    self.cached_chapters = self.scripture_db.get_chapters_for_book(&book);
                    if !self.cached_chapters.is_empty() {
                        self.chapter_state.select(Some(0));
                        self.chapter_scroll = 0;
                        self.nav_level = NavLevel::Chapter;
                        self.load_verses();
                    }
                }
            }
            NavLevel::Chapter => {
                // At chapter level, Enter focuses the content pane
                self.ensure_verse_selected();
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
                self.book_scroll = 0;
            }
            NavLevel::Chapter => {
                // For single-book volumes, go back to Volume level (skip Book level)
                // Use selected_volume() since is_single_book_volume expects a volume name
                let is_single_book = self
                    .selected_volume()
                    .map(|v| self.is_single_book_volume(v))
                    .unwrap_or(false);

                if is_single_book {
                    self.nav_level = NavLevel::Volume;
                    self.cached_books.clear();
                    self.book_state.select(None);
                    self.book_scroll = 0;
                } else {
                    self.nav_level = NavLevel::Book;
                }
                self.cached_chapters.clear();
                self.cached_verses.clear();
                self.chapter_state.select(None);
                self.chapter_scroll = 0;
                self.line_scroll = 0;
                self.verse_line_offset = 0;
            }
        }
    }

    pub fn nav_first(&mut self) {
        match self.nav_level {
            NavLevel::Volume => {
                self.volume_state.select(Some(0));
                self.volume_scroll = 0;
            }
            NavLevel::Book => {
                self.book_state.select(Some(0));
                self.book_scroll = 0;
            }
            NavLevel::Chapter => {
                self.chapter_state.select(Some(0));
                self.chapter_scroll = 0;
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

    /// Load verses for the current book/chapter selection.
    /// Returns true if verses were successfully loaded, false otherwise.
    fn load_verses(&mut self) -> bool {
        // Always reset scroll state when attempting to load verses
        self.line_scroll = 0;
        self.verse_line_offset = 0;
        self.last_scroll_direction = ScrollDirection::Down;

        if let (Some(book), Some(chapter)) =
            (self.selected_book().cloned(), self.selected_chapter())
        {
            let verses = self.scripture_db.get_verses_for_chapter(&book, chapter);
            self.cached_verses = verses.into_iter().cloned().collect();
            // Reset selected verse
            self.selected_verse_idx = if self.cached_verses.is_empty() {
                None
            } else {
                Some(0)
            };

            self.record_reading(&book, chapter);

            // Track browsed chapter (lightweight, not individual verses)
            if !self
                .browsed_chapters
                .iter()
                .any(|(b, c)| b == &book && *c == chapter)
            {
                self.browsed_chapters.push((book, chapter));
            }

            !self.cached_verses.is_empty()
        } else {
            false
        }
    }

    /// Load verses for a specific book and chapter.
    /// Returns true if verses were successfully loaded, false otherwise.
    /// Does NOT modify navigation state - caller should update state after success.
    fn load_verses_for(&mut self, book: &str, chapter: i32) -> bool {
        let verses = self.scripture_db.get_verses_for_chapter(book, chapter);
        if verses.is_empty() {
            return false;
        }

        // Success - update verse state atomically
        self.cached_verses = verses.into_iter().cloned().collect();
        self.line_scroll = 0;
        self.verse_line_offset = 0;
        self.last_scroll_direction = ScrollDirection::Down;
        self.selected_verse_idx = Some(0);

        self.record_reading(book, chapter);

        // Track browsed chapter
        let book_owned = book.to_string();
        if !self
            .browsed_chapters
            .iter()
            .any(|(b, c)| b == &book_owned && *c == chapter)
        {
            self.browsed_chapters.push((book_owned, chapter));
        }

        true
    }

    // Content scrolling - now verse-based to match rendering
    pub fn scroll_down(&mut self) {
        // Move to next verse (used by mouse scroll)
        if let Some(idx) = self.selected_verse_idx {
            if idx < self.cached_verses.len().saturating_sub(1) {
                self.selected_verse_idx = Some(idx + 1);
            }
        }
    }

    pub fn scroll_up(&mut self) {
        // Move to previous verse (used by mouse scroll)
        if let Some(idx) = self.selected_verse_idx {
            if idx > 0 {
                self.selected_verse_idx = Some(idx - 1);
            }
        }
    }

    pub fn scroll_half_page_down(&mut self) {
        // Move down several verses (Ctrl+D)
        let jump = 5; // Jump 5 verses at a time
        if let Some(idx) = self.selected_verse_idx {
            let max_idx = self.cached_verses.len().saturating_sub(1);
            self.selected_verse_idx = Some((idx + jump).min(max_idx));
        }
    }

    pub fn scroll_half_page_up(&mut self) {
        // Move up several verses (Ctrl+U)
        let jump = 5; // Jump 5 verses at a time
        if let Some(idx) = self.selected_verse_idx {
            self.selected_verse_idx = Some(idx.saturating_sub(jump));
        }
    }

    // Search - combines semantic (if available) and keyword results
    pub fn perform_search(&mut self) {
        if self.search_input.is_empty() {
            return;
        }

        // Up to 20 semantic results first, 50 results in total
        let combined_results: Vec<Scripture> = combined_search(
            &self.scripture_db,
            self.embeddings_db.as_ref(),
            &self.search_input,
            20,
            50,
        )
        .into_iter()
        .map(|hit| hit.scripture.clone())
        .collect();

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
            Self::adjust_list_offset(&mut self.search_state, self.search_visible_height);
        }
    }

    pub fn search_nav_up(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        self.search_state.select(Some(i.saturating_sub(1)));
        Self::adjust_list_offset(&mut self.search_state, self.search_visible_height);
    }

    // Title helpers
    pub fn current_nav_title(&self) -> String {
        match self.nav_level {
            NavLevel::Volume => "Volumes".to_string(),
            NavLevel::Book => self.selected_volume().cloned().unwrap_or_default(),
            NavLevel::Chapter => {
                // For single-book volumes, show "Select a section" for D&C
                if let Some(book) = self.selected_book() {
                    if book == "Doctrine and Covenants" {
                        return "Select a section".to_string();
                    }
                }
                "Select a chapter".to_string()
            }
        }
    }

    /// Check if a volume has only one book with the same name (like D&C)
    pub fn is_single_book_volume(&self, volume: &str) -> bool {
        let books = self.scripture_db.get_books_for_volume(volume);
        books.len() == 1 && books.first().map(|b| b == volume).unwrap_or(false)
    }

    /// Get the label for a chapter (returns "Section X" for D&C, "Chapter X" for others)
    pub fn get_chapter_label(&self, chapter: i32) -> String {
        if let Some(book) = self.selected_book() {
            if book == "Doctrine and Covenants" {
                return format!("Section {}", chapter);
            }
        }
        format!("Chapter {}", chapter)
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
            line_scroll: self.line_scroll,
            verse_line_offset: self.verse_line_offset,
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
                    self.book_state
                        .selected()
                        .and_then(|i| self.cached_books.get(i)),
                    self.cached_chapters.get(ch_idx),
                ) {
                    let verses = self.scripture_db.get_verses_for_chapter(book, chapter);
                    self.cached_verses = verses.into_iter().cloned().collect();
                    self.record_reading(book, chapter);
                }
            }

            self.line_scroll = state.line_scroll;
            self.verse_line_offset = state.verse_line_offset;
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
                if let Some(ch_idx) = self
                    .cached_chapters
                    .iter()
                    .position(|&c| c == range.chapter_number)
                {
                    self.chapter_state.select(Some(ch_idx));

                    // Load verses
                    let verses = self
                        .scripture_db
                        .get_verses_for_chapter(&range.book_title, range.chapter_number);
                    self.cached_verses = verses.into_iter().cloned().collect();
                    self.record_reading(&range.book_title, range.chapter_number);

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
            Self::adjust_list_offset(&mut self.references_state, self.refs_visible_height);
        }
    }

    pub fn references_nav_up(&mut self) {
        let i = self.references_state.selected().unwrap_or(0);
        self.references_state.select(Some(i.saturating_sub(1)));
        Self::adjust_list_offset(&mut self.references_state, self.refs_visible_height);
    }

    // Verse selection methods

    /// Ensure a verse is selected when focusing on content pane
    /// Selects the topmost visible verse if nothing is selected or if selection is out of bounds
    pub fn ensure_verse_selected(&mut self) {
        if self.cached_verses.is_empty() {
            self.selected_verse_idx = None;
            return;
        }

        let max_idx = self.cached_verses.len() - 1;

        // Check if selection is missing or out of bounds
        let needs_reset = self
            .selected_verse_idx
            .map(|idx| idx > max_idx)
            .unwrap_or(true);

        if needs_reset {
            // Select the first verse if selection is invalid
            self.selected_verse_idx = Some(0);
            self.line_scroll = 0;
            self.verse_line_offset = 0;
        }
    }

    pub fn select_next_verse(&mut self) {
        let len = self.cached_verses.len();
        if len == 0 {
            return;
        }

        let current = self.selected_verse_idx.unwrap_or(0);

        // Set direction for scroll positioning (lock to bottom when going down)
        self.last_scroll_direction = ScrollDirection::Down;

        // If not at the last verse of the chapter, just move to next verse
        if current < len - 1 {
            self.selected_verse_idx = Some(current + 1);
            self.verse_line_offset = 0; // Reset sub-verse offset for new verse
            return;
        }

        // At the last verse - try to navigate to next chapter
        if self.navigate_to_next_chapter() {
            self.selected_verse_idx = Some(0);
            self.verse_line_offset = 0;
            self.line_scroll = 0;
        }
        // If at volume boundary, stay at last verse (do nothing)
    }

    pub fn select_prev_verse(&mut self) {
        if self.cached_verses.is_empty() {
            return;
        }

        let current = self.selected_verse_idx.unwrap_or(0);

        // Set direction for scroll positioning (lock to top when going up)
        self.last_scroll_direction = ScrollDirection::Up;

        // If not at the first verse of the chapter, just move to previous verse
        if current > 0 {
            self.selected_verse_idx = Some(current - 1);
            self.verse_line_offset = 0; // Reset sub-verse offset for new verse
            return;
        }

        // At the first verse - try to navigate to previous chapter
        if self.navigate_to_prev_chapter() {
            // Select the last verse of the previous chapter
            let last_idx = self.cached_verses.len().saturating_sub(1);
            self.selected_verse_idx = Some(last_idx);
            self.verse_line_offset = 0;
        }
        // If at volume boundary, stay at first verse (do nothing)
    }

    /// Navigate to the next chapter within the current volume.
    /// Returns true if navigation was successful, false if at volume boundary.
    fn navigate_to_next_chapter(&mut self) -> bool {
        self.navigate_to_adjacent_chapter(true)
    }

    /// Navigate to the previous chapter within the current volume.
    /// Returns true if navigation was successful, false if at volume boundary.
    fn navigate_to_prev_chapter(&mut self) -> bool {
        self.navigate_to_adjacent_chapter(false)
    }

    /// Move to the next or previous chapter, crossing into the adjacent book
    /// of the same volume when needed. Navigation state is only updated after
    /// the new chapter's verses load successfully.
    fn navigate_to_adjacent_chapter(&mut self, forward: bool) -> bool {
        let Some(book) = self.selected_book().cloned() else {
            return false;
        };

        // ALWAYS refresh cached_chapters from the database to ensure consistency
        // This fixes issues where initial navigation left stale state
        self.cached_chapters = self.scripture_db.get_chapters_for_book(&book);
        let Some(chapter) = self.selected_chapter() else {
            return false;
        };

        let target = if forward {
            self.scripture_db.next_chapter(&book, chapter)
        } else {
            self.scripture_db.previous_chapter(&book, chapter)
        };
        let Some((new_book, new_chapter)) = target else {
            return false;
        };

        if !self.load_verses_for(&new_book, new_chapter) {
            return false;
        }

        let changed_book = new_book != book;
        if changed_book {
            if let Some(volume) = self.selected_volume().cloned() {
                self.cached_books = self.scripture_db.get_books_for_volume(&volume);
            }
            let book_idx = self.cached_books.iter().position(|b| *b == new_book);
            self.book_state.select(book_idx);
            self.cached_chapters = self.scripture_db.get_chapters_for_book(&new_book);
        }

        let chapter_idx = self
            .cached_chapters
            .iter()
            .position(|&c| c == new_chapter)
            .unwrap_or(0);
        self.chapter_state.select(Some(chapter_idx));
        if changed_book {
            self.chapter_scroll = chapter_idx;
        }
        true
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

    /// Append streamed pieces of the AI reply. Returns true if any arrived.
    pub fn receive_reply_deltas(&mut self) -> bool {
        let Some(deltas) = &mut self.query_deltas else {
            return false;
        };
        let mut received = false;
        while let Ok(delta) = deltas.try_recv() {
            self.streaming_reply.push_str(&delta);
            received = true;
        }
        if received {
            self.scroll_query_to_bottom();
        }
        received
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

        // Add lines for the reply in progress, or the "Thinking..." indicator
        total_lines += 1; // "AI:"
        if self.streaming_reply.is_empty() {
            total_lines += 1; // "Thinking..."
        } else {
            for line in self.streaming_reply.lines() {
                let char_count = line.chars().count();
                total_lines += ((char_count / wrap_width) + 1) as u16;
            }
        }

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
            Self::adjust_list_offset(&mut self.context_state, self.context_visible_height);
        }
    }

    pub fn context_nav_up(&mut self) {
        let i = self.context_state.selected().unwrap_or(0);
        self.context_state.select(Some(i.saturating_sub(1)));
        Self::adjust_list_offset(&mut self.context_state, self.context_visible_height);
    }

    pub fn remove_selected_context(&mut self) {
        if let Some(i) = self.context_state.selected() {
            if i < self.session_context.len() {
                let removed = self.session_context.remove(i);
                if let Some(study) = &self.study {
                    let _ = study.remove_saved_verse(&removed.verse_title);
                }
                // Adjust selection
                if self.session_context.is_empty() {
                    self.context_state.select(None);
                } else if i >= self.session_context.len() {
                    self.context_state
                        .select(Some(self.session_context.len() - 1));
                }
            }
        }
    }

    pub fn get_selected_verse(&self) -> Option<&Scripture> {
        self.selected_verse_idx
            .and_then(|idx| self.cached_verses.get(idx))
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
                let _ = Config::save_default_model(&self.selected_model);
            }
        }
    }

    // Provider picker methods
    pub fn provider_picker_nav_down(&mut self) {
        let providers = Provider::all();
        let len = providers.len();
        if len > 0 {
            let i = self.provider_picker_state.selected().unwrap_or(0);
            self.provider_picker_state
                .select(Some((i + 1).min(len - 1)));
        }
    }

    pub fn provider_picker_nav_up(&mut self) {
        let i = self.provider_picker_state.selected().unwrap_or(0);
        self.provider_picker_state.select(Some(i.saturating_sub(1)));
    }

    /// Where the API key for a provider comes from, or None if it needs one
    pub fn get_key_source(&self, provider: Provider) -> Option<KeySource> {
        self.assistant.key_source(provider)
    }

    /// Scroll adjustment is now handled in render_content() based on line_scroll
    /// This function is kept for API compatibility but does nothing
    fn scroll_to_selected_verse(&mut self) {
        // No-op: scroll positioning is now done in render_content()
        // based on selected_verse_idx, last_scroll_direction, and verse_line_offset
    }

    // Focus Mode methods

    /// Enter Focus Mode from the current selected verse
    pub fn enter_focus_mode(&mut self) {
        // Save the current screen before switching
        let previous_screen = self.screen;

        // Get the current verse based on which screen we're on
        let verse = match self.screen {
            Screen::Browse => self.get_selected_verse().cloned(),
            Screen::Search => self
                .search_state
                .selected()
                .and_then(|i| self.search_results.get(i).cloned()),
            Screen::Query => self.get_selected_verse().cloned(),
            Screen::Focus => None, // Already in focus mode
        };

        if let Some(verse) = verse {
            // Save navigation state for return
            self.push_navigation_state();

            // Build volume verse list for navigation
            let volume = verse.volume_title.clone();
            let volume_verses = self.scripture_db.get_all_verses_for_volume(&volume);

            // Find current index in the volume
            let current_index = volume_verses
                .iter()
                .position(|v| {
                    v.book_title == verse.book_title
                        && v.chapter_number == verse.chapter_number
                        && v.verse_number == verse.verse_number
                })
                .unwrap_or(0);

            self.focus_state = Some(FocusState {
                current_verse: verse,
                sub_mode: FocusSubMode::Reading,
                memorize_mode: MemorizeMode::Progressive,
                memorize_level: 0,
                memorize_revealed: false,
                volume_verses,
                current_index,
                previous_screen,
                flashcard_phase: FlashcardPhase::Hidden,
                flashcard_input: String::new(),
                flashcard_input_cursor: 0,
            });

            self.screen = Screen::Focus;
        }
    }

    /// Exit Focus Mode and return to previous screen
    pub fn exit_focus_mode(&mut self) {
        if let Some(state) = self.focus_state.take() {
            self.screen = state.previous_screen;
        }
        self.pop_navigation_state();
    }

    /// Navigate to next verse in Focus Mode
    pub fn focus_next_verse(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            // Use saturating_sub to avoid underflow when volume_verses is empty
            if state.current_index < state.volume_verses.len().saturating_sub(1) {
                state.current_index += 1;
                state.current_verse = state.volume_verses[state.current_index].clone();
                // Reset memorization state when navigating
                state.memorize_level = 0;
                state.memorize_revealed = false;
                state.flashcard_phase = FlashcardPhase::Hidden;
                state.flashcard_input.clear();
                state.flashcard_input_cursor = 0;
            }
            // At volume boundary - stay at last verse
        }
    }

    /// Navigate to previous verse in Focus Mode
    pub fn focus_prev_verse(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            if state.current_index > 0 {
                state.current_index -= 1;
                state.current_verse = state.volume_verses[state.current_index].clone();
                // Reset memorization state when navigating
                state.memorize_level = 0;
                state.memorize_revealed = false;
                state.flashcard_phase = FlashcardPhase::Hidden;
                state.flashcard_input.clear();
                state.flashcard_input_cursor = 0;
            }
            // At volume boundary - stay at first verse
        }
    }

    /// Toggle memorization mode on/off
    pub fn focus_toggle_memorize(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.sub_mode = match state.sub_mode {
                FocusSubMode::Reading => FocusSubMode::Memorize,
                FocusSubMode::Memorize => FocusSubMode::Reading,
            };
            // Reset memorization state
            state.memorize_level = 0;
            state.memorize_revealed = false;
        }
    }

    /// Cycle memorization mode (Progressive <-> Flashcard)
    pub fn focus_cycle_memorize_mode(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.memorize_mode = match state.memorize_mode {
                MemorizeMode::Progressive => MemorizeMode::Flashcard,
                MemorizeMode::Flashcard => MemorizeMode::Progressive,
            };
            state.memorize_level = 0;
            state.memorize_revealed = false;
            state.flashcard_phase = FlashcardPhase::Hidden;
            state.flashcard_input.clear();
            state.flashcard_input_cursor = 0;
        }
    }

    /// Increase memorization difficulty
    pub fn focus_increase_difficulty(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            if state.memorize_level < 5 {
                state.memorize_level += 1;
            }
        }
    }

    /// Decrease memorization difficulty
    pub fn focus_decrease_difficulty(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.memorize_level = state.memorize_level.saturating_sub(1);
        }
    }

    /// Reveal answer in flashcard mode
    pub fn focus_reveal_flashcard(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.memorize_revealed = true;
            state.flashcard_phase = FlashcardPhase::Revealed;
        }
    }

    /// Reset flashcard to hidden state
    pub fn focus_reset_flashcard(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.memorize_revealed = false;
            state.flashcard_phase = FlashcardPhase::Hidden;
            state.flashcard_input.clear();
            state.flashcard_input_cursor = 0;
        }
    }

    /// Start flashcard typing mode
    pub fn focus_start_typing(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.flashcard_phase = FlashcardPhase::Typing;
            state.flashcard_input.clear();
            state.flashcard_input_cursor = 0;
        }
    }

    /// Submit flashcard typing and reveal with diff
    pub fn focus_submit_typing(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.flashcard_phase = FlashcardPhase::Revealed;
            state.memorize_revealed = true;
        }
    }

    /// Cancel flashcard typing and return to hidden
    pub fn focus_cancel_typing(&mut self) {
        if let Some(ref mut state) = self.focus_state {
            state.flashcard_phase = FlashcardPhase::Hidden;
            state.flashcard_input.clear();
            state.flashcard_input_cursor = 0;
        }
    }

    /// Get the current verse in focus mode (for copy/save operations)
    pub fn get_focus_verse(&self) -> Option<&Scripture> {
        self.focus_state.as_ref().map(|s| &s.current_verse)
    }
}
