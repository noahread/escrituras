//! Tests against the bundled scripture data in `lds-scriptures-2020.12.08/`.

use escrituras_core::ScriptureDb;

async fn load_db() -> ScriptureDb {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../lds-scriptures-2020.12.08/json/lds-scriptures-json.txt"
    );
    let mut db = ScriptureDb::new();
    db.load_from_json(path)
        .await
        .expect("bundled scripture data loads");
    db
}

#[tokio::test]
async fn loads_all_volumes() {
    let db = load_db().await;
    assert_eq!(
        db.get_volumes(),
        [
            "Old Testament",
            "New Testament",
            "Book of Mormon",
            "Doctrine and Covenants",
            "Pearl of Great Price"
        ]
    );
    assert_eq!(db.get_chapters_for_book("1 Nephi").len(), 22);
    assert_eq!(db.get_verses_for_chapter("Moroni", 10).len(), 34);
}

#[tokio::test]
async fn extracts_references_from_prose() {
    let db = load_db().await;
    let text = "According to 1 Nephi 11:16, charity is important.
Alma 7:14-15 teaches about charity.
See also John 3:16 and Romans 8:28.
The Book of Mormon teaches in 2 Nephi 25:29-31 about coming unto Christ.
Paul wrote in 1 Corinthians 13:1-3 about love.
Moses 1:39 explains God's work and glory.";

    let titles: Vec<String> = db
        .extract_scripture_references(text)
        .iter()
        .map(|r| r.display_title())
        .collect();

    // "See also John 3:16" and "and Romans 8:28" are missed because the
    // book-name pattern greedily captures the preceding words; see
    // test_known_limitation_greedy_multiword_matching in scripture.rs.
    assert_eq!(
        titles,
        [
            "1 Nephi 11:16",
            "Alma 7:14-15",
            "2 Nephi 25:29-31",
            "1 Corinthians 13:1-3",
            "Moses 1:39"
        ]
    );
}

#[tokio::test]
async fn navigates_between_chapters_within_a_volume() {
    let db = load_db().await;
    let at = |book: &str, chapter: i32| Some((book.to_string(), chapter));

    // Within a book
    assert_eq!(db.next_chapter("Alma", 32), at("Alma", 33));
    assert_eq!(db.previous_chapter("Alma", 32), at("Alma", 31));

    // Across books in the same volume
    assert_eq!(db.next_chapter("1 Nephi", 22), at("2 Nephi", 1));
    assert_eq!(db.previous_chapter("2 Nephi", 1), at("1 Nephi", 22));

    // Stops at volume boundaries
    assert_eq!(db.next_chapter("Moroni", 10), None);
    assert_eq!(db.previous_chapter("1 Nephi", 1), None);
    assert_eq!(db.previous_chapter("Matthew", 1), None);
    assert_eq!(db.next_chapter("Malachi", 4), None);

    // Single-book volume
    assert_eq!(
        db.next_chapter("Doctrine and Covenants", 76),
        at("Doctrine and Covenants", 77)
    );
    assert_eq!(db.next_chapter("Doctrine and Covenants", 138), None);

    // Unknown book or chapter
    assert_eq!(db.next_chapter("Nowhere", 1), None);
    assert_eq!(db.next_chapter("Alma", 99), None);

    assert_eq!(db.get_volume_for_book("Alma"), Some("Book of Mormon"));
}
