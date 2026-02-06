use crate::embeddings::EmbeddingsDb;
use crate::scripture::ScriptureDb;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, Write};

#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

#[derive(Debug, Serialize)]
struct McpError {
    code: i32,
    message: String,
}

impl McpResponse {
    fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<serde_json::Value>, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

fn get_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "lookup_verse".to_string(),
            description: "Get a specific scripture verse by reference (e.g., 'John 3:16', '1 Nephi 3:7')".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reference": {
                        "type": "string",
                        "description": "Scripture reference (e.g., 'John 3:16', '1 Nephi 3:7', 'D&C 4:2')"
                    }
                },
                "required": ["reference"]
            }),
        },
        ToolDefinition {
            name: "lookup_chapter".to_string(),
            description: "Get all verses in a chapter".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "book": {
                        "type": "string",
                        "description": "Book name (e.g., 'John', '1 Nephi', 'Doctrine and Covenants')"
                    },
                    "chapter": {
                        "type": "integer",
                        "description": "Chapter number"
                    }
                },
                "required": ["book", "chapter"]
            }),
        },
        ToolDefinition {
            name: "search_scriptures".to_string(),
            description: "Search scriptures by keyword with stemming (e.g., 'faith' matches 'faithful', 'faithfully')".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 10)",
                        "default": 10
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_context".to_string(),
            description: "Get verses surrounding a reference for context".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reference": {
                        "type": "string",
                        "description": "Scripture reference (e.g., 'John 3:16')"
                    },
                    "before": {
                        "type": "integer",
                        "description": "Number of verses before (default: 2)",
                        "default": 2
                    },
                    "after": {
                        "type": "integer",
                        "description": "Number of verses after (default: 2)",
                        "default": 2
                    }
                },
                "required": ["reference"]
            }),
        },
        ToolDefinition {
            name: "list_books".to_string(),
            description: "List all books, optionally filtered by volume".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "volume": {
                        "type": "string",
                        "description": "Optional volume filter: 'Old Testament', 'New Testament', 'Book of Mormon', 'Doctrine and Covenants', or 'Pearl of Great Price'"
                    }
                }
            }),
        },
    ]
}

fn handle_initialize(id: Option<serde_json::Value>) -> McpResponse {
    McpResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "scriptures",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Option<serde_json::Value>) -> McpResponse {
    McpResponse::success(
        id,
        serde_json::json!({
            "tools": get_tools()
        }),
    )
}

fn handle_tool_call(
    id: Option<serde_json::Value>,
    params: &serde_json::Value,
    db: &ScriptureDb,
    embeddings: &mut Option<EmbeddingsDb>,
) -> McpResponse {
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

    match tool_name {
        "lookup_verse" => handle_lookup_verse(id, &arguments, db),
        "lookup_chapter" => handle_lookup_chapter(id, &arguments, db),
        "search_scriptures" => handle_search_scriptures(id, &arguments, db, embeddings),
        "get_context" => handle_get_context(id, &arguments, db),
        "list_books" => handle_list_books(id, &arguments, db),
        _ => McpResponse::error(id, -32601, &format!("Unknown tool: {}", tool_name)),
    }
}

fn handle_lookup_verse(
    id: Option<serde_json::Value>,
    args: &serde_json::Value,
    db: &ScriptureDb,
) -> McpResponse {
    let reference = match args.get("reference").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => return McpResponse::error(id, -32602, "Missing 'reference' parameter"),
    };

    // Parse reference (e.g., "John 3:16" or "1 Nephi 3:7")
    let refs = db.extract_scripture_references(reference);
    if refs.is_empty() {
        return McpResponse::error(id, -32602, &format!("Could not parse reference: {}", reference));
    }

    let scripture_ref = &refs[0];
    let verses = db.get_verses_for_chapter(&scripture_ref.book_title, scripture_ref.chapter_number);

    let matching_verses: Vec<_> = verses
        .into_iter()
        .filter(|v| v.verse_number >= scripture_ref.start_verse && v.verse_number <= scripture_ref.end_verse)
        .collect();

    if matching_verses.is_empty() {
        return McpResponse::error(id, -32602, &format!("Verse not found: {}", reference));
    }

    let content = matching_verses
        .iter()
        .map(|v| format!("{} - {}", v.verse_title, v.scripture_text))
        .collect::<Vec<_>>()
        .join("\n\n");

    McpResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": content
            }]
        }),
    )
}

fn handle_lookup_chapter(
    id: Option<serde_json::Value>,
    args: &serde_json::Value,
    db: &ScriptureDb,
) -> McpResponse {
    let book = match args.get("book").and_then(|v| v.as_str()) {
        Some(b) => b,
        None => return McpResponse::error(id, -32602, "Missing 'book' parameter"),
    };

    let chapter = match args.get("chapter").and_then(|v| v.as_i64()) {
        Some(c) => c as i32,
        None => return McpResponse::error(id, -32602, "Missing 'chapter' parameter"),
    };

    let verses = db.get_verses_for_chapter(book, chapter);
    if verses.is_empty() {
        return McpResponse::error(id, -32602, &format!("Chapter not found: {} {}", book, chapter));
    }

    let content = verses
        .iter()
        .map(|v| format!("{}. {}", v.verse_number, v.scripture_text))
        .collect::<Vec<_>>()
        .join("\n\n");

    McpResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("{} {}\n\n{}", book, chapter, content)
            }]
        }),
    )
}

