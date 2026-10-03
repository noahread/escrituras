//! Persistent study data: saved verses, verse notes and reading history.
//!
//! Stored in a SQLite database at `<config dir>/escrituras/study.db`. Verses
//! are identified by their verse title (e.g. "Alma 32:21"), which is stable
//! across releases of the scripture data.

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Bump when adding a migration to [`MIGRATIONS`]
const SCHEMA_VERSION: i64 = 1;

/// Migrations to run in order; entry `i` upgrades the schema from version `i`
const MIGRATIONS: &[&str] = &["
    CREATE TABLE saved_verses (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        verse_title TEXT NOT NULL UNIQUE,
        saved_at INTEGER NOT NULL
    );
    CREATE TABLE notes (
        verse_title TEXT PRIMARY KEY,
        body TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE reading_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        book_title TEXT NOT NULL,
        chapter_number INTEGER NOT NULL,
        viewed_at INTEGER NOT NULL,
        UNIQUE (book_title, chapter_number)
    );
"];

/// A note attached to a verse
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub verse_title: String,
    pub body: String,
    /// Unix timestamps (seconds)
    pub created_at: i64,
    pub updated_at: i64,
}

/// Saved verses, notes and reading history for one user
pub struct StudyStore {
    conn: Mutex<Connection>,
}

impl StudyStore {
    /// Open (or create) the store at `<config dir>/escrituras/study.db`
    pub fn open_default() -> Result<Self> {
        Self::open(&Self::default_path()?)
    }

