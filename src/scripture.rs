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

/// Represents a scripture reference that may span multiple verses
#[derive(Debug, Clone)]
pub struct ScriptureRange {
    pub book_title: String,
    #[allow(dead_code)]
    pub book_short_title: String,
    pub chapter_number: i32,
    pub start_verse: i32,
    pub end_verse: i32, // Same as start_verse for single verses
}

impl ScriptureRange {
    pub fn display_title(&self) -> String {
        if self.start_verse == self.end_verse {
            format!("{} {}:{}", self.book_title, self.chapter_number, self.start_verse)
        } else {
            format!("{} {}:{}-{}", self.book_title, self.chapter_number, self.start_verse, self.end_verse)
        }
    }

    pub fn contains_verse(&self, verse_num: i32) -> bool {
        verse_num >= self.start_verse && verse_num <= self.end_verse
    }
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
        let content = tokio::fs::read_to_string(path).await?;
        self.scriptures = serde_json::from_str(&content)?;
        self.build_indexes();
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
                .or_default()
                .contains(&scripture.book_title) 
            {
                books_by_vol
                    .entry(scripture.volume_title.clone())
                    .or_default()
                    .push(scripture.book_title.clone());
                    
                seen_books
                    .get_mut(&scripture.volume_title)
                    .unwrap()
                    .insert(scripture.book_title.clone());
            }
            
            // Collect chapters by book in order
            if !seen_chapters
                .entry(scripture.book_title.clone())
                .or_default()
                .contains(&scripture.chapter_number)
            {
                chapters_by_bk
                    .entry(scripture.book_title.clone())
                    .or_default()
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

    /// Extract scripture references from text (e.g., AI responses)
    /// Returns ranges that preserve the original reference format (e.g., "Mormon 11:2-4")
    pub fn extract_scripture_references(&self, text: &str) -> Vec<ScriptureRange> {
        use regex::Regex;

        let mut references = Vec::new();

        // Comprehensive scripture reference patterns
        // Allow optional leading/trailing markdown (**, *, _) and trailing letters/punctuation
        // Also handle en-dash (–) and em-dash (—) in verse ranges
        let patterns = vec![
            // Pattern: "**1 Nephi 11:15–16**:", "2 Corinthians 13:14", "Mosiah 3:19a"
            r"(?:\*+|_+)?(?P<num>[1234]\s+)?(?P<book>[A-Za-z]+(?:\s+[A-Za-z]+)*)\s+(?P<chapter>\d+):(?P<verse>\d+)(?:[-–—](?P<endverse>\d+))?[a-zA-Z]*(?:\*+|_+)?",
        ];

        for pattern_str in patterns {
            if let Ok(re) = Regex::new(pattern_str) {
                for caps in re.captures_iter(text) {
                    let num_prefix = caps.name("num").map(|m| m.as_str().trim()).unwrap_or("");
                    let book_name = caps.name("book").map(|m| m.as_str().trim()).unwrap_or("");
                    let chapter_str = caps.name("chapter").map(|m| m.as_str()).unwrap_or("");
                    let verse_str = caps.name("verse").map(|m| m.as_str()).unwrap_or("");

                    if let (Ok(chapter), Ok(start_verse)) = (chapter_str.parse::<i32>(), verse_str.parse::<i32>()) {
                        // Build full book name with number prefix if present
                        let full_book_name = if !num_prefix.is_empty() {
                            format!("{} {}", num_prefix, book_name)
                        } else {
                            book_name.to_string()
                        };

                        // Verify the reference exists in our database
                        if let Some(scripture) = self.find_exact_scripture(&full_book_name, chapter, start_verse) {
                            // Determine end verse (same as start for single verse references)
                            let end_verse = caps.name("endverse")
                                .and_then(|m| m.as_str().parse::<i32>().ok())
                                .unwrap_or(start_verse);

                            let range = ScriptureRange {
                                book_title: scripture.book_title.clone(),
                                book_short_title: scripture.book_short_title.clone(),
                                chapter_number: chapter,
                                start_verse,
                                end_verse,
                            };

                            // Avoid duplicate ranges
                            if !references.iter().any(|r: &ScriptureRange| {
                                r.book_title == range.book_title
                                    && r.chapter_number == range.chapter_number
                                    && r.start_verse == range.start_verse
                                    && r.end_verse == range.end_verse
                            }) {
                                references.push(range);
                            }
                        }
                    }
                }
            }
        }

        references
    }

    fn find_exact_scripture(&self, book_name: &str, chapter: i32, verse: i32) -> Option<Scripture> {
        for scripture in &self.scriptures {
            // Try exact book title match
            if (scripture.book_title.eq_ignore_ascii_case(book_name) ||
                scripture.book_short_title.eq_ignore_ascii_case(book_name)) &&
               scripture.chapter_number == chapter &&
               scripture.verse_number == verse {
                return Some(scripture.clone());
            }

            // Try fuzzy matching for common variations
            if self.book_matches_fuzzy(&scripture.book_title, book_name) &&
               scripture.chapter_number == chapter &&
               scripture.verse_number == verse {
                return Some(scripture.clone());
            }
        }

        None
    }

    fn book_matches_fuzzy(&self, db_book: &str, search_book: &str) -> bool {
        let db_lower = db_book.to_lowercase();
        let search_lower = search_book.to_lowercase();

        // Direct match
        if db_lower == search_lower {
            return true;
        }

        // Handle common variations (numbered books)
        let variations = [
            ("1 nephi", "nephi"),
            ("2 nephi", "nephi"),
            ("3 nephi", "nephi"),
            ("4 nephi", "nephi"),
            ("1 corinthians", "corinthians"),
            ("2 corinthians", "corinthians"),
            ("1 thessalonians", "thessalonians"),
            ("2 thessalonians", "thessalonians"),
            ("1 timothy", "timothy"),
            ("2 timothy", "timothy"),
            ("1 peter", "peter"),
            ("2 peter", "peter"),
            ("1 john", "john"),
            ("2 john", "john"),
            ("3 john", "john"),
            ("1 samuel", "samuel"),
            ("2 samuel", "samuel"),
            ("1 kings", "kings"),
            ("2 kings", "kings"),
            ("1 chronicles", "chronicles"),
            ("2 chronicles", "chronicles"),
        ];

        for (full, short) in &variations {
            if db_lower == *full && search_lower == *short {
                return true;
            }
        }

        // Also check if db_book contains search_book
        db_lower.contains(&search_lower)
    }
}