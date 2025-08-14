use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Scripture {
    pub volume_title: String,
    pub book_title: String,
    pub book_short_title: String,
    pub chapter_number: i32,
    pub verse_number: i32,
    pub verse_title: String,
    pub verse_short_title: String,
    pub scripture_text: String,
}

pub struct ScriptureDb {
    scriptures: Vec<Scripture>,
    volumes: Vec<String>,
    books_by_volume: HashMap<String, Vec<String>>,
    chapters_by_book: HashMap<String, Vec<i32>>,
}

impl ScriptureDb {
    pub fn new() -> Self {
        Self {
            scriptures: Vec::new(),
            volumes: Vec::new(),
            books_by_volume: HashMap::new(),
            chapters_by_book: HashMap::new(),
        }
    }
    
    pub async fn load_from_json(&mut self, path: &str) -> Result<()> {
        println!("📚 Loading scriptures from {}...", path);
        
        let content = tokio::fs::read_to_string(path).await?;
        self.scriptures = serde_json::from_str(&content)?;
        
        self.build_indexes();
        
        println!("✅ Loaded {} scriptures across {} volumes", 
                 self.scriptures.len(), 
                 self.volumes.len());
        
        Ok(())
    }
    
    fn build_indexes(&mut self) {
        let mut volumes_order = Vec::new();
        let mut books_by_vol: HashMap<String, Vec<String>> = HashMap::new();
        let mut chapters_by_bk: HashMap<String, Vec<i32>> = HashMap::new();
        
        // Track seen items to maintain order while avoiding duplicates
        let mut seen_volumes = HashSet::new();
        let mut seen_books: HashMap<String, HashSet<String>> = HashMap::new();
        let mut seen_chapters: HashMap<String, HashSet<i32>> = HashMap::new();
        
        // Process in original order to preserve canonical sequence
        for scripture in &self.scriptures {
            // Collect volumes in order
            if !seen_volumes.contains(&scripture.volume_title) {
                volumes_order.push(scripture.volume_title.clone());
                seen_volumes.insert(scripture.volume_title.clone());
            }
            
            // Collect books by volume in order
            if !seen_books
                .entry(scripture.volume_title.clone())
                .or_insert_with(HashSet::new)
                .contains(&scripture.book_title) 
            {
                books_by_vol
                    .entry(scripture.volume_title.clone())
                    .or_insert_with(Vec::new)
                    .push(scripture.book_title.clone());
                    
                seen_books
                    .get_mut(&scripture.volume_title)
                    .unwrap()
                    .insert(scripture.book_title.clone());
            }
            
            // Collect chapters by book in order
            if !seen_chapters
                .entry(scripture.book_title.clone())
                .or_insert_with(HashSet::new)
                .contains(&scripture.chapter_number)
            {
                chapters_by_bk
                    .entry(scripture.book_title.clone())
                    .or_insert_with(Vec::new)
                    .push(scripture.chapter_number);
                    
                seen_chapters
                    .get_mut(&scripture.book_title)
                    .unwrap()
                    .insert(scripture.chapter_number);
            }
        }
        
        // Store in order (no sorting needed since we preserved original order)
        self.volumes = volumes_order;
        self.books_by_volume = books_by_vol;
        
        // Sort chapters numerically for each book
        for chapters in chapters_by_bk.values_mut() {
            chapters.sort();
        }
        self.chapters_by_book = chapters_by_bk;
    }
    
    pub fn get_volumes(&self) -> &[String] {
        &self.volumes
    }
    
    pub fn get_books_for_volume(&self, volume: &str) -> Vec<String> {
        self.books_by_volume
            .get(volume)
            .cloned()
            .unwrap_or_default()
    }
    
    pub fn get_chapters_for_book(&self, book: &str) -> Vec<i32> {
        self.chapters_by_book
            .get(book)
            .cloned()
            .unwrap_or_default()
    }
    
    pub fn get_verses_for_chapter(&self, book: &str, chapter: i32) -> Vec<&Scripture> {
        self.scriptures
            .iter()
            .filter(|s| s.book_title == book && s.chapter_number == chapter)
            .collect()
    }
    
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Scripture> {
        let query_lower = query.to_lowercase();
        
        self.scriptures
            .iter()
            .filter(|scripture| {
                scripture.scripture_text.to_lowercase().contains(&query_lower)
                    || scripture.verse_title.to_lowercase().contains(&query_lower)
                    || scripture.book_title.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .collect()
    }
    
    pub fn get_all_scriptures(&self) -> &[Scripture] {
        &self.scriptures
    }
}