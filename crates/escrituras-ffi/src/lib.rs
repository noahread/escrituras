//! Bindings for escrituras-core used by native front-ends (the SwiftUI app).
//!
//! Everything goes through one [`Library`] object. Records are plain data,
//! so the generated Swift types are simple structs and enums. Network calls
//! (AI replies, Ollama model lists) are async; everything else is quick and
//! synchronous.

use escrituras_core::{
    build_study_prompt, combined_search, Assistant, ChatMessage, ChatRole, DataPaths, EmbeddingsDb,
    Provider, Scripture, ScriptureDb, ScriptureRange, StudyContext, StudyStore,
};
use std::path::PathBuf;
use std::sync::Arc;

uniffi::setup_scaffolding!();

// ---- Errors -----------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum EscriturasError {
    /// Scripture data couldn't be found or read
    #[error("{0}")]
    Data(String),
    /// The study database couldn't be opened or written
    #[error("{0}")]
    Storage(String),
    /// An AI provider request failed
    #[error("{0}")]
    Ai(String),
}

type Result<T> = std::result::Result<T, EscriturasError>;

fn data_err(e: impl std::fmt::Display) -> EscriturasError {
    EscriturasError::Data(e.to_string())
}

fn storage_err(e: impl std::fmt::Display) -> EscriturasError {
    EscriturasError::Storage(e.to_string())
}

// ---- Records ----------------------------------------------------------------

/// One verse of scripture
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Verse {
    /// e.g. "Alma 32:21"
    pub title: String,
    /// e.g. "Alma 32:21" or "D&C 4:2"
    pub short_title: String,
    pub volume: String,
    pub book: String,
    pub chapter: i32,
    pub number: i32,
    pub text: String,
}

impl From<&Scripture> for Verse {
    fn from(s: &Scripture) -> Self {
        Self {
            title: s.verse_title.clone(),
            short_title: s.verse_short_title.clone(),
            volume: s.volume_title.clone(),
            book: s.book_title.clone(),
            chapter: s.chapter_number,
            number: s.verse_number,
            text: s.scripture_text.clone(),
        }
    }
}

/// A chapter, e.g. Alma 32
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChapterRef {
    pub book: String,
    pub chapter: i32,
}

/// A verse or verse range found in text, e.g. "Mosiah 4:19-21"
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ScriptureRef {
    pub title: String,
    pub book: String,
    pub chapter: i32,
    pub start_verse: i32,
    pub end_verse: i32,
}

