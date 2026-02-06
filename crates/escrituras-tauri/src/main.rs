//! Escrituras Tauri Desktop Application
//!
//! This is a placeholder demonstrating how to integrate the escrituras-core library
//! with a Tauri desktop application. The actual UI would be built separately
//! using web technologies (React, Svelte, etc.).

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use escrituras_core::{EmbeddingsDb, Scripture, ScriptureDb};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

/// Application state shared across Tauri commands
struct AppState {
    scripture_db: ScriptureDb,
    embeddings_db: Option<EmbeddingsDb>,
}

/// A scripture verse for JSON serialization to the frontend
#[derive(Serialize)]
struct ScriptureResult {
    verse_title: String,
    book_title: String,
    chapter_number: i32,
    verse_number: i32,
    scripture_text: String,
}

impl From<&Scripture> for ScriptureResult {
    fn from(s: &Scripture) -> Self {
        Self {
            verse_title: s.verse_title.clone(),
            book_title: s.book_title.clone(),
            chapter_number: s.chapter_number,
            verse_number: s.verse_number,
            scripture_text: s.scripture_text.clone(),
        }
    }
}

/// Search result with similarity score
#[derive(Serialize)]
struct SearchResult {
    verse: ScriptureResult,
    score: Option<f32>,
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Get all available scripture volumes
#[tauri::command]
fn get_volumes(state: State<Mutex<AppState>>) -> Vec<String> {
    let state = state.lock().unwrap();
    state.scripture_db.get_volumes().to_vec()
}

/// Get all books in a volume
#[tauri::command]
fn get_books(state: State<Mutex<AppState>>, volume: &str) -> Vec<String> {
    let state = state.lock().unwrap();
    state.scripture_db.get_books_for_volume(volume)
}

/// Get all chapters in a book
#[tauri::command]
fn get_chapters(state: State<Mutex<AppState>>, book: &str) -> Vec<i32> {
    let state = state.lock().unwrap();
    state.scripture_db.get_chapters_for_book(book)
}

/// Get all verses in a chapter
#[tauri::command]
fn get_verses(state: State<Mutex<AppState>>, book: &str, chapter: i32) -> Vec<ScriptureResult> {
    let state = state.lock().unwrap();
    state
        .scripture_db
        .get_verses_for_chapter(book, chapter)
        .iter()
        .map(|s| ScriptureResult::from(*s))
        .collect()
}

/// Look up a verse by reference (e.g., "John 3:16", "1 Nephi 3:7")
#[tauri::command]
fn lookup_verse(state: State<Mutex<AppState>>, reference: &str) -> Option<ScriptureResult> {
    let state = state.lock().unwrap();
    state
        .scripture_db
        .get_by_title(reference)
        .map(|s| ScriptureResult::from(s))
}

/// Search scriptures by keyword
#[tauri::command]
fn search(state: State<Mutex<AppState>>, query: &str, limit: usize) -> Vec<SearchResult> {
    let state = state.lock().unwrap();
    state
        .scripture_db
        .search(query, limit)
        .iter()
        .map(|s| SearchResult {
            verse: ScriptureResult::from(*s),
            score: None,
        })
        .collect()
}

/// Semantic search using embeddings (if available)
#[tauri::command]
fn semantic_search(
    state: State<Mutex<AppState>>,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let mut state = state.lock().unwrap();

    let embeddings_db = state
        .embeddings_db
        .as_mut()
        .ok_or_else(|| "Embeddings not available".to_string())?;

    let results = embeddings_db
        .search(query, limit)
        .map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .filter_map(|(verse_title, score)| {
            state.scripture_db.get_by_title(&verse_title).map(|s| SearchResult {
                verse: ScriptureResult::from(s),
                score: Some(score),
            })
        })
        .collect())
}

/// Extract scripture references from text (e.g., AI response)
#[tauri::command]
fn extract_references(state: State<Mutex<AppState>>, text: &str) -> Vec<String> {
    let state = state.lock().unwrap();
    state
        .scripture_db
        .extract_scripture_references(text)
        .iter()
        .map(|r| r.display_title())
        .collect()
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            // Initialize state - this would normally load data asynchronously
            // For now, we create empty state. The actual app would load scripture data
            // during startup or on-demand.
            Ok(())
        })
        .manage(Mutex::new(AppState {
            scripture_db: ScriptureDb::new(),
            embeddings_db: None,
        }))
        .invoke_handler(tauri::generate_handler![
            get_volumes,
            get_books,
            get_chapters,
            get_verses,
            lookup_verse,
            search,
            semantic_search,
            extract_references,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