fn handle_search_scriptures(
    id: Option<serde_json::Value>,
    args: &serde_json::Value,
    db: &ScriptureDb,
    embeddings: &mut Option<EmbeddingsDb>,
) -> McpResponse {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q,
        None => return McpResponse::error(id, -32602, "Missing 'query' parameter"),
    };

    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10) as usize;

    let semantic_limit = (limit / 2).max(5); // Use half for semantic, at least 5
    let mut combined_results: Vec<String> = Vec::new();
    let mut seen_titles: HashSet<String> = HashSet::new();

    // Try semantic search if embeddings are available (uses local ONNX model)
    if let Some(emb) = embeddings {
        if let Ok(semantic_matches) = emb.search(query, semantic_limit) {
            for (verse_title, _score) in semantic_matches {
                if let Some(scripture) = db.get_by_title(&verse_title) {
                    seen_titles.insert(verse_title);
                    combined_results.push(format!("{} - {}", scripture.verse_title, scripture.scripture_text));
                }
            }
        }
    }

    // Add keyword search results (deduped)
    let keyword_results = db.search(query, limit);
    for scripture in keyword_results {
        if !seen_titles.contains(&scripture.verse_title) {
            seen_titles.insert(scripture.verse_title.clone());
            combined_results.push(format!("{} - {}", scripture.verse_title, scripture.scripture_text));
            if combined_results.len() >= limit {
                break;
            }
        }
    }

    if combined_results.is_empty() {
        return McpResponse::success(
            id,
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": format!("No results found for: {}", query)
                }]
            }),
        );
    }

    let content = combined_results.join("\n\n");

    McpResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Found {} results for '{}':\n\n{}", combined_results.len(), query, content)
            }]
        }),
    )
}

fn handle_get_context(
    id: Option<serde_json::Value>,
    args: &serde_json::Value,
    db: &ScriptureDb,
) -> McpResponse {
    let reference = match args.get("reference").and_then(|v| v.as_str()) {
        Some(r) => r,
        None => return McpResponse::error(id, -32602, "Missing 'reference' parameter"),
    };

    let before = args.get("before").and_then(|v| v.as_i64()).unwrap_or(2) as i32;
    let after = args.get("after").and_then(|v| v.as_i64()).unwrap_or(2) as i32;

    // Parse reference
    let refs = db.extract_scripture_references(reference);
    if refs.is_empty() {
        return McpResponse::error(id, -32602, &format!("Could not parse reference: {}", reference));
    }

    let scripture_ref = &refs[0];
    let verses = db.get_verses_for_chapter(&scripture_ref.book_title, scripture_ref.chapter_number);

    let start_verse = (scripture_ref.start_verse - before).max(1);
    let end_verse = scripture_ref.end_verse + after;

    let context_verses: Vec<_> = verses
        .into_iter()
        .filter(|v| v.verse_number >= start_verse && v.verse_number <= end_verse)
        .collect();

    if context_verses.is_empty() {
        return McpResponse::error(id, -32602, &format!("Verse not found: {}", reference));
    }

    let content = context_verses
        .iter()
        .map(|v| {
            let marker = if v.verse_number >= scripture_ref.start_verse
                && v.verse_number <= scripture_ref.end_verse
            {
                ">>> "
            } else {
                "    "
            };
            format!("{}{}. {}", marker, v.verse_number, v.scripture_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    McpResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("{} {} (context)\n\n{}", scripture_ref.book_title, scripture_ref.chapter_number, content)
            }]
        }),
    )
}

fn handle_list_books(
    id: Option<serde_json::Value>,
    args: &serde_json::Value,
    db: &ScriptureDb,
) -> McpResponse {
    let volume_filter = args.get("volume").and_then(|v| v.as_str());

    let volumes = db.get_volumes();
    let mut result = String::new();

    for volume in volumes {
        if let Some(filter) = volume_filter {
            if !volume.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        result.push_str(&format!("## {}\n", volume));
        let books = db.get_books_for_volume(volume);
        for book in books {
            let chapters = db.get_chapters_for_book(&book);
            result.push_str(&format!("- {} ({} chapters)\n", book, chapters.len()));
        }
        result.push('\n');
    }

    if result.is_empty() {
        return McpResponse::error(
            id,
            -32602,
            &format!(
                "Volume not found: {}. Available: Old Testament, New Testament, Book of Mormon, Doctrine and Covenants, Pearl of Great Price",
                volume_filter.unwrap_or("(none)")
            ),
        );
    }

    McpResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": result.trim()
            }]
        }),
    )
}

pub fn run_mcp_server(
    db: ScriptureDb,
    mut embeddings: Option<EmbeddingsDb>,
) {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = McpResponse::error(None, -32700, &format!("Parse error: {}", e));
                let mut stdout = stdout.lock();
                let _ = serde_json::to_writer(&mut stdout, &response);
                let _ = writeln!(stdout);
                let _ = stdout.flush();
                continue;
            }
        };

        let response = match request.method.as_str() {
            "initialize" => handle_initialize(request.id),
            "notifications/initialized" => continue, // Notification, no response
            "tools/list" => handle_tools_list(request.id),
            "tools/call" => handle_tool_call(request.id, &request.params, &db, &mut embeddings),
            _ => McpResponse::error(request.id, -32601, &format!("Method not found: {}", request.method)),
        };

        let mut stdout = stdout.lock();
        let _ = serde_json::to_writer(&mut stdout, &response);
        let _ = writeln!(stdout);
        let _ = stdout.flush();
    }
}