impl From<&ScriptureRange> for ScriptureRef {
    fn from(r: &ScriptureRange) -> Self {
        Self {
            title: r.display_title(),
            book: r.book_title.clone(),
            chapter: r.chapter_number,
            start_verse: r.start_verse,
            end_verse: r.end_verse,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SearchResult {
    pub verse: Verse,
    /// Similarity for semantic matches (higher is closer); `None` for keyword matches
    pub score: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VerseNote {
    pub verse_title: String,
    pub body: String,
    /// Unix timestamps (seconds)
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AiProvider {
    Ollama,
    Claude,
    OpenAi,
}

impl From<AiProvider> for Provider {
    fn from(p: AiProvider) -> Self {
        match p {
            AiProvider::Ollama => Provider::Ollama,
            AiProvider::Claude => Provider::Claude,
            AiProvider::OpenAi => Provider::OpenAI,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl From<Message> for ChatMessage {
    fn from(m: Message) -> Self {
        ChatMessage {
            role: match m.role {
                MessageRole::User => ChatRole::User,
                MessageRole::Assistant => ChatRole::Assistant,
            },
            content: m.content,
        }
    }
}

/// What the user is studying, sent with AI questions
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct StudyContextInput {
    /// The chapter or reference on screen, e.g. "Alma 32"
    pub current_reading: Option<String>,
    /// Chapters viewed this session, oldest first
    pub browsed_chapters: Vec<ChapterRef>,
    /// Include the user's saved verses as context
    pub include_saved_verses: bool,
}

/// Receives each piece of an AI reply as it streams in
#[uniffi::export(with_foreign)]
pub trait ReplyListener: Send + Sync {
    fn on_delta(&self, text: String);
}

// ---- Library ----------------------------------------------------------------

/// Scripture data, search, saved study data and AI chat
#[derive(uniffi::Object)]
pub struct Library {
    scriptures: ScriptureDb,
    embeddings: Option<EmbeddingsDb>,
    study: Option<StudyStore>,
    runtime: tokio::runtime::Runtime,
}

#[uniffi::export]
impl Library {
    /// Load scripture data from the first of `data_roots` that has it (each
    /// root contains `lds-scriptures-2020.12.08/` and optionally `data/`), and
    /// open the study database at `study_db_path` (`None` keeps study data in
    /// memory only).
    #[uniffi::constructor]
    pub fn open(data_roots: Vec<String>, study_db_path: Option<String>) -> Result<Arc<Self>> {
        let roots: Vec<PathBuf> = data_roots.into_iter().map(PathBuf::from).collect();
        let paths = DataPaths::find_in(&roots).map_err(data_err)?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(data_err)?;
        let scriptures = runtime
            .block_on(paths.load_scriptures())
            .map_err(data_err)?;
        let embeddings = paths.load_embeddings();

        let study = match study_db_path {
            Some(path) => StudyStore::open(PathBuf::from(path).as_path()),
            None => StudyStore::open_in_memory(),
        }
        .map_err(storage_err)?;

        Ok(Arc::new(Self {
            scriptures,
            embeddings,
            study: Some(study),
            runtime,
        }))
    }

    // ---- Browsing ----

    /// Volumes in canonical order, e.g. "Old Testament", "Book of Mormon"
    pub fn volumes(&self) -> Vec<String> {
        self.scriptures.get_volumes().to_vec()
    }

    pub fn books(&self, volume: String) -> Vec<String> {
        self.scriptures.get_books_for_volume(&volume)
    }

    pub fn chapters(&self, book: String) -> Vec<i32> {
        self.scriptures.get_chapters_for_book(&book)
    }

    pub fn verses(&self, book: String, chapter: i32) -> Vec<Verse> {
        self.scriptures
            .get_verses_for_chapter(&book, chapter)
            .into_iter()
            .map(Verse::from)
            .collect()
    }

    pub fn volume_for_book(&self, book: String) -> Option<String> {
        self.scriptures
            .get_volume_for_book(&book)
            .map(str::to_string)
    }

    /// The next chapter in canonical order within the same volume
    pub fn next_chapter(&self, book: String, chapter: i32) -> Option<ChapterRef> {
        self.scriptures
            .next_chapter(&book, chapter)
            .map(|(book, chapter)| ChapterRef { book, chapter })
    }

    /// The previous chapter in canonical order within the same volume
    pub fn previous_chapter(&self, book: String, chapter: i32) -> Option<ChapterRef> {
        self.scriptures
            .previous_chapter(&book, chapter)
            .map(|(book, chapter)| ChapterRef { book, chapter })
    }

    /// A verse by its full title, e.g. "Alma 32:21"
    pub fn verse(&self, title: String) -> Option<Verse> {
        self.scriptures.get_by_title(&title).map(Verse::from)
    }

    /// The verses for a reference such as "John 3:16" or "Mosiah 4:19-21"
    pub fn lookup(&self, reference: String) -> Vec<Verse> {
        let Some(range) = self
            .scriptures
            .extract_scripture_references(&reference)
            .into_iter()
            .next()
        else {
            return Vec::new();
        };
        self.scriptures
            .get_verses_for_chapter(&range.book_title, range.chapter_number)
            .into_iter()
            .filter(|v| range.contains_verse(v.verse_number))
            .map(Verse::from)
            .collect()
    }

    // ---- Search ----

    /// Search by meaning (when embeddings are installed) and by keyword
    pub fn search(&self, query: String, limit: u32) -> Vec<SearchResult> {
        let limit = limit as usize;
        let semantic_limit = (limit / 2).max(5).min(limit);
        combined_search(
            &self.scriptures,
            self.embeddings.as_ref(),
            &query,
            semantic_limit,
            limit,
        )
        .into_iter()
        .map(|hit| SearchResult {
            verse: Verse::from(hit.scripture),
            score: hit.score,
        })
        .collect()
    }

    /// Whether search by meaning is available
    pub fn has_semantic_search(&self) -> bool {
        self.embeddings.is_some()
    }

    /// Scripture references mentioned in text, e.g. an AI reply
    pub fn extract_references(&self, text: String) -> Vec<ScriptureRef> {
        self.scriptures
            .extract_scripture_references(&text)
            .iter()
            .map(ScriptureRef::from)
            .collect()
    }

    // ---- Saved verses, notes and reading history ----

    pub fn saved_verses(&self) -> Result<Vec<Verse>> {
        Ok(self
            .study()?
            .saved_verses()
            .map_err(storage_err)?
            .iter()
            .filter_map(|title| self.scriptures.get_by_title(title))
            .map(Verse::from)
            .collect())
    }

    /// Save a verse; returns false if it was already saved
    pub fn save_verse(&self, title: String) -> Result<bool> {
        self.study()?.save_verse(&title).map_err(storage_err)
    }

    pub fn remove_saved_verse(&self, title: String) -> Result<()> {
        self.study()?
            .remove_saved_verse(&title)
            .map_err(storage_err)
    }

    pub fn note(&self, verse_title: String) -> Result<Option<VerseNote>> {
        Ok(self
            .study()?
            .note(&verse_title)
            .map_err(storage_err)?
            .map(note_record))
    }

    /// All notes, most recently updated first
    pub fn notes(&self) -> Result<Vec<VerseNote>> {
        Ok(self
            .study()?
            .notes()
            .map_err(storage_err)?
            .into_iter()
            .map(note_record)
            .collect())
    }

    /// Set the note on a verse; blank text deletes it
    pub fn set_note(&self, verse_title: String, body: String) -> Result<()> {
        self.study()?
            .set_note(&verse_title, &body)
            .map_err(storage_err)
    }

    /// Record that a chapter was opened
    pub fn record_reading(&self, book: String, chapter: i32) -> Result<()> {
        self.study()?
            .record_reading(&book, chapter)
            .map_err(storage_err)
    }

    /// Chapters most recently opened first
    pub fn recent_chapters(&self, limit: u32) -> Result<Vec<ChapterRef>> {
        Ok(self
            .study()?
            .recent_chapters(limit as usize)
            .map_err(storage_err)?
            .into_iter()
            .map(|(book, chapter)| ChapterRef { book, chapter })
            .collect())
    }

    /// The chapter to resume reading at
    pub fn last_position(&self) -> Result<Option<ChapterRef>> {
        Ok(self
            .study()?
            .last_position()
            .map_err(storage_err)?
            .map(|(book, chapter)| ChapterRef { book, chapter }))
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl Library {
    /// Models offered for a provider. For Ollama this asks the local server
    /// and returns an empty list if it isn't running.
    pub async fn models(&self, provider: AiProvider) -> Vec<String> {
        let assistant = Assistant::with_keys(None, None);
        let provider = Provider::from(provider);
        self.runtime
            .spawn(async move { assistant.list_models(provider).await })
            .await
            .unwrap_or_default()
    }

    /// Ask a question about the scriptures. `history` is the conversation so
    /// far, ending with the question. Each piece of the reply is passed to
    /// `listener` as it streams in, and the full reply is returned.
    ///
    /// `api_key` is required for Claude and OpenAI and ignored for Ollama.
    #[allow(clippy::too_many_arguments)]
    pub async fn ask(
        &self,
        provider: AiProvider,
        model: String,
        api_key: Option<String>,
        history: Vec<Message>,
        context: StudyContextInput,
        listener: Arc<dyn ReplyListener>,
    ) -> Result<String> {
        let saved_verses: Vec<Scripture> = if context.include_saved_verses {
            self.study()?
                .saved_verses()
                .map_err(storage_err)?
                .iter()
                .filter_map(|title| self.scriptures.get_by_title(title).cloned())
                .collect()
        } else {
            Vec::new()
        };
        let browsed: Vec<(String, i32)> = context
            .browsed_chapters
            .into_iter()
            .map(|c| (c.book, c.chapter))
            .collect();
        let history: Vec<ChatMessage> = history.into_iter().map(ChatMessage::from).collect();
        let prompt = build_study_prompt(
            &history,
            &StudyContext {
                current_reading: context.current_reading.as_deref(),
                browsed_chapters: &browsed,
                saved_verses: &saved_verses,
            },
        );

        let provider = Provider::from(provider);
        let assistant = match provider {
            Provider::Claude => Assistant::with_keys(api_key.as_deref(), None),
            Provider::OpenAI => Assistant::with_keys(None, api_key.as_deref()),
            Provider::Ollama => Assistant::with_keys(None, None),
        };

        // Run on our own runtime so callers don't need a Tokio context
        self.runtime
            .spawn(async move {
                assistant
                    .reply(provider, &model, &prompt, &mut |delta| {
                        listener.on_delta(delta.to_string())
                    })
                    .await
            })
            .await
            .map_err(|e| EscriturasError::Ai(e.to_string()))?
            .map_err(|e| EscriturasError::Ai(e.to_string()))
    }
}

impl Library {
    fn study(&self) -> Result<&StudyStore> {
        self.study
            .as_ref()
            .ok_or_else(|| EscriturasError::Storage("Study database is not open".to_string()))
    }
}

fn note_record(note: escrituras_core::Note) -> VerseNote {
    VerseNote {
        verse_title: note.verse_title,
        body: note.body,
        created_at: note.created_at,
        updated_at: note.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn repo_root() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../..").to_string()
    }

    fn library() -> Arc<Library> {
        Library::open(vec![repo_root()], None).expect("bundled data loads")
    }

    #[test]
    fn test_open_fails_without_data() {
        let empty = tempfile::tempdir().unwrap();
        let err = Library::open(vec![empty.path().display().to_string()], None)
            .err()
            .unwrap();
        assert!(matches!(err, EscriturasError::Data(_)));
    }

    #[test]
    fn test_browsing_and_navigation() {
        let lib = library();
        assert_eq!(lib.volumes().len(), 5);
        assert_eq!(lib.books("Pearl of Great Price".into())[0], "Moses");
        assert_eq!(lib.chapters("1 Nephi".into()).len(), 22);

        let verses = lib.verses("Moroni".into(), 10);
        assert_eq!(verses.len(), 34);
        assert_eq!(verses[3].title, "Moroni 10:4");
        assert_eq!(verses[3].volume, "Book of Mormon");

        assert_eq!(
            lib.next_chapter("1 Nephi".into(), 22),
            Some(ChapterRef {
                book: "2 Nephi".into(),
                chapter: 1
            })
        );
        assert_eq!(lib.previous_chapter("1 Nephi".into(), 1), None);
        assert_eq!(
            lib.volume_for_book("Alma".into()).as_deref(),
            Some("Book of Mormon")
        );
    }

    #[test]
    fn test_lookup_and_references() {
        let lib = library();
        assert_eq!(lib.verse("John 3:16".into()).unwrap().number, 16);
        assert!(lib.verse("Nowhere 1:1".into()).is_none());

        let range = lib.lookup("Mosiah 4:19-21".into());
        let numbers: Vec<i32> = range.iter().map(|v| v.number).collect();
        assert_eq!(numbers, [19, 20, 21]);
        assert!(lib.lookup("not a reference".into()).is_empty());

        let refs = lib.extract_references("Alma 32:21, Ether 12:6.".into());
        let titles: Vec<&str> = refs.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Alma 32:21", "Ether 12:6"]);
    }

    #[test]
    fn test_search_respects_limit() {
        let lib = library();
        let results = lib.search("charity never faileth".into(), 3);
        assert!(!results.is_empty());
        assert!(results.len() <= 5);
        assert!(results
            .iter()
            .any(|r| r.verse.title == "1 Corinthians 13:8"));
    }

    #[test]
    fn test_study_data_persists() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("study.db").display().to_string();

        {
            let lib = Library::open(vec![repo_root()], Some(db.clone())).unwrap();
            assert!(lib.save_verse("Alma 32:21".into()).unwrap());
            lib.set_note("Alma 32:21".into(), "Faith is hope".into())
                .unwrap();
            lib.record_reading("Alma".into(), 32).unwrap();
        }

        let lib = Library::open(vec![repo_root()], Some(db)).unwrap();
        let saved = lib.saved_verses().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Alma 32:21");
        assert_eq!(
            lib.note("Alma 32:21".into()).unwrap().unwrap().body,
            "Faith is hope"
        );
        assert_eq!(lib.notes().unwrap().len(), 1);
        assert_eq!(
            lib.last_position().unwrap(),
            Some(ChapterRef {
                book: "Alma".into(),
                chapter: 32
            })
        );
    }

    struct Collect(Mutex<Vec<String>>);

    impl ReplyListener for Collect {
        fn on_delta(&self, text: String) {
            self.0.lock().unwrap().push(text);
        }
    }

    #[test]
    fn test_ask_without_api_key_is_an_ai_error() {
        let lib = library();
        let listener = Arc::new(Collect(Mutex::new(Vec::new())));
        let result = lib.runtime.block_on(lib.ask(
            AiProvider::Claude,
            "claude-opus-5-5".into(),
            None,
            vec![Message {
                role: MessageRole::User,
                content: "What is faith?".into(),
            }],
            StudyContextInput::default(),
            listener.clone(),
        ));
        assert!(matches!(result, Err(EscriturasError::Ai(_))));
        assert!(listener.0.lock().unwrap().is_empty());
    }
}