    /// Where [`StudyStore::open_default`] keeps the database
    pub fn default_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("escrituras").join("study.db"))
    }

    /// Open (or create) the store at `path`, creating parent directories
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Creating {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Opening study database {}", path.display()))?;
        Self::from_connection(conn)
    }

    /// An empty store that is not saved to disk
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("Study database lock poisoned"))
    }

    // ---- Saved verses -------------------------------------------------------

    /// Saved verse titles, oldest first
    pub fn saved_verses(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT verse_title FROM saved_verses ORDER BY id")?;
        let titles = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(titles)
    }

    /// Save a verse. Returns false if it was already saved.
    pub fn save_verse(&self, verse_title: &str) -> Result<bool> {
        let inserted = self.conn()?.execute(
            "INSERT OR IGNORE INTO saved_verses (verse_title, saved_at) VALUES (?1, ?2)",
            params![verse_title, now()],
        )?;
        Ok(inserted > 0)
    }

    /// Remove a saved verse (no-op if it isn't saved)
    pub fn remove_saved_verse(&self, verse_title: &str) -> Result<()> {
        self.conn()?.execute(
            "DELETE FROM saved_verses WHERE verse_title = ?1",
            params![verse_title],
        )?;
        Ok(())
    }

    // ---- Notes --------------------------------------------------------------

    /// The note on a verse, if any
    pub fn note(&self, verse_title: &str) -> Result<Option<Note>> {
        let note = self
            .conn()?
            .query_row(
                "SELECT verse_title, body, created_at, updated_at FROM notes WHERE verse_title = ?1",
                params![verse_title],
                note_from_row,
            )
            .optional()?;
        Ok(note)
    }

    /// All notes, most recently updated first
    pub fn notes(&self) -> Result<Vec<Note>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT verse_title, body, created_at, updated_at FROM notes
             ORDER BY updated_at DESC, rowid DESC",
        )?;
        let notes = stmt
            .query_map([], note_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(notes)
    }

    /// Set the note on a verse. Blank text deletes the note.
    pub fn set_note(&self, verse_title: &str, body: &str) -> Result<()> {
        let body = body.trim();
        let conn = self.conn()?;
        if body.is_empty() {
            conn.execute(
                "DELETE FROM notes WHERE verse_title = ?1",
                params![verse_title],
            )?;
        } else {
            let now = now();
            conn.execute(
                "INSERT INTO notes (verse_title, body, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT (verse_title) DO UPDATE SET body = ?2, updated_at = ?3",
                params![verse_title, body, now],
            )?;
        }
        Ok(())
    }

    // ---- Reading history ----------------------------------------------------

    /// Record that a chapter was opened, making it the most recent
    pub fn record_reading(&self, book_title: &str, chapter_number: i32) -> Result<()> {
        let mut conn = self.conn()?;
        // Delete and re-insert so the AUTOINCREMENT id orders by recency
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM reading_history WHERE book_title = ?1 AND chapter_number = ?2",
            params![book_title, chapter_number],
        )?;
        tx.execute(
            "INSERT INTO reading_history (book_title, chapter_number, viewed_at)
             VALUES (?1, ?2, ?3)",
            params![book_title, chapter_number, now()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Chapters as (book_title, chapter_number), most recently opened first
    pub fn recent_chapters(&self, limit: usize) -> Result<Vec<(String, i32)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT book_title, chapter_number FROM reading_history ORDER BY id DESC LIMIT ?1",
        )?;
        let chapters = stmt
            .query_map(params![limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(chapters)
    }

    /// The chapter opened most recently, to resume reading
    pub fn last_position(&self) -> Result<Option<(String, i32)>> {
        Ok(self.recent_chapters(1)?.into_iter().next())
    }
}

fn note_from_row(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        verse_title: row.get(0)?,
        body: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(anyhow!(
            "Study database is from a newer version of escrituras (schema {}, expected {})",
            version,
            SCHEMA_VERSION
        ));
    }

    let tx = conn.transaction()?;
    for (from, migration) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        tx.execute_batch(migration)?;
        tx.pragma_update(None, "user_version", from as i64 + 1)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_verses_keep_order_and_ignore_duplicates() {
        let store = StudyStore::open_in_memory().unwrap();
        assert!(store.save_verse("Alma 32:21").unwrap());
        assert!(store.save_verse("Ether 12:6").unwrap());
        assert!(!store.save_verse("Alma 32:21").unwrap());

        assert_eq!(store.saved_verses().unwrap(), ["Alma 32:21", "Ether 12:6"]);

        store.remove_saved_verse("Alma 32:21").unwrap();
        store.remove_saved_verse("Not saved 1:1").unwrap();
        assert_eq!(store.saved_verses().unwrap(), ["Ether 12:6"]);
    }

    #[test]
    fn test_notes_create_update_and_delete() {
        let store = StudyStore::open_in_memory().unwrap();
        assert_eq!(store.note("Alma 32:21").unwrap(), None);

        store.set_note("Alma 32:21", "  Faith is a hope  ").unwrap();
        let note = store.note("Alma 32:21").unwrap().unwrap();
        assert_eq!(note.body, "Faith is a hope");

        store
            .set_note("Alma 32:21", "Faith is a hope for things not seen")
            .unwrap();
        let updated = store.note("Alma 32:21").unwrap().unwrap();
        assert_eq!(updated.body, "Faith is a hope for things not seen");
        assert_eq!(updated.created_at, note.created_at);

        store.set_note("Alma 32:21", "   ").unwrap();
        assert_eq!(store.note("Alma 32:21").unwrap(), None);
    }

    #[test]
    fn test_notes_listed_most_recent_first() {
        let store = StudyStore::open_in_memory().unwrap();
        store.set_note("Alma 32:21", "a").unwrap();
        store.set_note("Ether 12:6", "b").unwrap();

        let titles: Vec<String> = store
            .notes()
            .unwrap()
            .into_iter()
            .map(|n| n.verse_title)
            .collect();
        assert_eq!(titles, ["Ether 12:6", "Alma 32:21"]);
    }

    #[test]
    fn test_reading_history_orders_by_recency() {
        let store = StudyStore::open_in_memory().unwrap();
        assert_eq!(store.last_position().unwrap(), None);

        store.record_reading("Alma", 32).unwrap();
        store.record_reading("Ether", 12).unwrap();
        store.record_reading("Alma", 32).unwrap();

        assert_eq!(
            store.recent_chapters(10).unwrap(),
            [("Alma".to_string(), 32), ("Ether".to_string(), 12)]
        );
        assert_eq!(
            store.last_position().unwrap(),
            Some(("Alma".to_string(), 32))
        );
        assert_eq!(store.recent_chapters(1).unwrap().len(), 1);
    }

    #[test]
    fn test_data_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("study.db");

        {
            let store = StudyStore::open(&path).unwrap();
            store.save_verse("Moroni 10:4").unwrap();
            store.set_note("Moroni 10:4", "Promise").unwrap();
            store.record_reading("Moroni", 10).unwrap();
        }

        let store = StudyStore::open(&path).unwrap();
        assert_eq!(store.saved_verses().unwrap(), ["Moroni 10:4"]);
        assert_eq!(store.note("Moroni 10:4").unwrap().unwrap().body, "Promise");
        assert_eq!(
            store.last_position().unwrap(),
            Some(("Moroni".to_string(), 10))
        );
    }

    #[test]
    fn test_rejects_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("study.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }
        assert!(StudyStore::open(&path).is_err());
    }
}
