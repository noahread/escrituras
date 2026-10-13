//! Combined semantic + keyword search.

use crate::embeddings::EmbeddingsDb;
use crate::scripture::{Scripture, ScriptureDb};
use std::collections::HashSet;

/// A verse returned by [`combined_search`]
#[derive(Debug, Clone)]
pub struct SearchHit<'a> {
    pub scripture: &'a Scripture,
    /// Cosine similarity for semantic matches; `None` for keyword matches
    pub score: Option<f32>,
}

/// Search by meaning (when embeddings are available) and by keyword.
///
/// Up to `semantic_limit` semantic matches come first, then keyword matches
/// not already included, until there are `limit` results in total. Semantic
/// search failures (e.g. the model can't be loaded) fall back to keyword
/// results only.
pub fn combined_search<'a>(
    db: &'a ScriptureDb,
    embeddings: Option<&EmbeddingsDb>,
    query: &str,
    semantic_limit: usize,
    limit: usize,
) -> Vec<SearchHit<'a>> {
    let semantic = embeddings
        .and_then(|emb| emb.search(query, semantic_limit).ok())
        .unwrap_or_default();
    merge(db, semantic, db.search(query, limit), limit)
}

fn merge<'a>(
    db: &'a ScriptureDb,
    semantic: Vec<(String, f32)>,
    keyword: Vec<&'a Scripture>,
    limit: usize,
) -> Vec<SearchHit<'a>> {
    let mut hits = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();

    for (verse_title, score) in semantic {
        if let Some(scripture) = db.get_by_title(&verse_title) {
            if seen.insert(&scripture.verse_title) {
                hits.push(SearchHit {
                    scripture,
                    score: Some(score),
                });
            }
        }
    }

    for scripture in keyword {
        if hits.len() >= limit {
            break;
        }
        if seen.insert(&scripture.verse_title) {
            hits.push(SearchHit {
                scripture,
                score: None,
            });
        }
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verse(book: &str, chapter: i32, verse: i32, text: &str) -> Scripture {
        let title = format!("{} {}:{}", book, chapter, verse);
        Scripture {
            volume_title: "Book of Mormon".to_string(),
            book_title: book.to_string(),
            book_short_title: book.to_string(),
            chapter_number: chapter,
            verse_number: verse,
            verse_title: title.clone(),
            verse_short_title: title,
            scripture_text: text.to_string(),
        }
    }

    fn test_db() -> ScriptureDb {
        ScriptureDb::from_scriptures(vec![
            verse("Alma", 32, 21, "faith is not to have a perfect knowledge"),
            verse("Alma", 32, 27, "exercise a particle of faith"),
            verse("Ether", 12, 6, "faith is things which are hoped for"),
            verse("Moroni", 10, 5, "by the power of the Holy Ghost"),
        ])
    }

    fn titles(hits: &[SearchHit]) -> Vec<String> {
        hits.iter()
            .map(|h| h.scripture.verse_title.clone())
            .collect()
    }

    #[test]
    fn test_semantic_first_then_keyword_without_duplicates() {
        let db = test_db();
        let semantic = vec![
            ("Moroni 10:5".to_string(), 0.9),
            ("Alma 32:27".to_string(), 0.8),
        ];
        let hits = merge(&db, semantic, db.search("faith", 10), 10);

        assert_eq!(
            titles(&hits),
            ["Moroni 10:5", "Alma 32:27", "Alma 32:21", "Ether 12:6"]
        );
        assert_eq!(hits[0].score, Some(0.9));
        assert_eq!(hits[2].score, None);
    }

    #[test]
    fn test_keyword_results_stop_at_limit() {
        let db = test_db();
        let semantic = vec![("Moroni 10:5".to_string(), 0.9)];
        let hits = merge(&db, semantic, db.search("faith", 10), 2);

        assert_eq!(titles(&hits), ["Moroni 10:5", "Alma 32:21"]);
    }

    #[test]
    fn test_semantic_results_are_not_padded_past_limit() {
        let db = test_db();
        let semantic = vec![
            ("Moroni 10:5".to_string(), 0.9),
            ("Ether 12:6".to_string(), 0.8),
        ];
        let hits = merge(&db, semantic, db.search("faith", 10), 1);

        assert_eq!(titles(&hits), ["Moroni 10:5", "Ether 12:6"]);
    }

    #[test]
    fn test_unknown_semantic_titles_are_skipped() {
        let db = test_db();
        let semantic = vec![("Nowhere 1:1".to_string(), 0.9)];
        let hits = merge(&db, semantic, Vec::new(), 10);

        assert!(hits.is_empty());
    }

    #[test]
    fn test_keyword_only_without_embeddings() {
        let db = test_db();
        let hits = combined_search(&db, None, "faith", 20, 50);

        assert_eq!(titles(&hits), ["Alma 32:21", "Alma 32:27", "Ether 12:6"]);
    }
}
