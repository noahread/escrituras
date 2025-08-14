use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{Select, Input, theme::ColorfulTheme};
use anyhow::Result;

mod scripture;
mod ollama;

use scripture::{ScriptureDb, Scripture};
use ollama::OllamaClient;

#[derive(Clone)]
struct ConversationMessage {
    question: String,
    response: String,
    context: Vec<Scripture>,
}

#[derive(Parser)]
#[command(name = "scripture")]
#[command(about = "CLI for searching LDS scriptures and querying with Ollama")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Browse scriptures by book (interactive)
    Browse,
    /// Search scripture text
    Search {
        /// Search query
        query: String,
        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Query Ollama with scripture context
    Query {
        /// Your question
        question: String,
        /// Scripture context (search term to find relevant verses)
        #[arg(short, long)]
        context: Option<String>,
        /// Ollama model to use
        #[arg(short, long, default_value = "llama3.2:latest")]
        model: String,
    },
    /// List available volumes and books
    List,
    /// List available Ollama models
    Models,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize scripture database
    let mut db = ScriptureDb::new();
    db.load_from_json("lds-scriptures-2020.12.08/json/lds-scriptures-json.txt").await?;
    
    match cli.command {
        Commands::Browse => browse_interactive(&db).await?,
        Commands::Search { query, limit } => search_scriptures(&db, &query, limit).await?,
        Commands::Query { question, context, model } => {
            interactive_query_session(&db, &question, context.as_deref(), &model).await?
        },
        Commands::List => list_books(&db).await?,
        Commands::Models => list_ollama_models().await?,
    }
    
    Ok(())
}

async fn browse_interactive(db: &ScriptureDb) -> Result<()> {
    let volumes = db.get_volumes();
    
    loop {
        println!("\n{}", "📖 Scripture Browser".bold().blue());
        
        // Select volume
        let volume_names: Vec<String> = volumes.iter()
            .map(|v| v.clone())
            .collect();
        
        let volume_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a volume")
            .items(&volume_names)
            .default(0)
            .interact()?;
            
        let selected_volume = &volume_names[volume_selection];
        
        // Get books for this volume
        let books = db.get_books_for_volume(selected_volume);
        
        if books.is_empty() {
            println!("{}", "No books found for this volume".red());
            continue;
        }
        
        // Select book
        let book_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Select a book from {}", selected_volume))
            .items(&books)
            .default(0)
            .interact()?;
            
        let selected_book = &books[book_selection];
        
        // Get chapters for this book
        let chapters = db.get_chapters_for_book(selected_book);
        
        // Select chapter
        let chapter_options: Vec<String> = chapters.iter()
            .map(|&ch| format!("Chapter {}", ch))
            .collect();
            
        let chapter_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Select a chapter from {}", selected_book))
            .items(&chapter_options)
            .default(0)
            .interact()?;
            
        let selected_chapter = chapters[chapter_selection];
        
        // Display chapter verses
        display_chapter(db, selected_book, selected_chapter).await?;
        
        // Ask if user wants to continue
        let continue_browsing = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("What would you like to do?")
            .items(&["Browse another chapter", "Exit"])
            .default(0)
            .interact()?;
            
        if continue_browsing == 1 {
            break;
        }
    }
    
    Ok(())
}

async fn display_chapter(db: &ScriptureDb, book: &str, chapter: i32) -> Result<()> {
    let verses = db.get_verses_for_chapter(book, chapter);
    
    println!("\n{}", format!("📜 {} Chapter {}", book, chapter).bold().green());
    println!("{}", "=".repeat(50).dimmed());
    
    for verse in &verses {
        println!(
            "\n{}  {}",
            format!("{}:{}", chapter, verse.verse_number).bold().yellow(),
            verse.scripture_text
        );
    }
    
    println!("\n{}", "=".repeat(50).dimmed());
    println!("{} verses displayed", verses.len().to_string().bold());
    
    Ok(())
}

async fn search_scriptures(db: &ScriptureDb, query: &str, limit: usize) -> Result<()> {
    println!("🔍 Searching for: {}", query.bold().cyan());
    
    let results = db.search(query, limit);
    
    if results.is_empty() {
        println!("{}", "No results found".red());
        return Ok(());
    }
    
    println!("\n{} results found:\n", results.len().to_string().bold().green());
    
    for (i, verse) in results.iter().enumerate() {
        println!(
            "{}. {} - {}",
            (i + 1).to_string().bold().blue(),
            verse.verse_title.bold().yellow(),
            verse.book_title.dimmed()
        );
        println!("   {}\n", verse.scripture_text);
    }
    
    Ok(())
}

async fn query_ollama(
    db: &ScriptureDb, 
    question: &str, 
    context_search: Option<&str>, 
    model: &str
) -> Result<()> {
    let ollama = OllamaClient::new("http://localhost:11434");
    
    // Get scripture context if requested
    let context_verses = if let Some(search_term) = context_search {
        println!("🔍 Finding scripture context for: {}", search_term.cyan());
        let verses = db.search(search_term, 5);
        if !verses.is_empty() {
            println!("📖 Using {} verses as context\n", verses.len().to_string().bold());
            Some(verses)
        } else {
            println!("⚠️  No verses found for context search\n");
            None
        }
    } else {
        None
    };
    
    // Build prompt with context
    let prompt = if let Some(verses) = &context_verses {
        let owned_verses: Vec<Scripture> = verses.iter().map(|v| (*v).clone()).collect();
        build_prompt_with_context(question, Some(&owned_verses))
    } else {
        build_prompt_with_context(question, None)
    };
    
    println!("🤖 Querying {} with your question...\n", model.bold().magenta());
    
    match ollama.query(model, &prompt).await {
        Ok(response) => {
            println!("{}", "Response:".bold().green());
            println!("{}", response);
            
            if let Some(verses) = context_verses {
                println!("\n{}", "Scripture Context Used:".bold().blue());
                for verse in verses {
                    println!("• {} - {}", verse.verse_title.yellow(), verse.book_title.dimmed());
                }
            }
        },
        Err(e) => {
            println!("{}: {}", "Error querying Ollama".red(), e);
            println!("Make sure Ollama is running: {}", "ollama serve".bold());
        }
    }
    
    Ok(())
}

async fn interactive_query_session(
    db: &ScriptureDb,
    initial_question: &str, 
    initial_context: Option<&str>, 
    model: &str
) -> Result<()> {
    let mut conversation: Vec<ConversationMessage> = Vec::new();
    let mut current_question = initial_question.to_string();
    let mut current_context = initial_context.map(|s| s.to_string());
    
    loop {
        // Execute the current query
        let context_verses = if let Some(search_term) = &current_context {
            println!("🔍 Finding scripture context for: {}", search_term.cyan());
            let verses = db.search(search_term, 5);
            if !verses.is_empty() {
                println!("📖 Using {} verses as context\n", verses.len().to_string().bold());
                Some(verses.iter().map(|v| (*v).clone()).collect::<Vec<Scripture>>())
            } else {
                println!("⚠️  No verses found for context search\n");
                None
            }
        } else {
            None
        };
        
        // Build prompt with conversation history
        let prompt = build_conversation_prompt(&current_question, context_verses.as_deref(), &conversation);
        
        println!("🤖 Querying {} with your question...\n", model.bold().magenta());
        
        let ollama = OllamaClient::new("http://localhost:11434");
        let response = match ollama.query(model, &prompt).await {
            Ok(response) => {
                println!("{}", "Response:".bold().green());
                println!("{}", response);
                response
            },
            Err(e) => {
                println!("{}: {}", "Error querying Ollama".red(), e);
                println!("Make sure Ollama is running: {}", "ollama serve".bold());
                return Ok(());
            }
        };
        
        // Store in conversation history
        conversation.push(ConversationMessage {
            question: current_question.clone(),
            response: response.clone(),
            context: context_verses.clone().unwrap_or_default(),
        });
        
        // Show scripture references if they were used
        if let Some(verses) = &context_verses {
            println!("\n{}", "Scripture Context Used:".bold().blue());
            for verse in verses {
                println!("• {} - {}", verse.verse_title.yellow(), verse.book_title.dimmed());
            }
        }
        
        // Extract and offer referenced scriptures
        let referenced_scriptures = extract_scripture_references(&response, db);
        if !referenced_scriptures.is_empty() {
            println!("\n{}", "📖 Scripture References Found:".bold().blue());
            for (i, scripture) in referenced_scriptures.iter().enumerate() {
                println!("{}. {} - {}", 
                    (i + 1).to_string().bold().blue(),
                    scripture.verse_title.yellow(), 
                    scripture.book_title.dimmed()
                );
            }
            
            // Offer to read any referenced scripture
            let options = vec![
                "Ask a follow-up question".to_string(),
                "Read a referenced scripture".to_string(),
                "Exit".to_string(),
            ];
            
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("What would you like to do next?")
                .items(&options)
                .default(0)
                .interact()?;
                
            match selection {
                0 => {
                    // Follow-up question
                    let follow_up: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Follow-up question")
                        .interact_text()?;
                    current_question = follow_up;
                    current_context = None; // Let them optionally specify new context
                },
                1 => {
                    // Read referenced scripture
                    let scripture_options: Vec<String> = referenced_scriptures
                        .iter()
                        .map(|s| s.verse_title.clone())
                        .collect();
                    
                    let scripture_selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select a scripture to read")
                        .items(&scripture_options)
                        .default(0)
                        .interact()?;
                    
                    let selected_scripture = &referenced_scriptures[scripture_selection];
                    display_scripture_with_context(db, selected_scripture).await?;
                    
                    // After reading, show simplified options and continue without re-querying
                    println!("\n{}", "📖 Scripture reading complete".dimmed());
                    
                    // Return to the conversation options without running LLM again
                    let options = vec![
                        "Ask a follow-up question".to_string(),
                        "Read another referenced scripture".to_string(),
                        "Exit".to_string(),
                    ];
            
                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("What would you like to do next?")
                        .items(&options)
                        .default(0)
                        .interact()?;
                        
                    match selection {
                        0 => {
                            // Follow-up question
                            let follow_up: String = Input::with_theme(&ColorfulTheme::default())
                                .with_prompt("Follow-up question")
                                .interact_text()?;
                            current_question = follow_up;
                            current_context = None;
                            break; // Exit this inner logic to proceed with new question
                        },
                        1 => {
                            // Loop back to read another scripture
                            // Stay in the scripture selection
                        },
                        _ => return Ok(()), // Exit
                    }
                },
                2 => break, // Exit
                _ => break,
            }
        } else {
            // No references found, just offer follow-up or exit
            let options = vec![
                "Ask a follow-up question".to_string(),
                "Search for context on this topic".to_string(),
                "Exit".to_string(),
            ];
            
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("What would you like to do next?")
                .items(&options)
                .default(0)
                .interact()?;
                
            match selection {
                0 => {
                    // Follow-up question
                    let follow_up: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Follow-up question")
                        .interact_text()?;
                    current_question = follow_up;
                },
                1 => {
                    // Search for context
                    let search_term: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Search for scripture context")
                        .interact_text()?;
                    current_context = Some(search_term);
                    continue; // Use same question with new context
                },
                2 => break, // Exit
                _ => break,
            }
        }
    }
    
    println!("\n{}", "Thanks for studying the scriptures! 📖✨".bold().blue());
    Ok(())
}

fn build_prompt_with_context(question: &str, verses: Option<&[Scripture]>) -> String {
    let mut prompt = String::new();
    
    if let Some(context_verses) = verses {
        prompt.push_str("Please answer the following question using the provided scripture context:\n\n");
        prompt.push_str("Scripture Context:\n");
        for verse in context_verses {
            prompt.push_str(&format!("{}: {}\n", verse.verse_title, verse.scripture_text));
        }
        prompt.push_str("\n");
    }
    
    prompt.push_str("Question: ");
    prompt.push_str(question);
    
    if verses.is_some() {
        prompt.push_str("\n\nPlease reference the scriptures in your answer when relevant.");
    }
    
    prompt
}

fn build_conversation_prompt(question: &str, verses: Option<&[Scripture]>, conversation: &[ConversationMessage]) -> String {
    let mut prompt = String::new();
    
    // Add conversation history
    if !conversation.is_empty() {
        prompt.push_str("Previous conversation context:\n");
        for msg in conversation.iter().take(3) { // Keep last 3 exchanges
            prompt.push_str(&format!("Q: {}\nA: {}\n\n", msg.question, msg.response));
        }
        prompt.push_str("---\n\n");
    }
    
    // Add current context and question
    if let Some(context_verses) = verses {
        prompt.push_str("Please answer the following question using the provided scripture context:\n\n");
        prompt.push_str("Scripture Context:\n");
        for verse in context_verses {
            prompt.push_str(&format!("{}: {}\n", verse.verse_title, verse.scripture_text));
        }
        prompt.push_str("\n");
    }
    
    prompt.push_str("Question: ");
    prompt.push_str(question);
    
    if verses.is_some() {
        prompt.push_str("\n\nPlease reference the scriptures in your answer when relevant.");
    }
    
    prompt
}

fn extract_scripture_references(response: &str, db: &ScriptureDb) -> Vec<Scripture> {
    use regex::Regex;
    
    let mut references = Vec::new();
    
    // Comprehensive scripture reference patterns
    let patterns = vec![
        // Pattern: "1 Nephi 11:16", "2 Corinthians 13:14", "3 John 1:4"
        r"(?P<num>[123]\s+)?(?P<book>[A-Za-z]+(?:\s+[A-Za-z]+)*)\s+(?P<chapter>\d+):(?P<verse>\d+)(?:-(?P<endverse>\d+))?",
        // Pattern: "Genesis 1:1", "John 3:16", "Romans 8:28" 
        r"(?P<book>[A-Za-z]+(?:\s+[A-Za-z]+)*)\s+(?P<chapter>\d+):(?P<verse>\d+)(?:-(?P<endverse>\d+))?",
        // Pattern: "Alma 7:14-15" (range), "Matthew 5:3-12"
        r"(?P<book>[A-Za-z]+(?:\s+[A-Za-z]+)*)\s+(?P<chapter>\d+):(?P<verse>\d+)-(?P<endverse>\d+)",
    ];
    
    for pattern_str in patterns {
        if let Ok(re) = Regex::new(pattern_str) {
            for caps in re.captures_iter(response) {
                let num_prefix = caps.name("num").map(|m| m.as_str().trim()).unwrap_or("");
                let book_name = caps.name("book").map(|m| m.as_str().trim()).unwrap_or("");
                let chapter_str = caps.name("chapter").map(|m| m.as_str()).unwrap_or("");
                let verse_str = caps.name("verse").map(|m| m.as_str()).unwrap_or("");
                
                if let (Ok(chapter), Ok(verse)) = (chapter_str.parse::<i32>(), verse_str.parse::<i32>()) {
                    // Build full book name with number prefix if present
                    let full_book_name = if !num_prefix.is_empty() {
                        format!("{}{}", num_prefix, book_name)
                    } else {
                        book_name.to_string()
                    };
                    
                    // Try to find exact match first
                    if let Some(scripture) = find_exact_scripture(db, &full_book_name, chapter, verse) {
                        if !references.iter().any(|r: &Scripture| r.verse_title == scripture.verse_title) {
                            references.push(scripture);
                        }
                    }
                    
                    // Handle verse ranges (e.g., "7:14-15")
                    if let Some(end_verse_match) = caps.name("endverse") {
                        if let Ok(end_verse) = end_verse_match.as_str().parse::<i32>() {
                            for v in (verse + 1)..=end_verse {
                                if let Some(scripture) = find_exact_scripture(db, &full_book_name, chapter, v) {
                                    if !references.iter().any(|r: &Scripture| r.verse_title == scripture.verse_title) {
                                        references.push(scripture);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    references
}

fn find_exact_scripture(db: &ScriptureDb, book_name: &str, chapter: i32, verse: i32) -> Option<Scripture> {
    // Get all scriptures for this book and find exact match
    let book_scriptures = db.search(book_name, 1000);
    
    for scripture in book_scriptures {
        // Try exact book title match
        if (scripture.book_title.eq_ignore_ascii_case(book_name) || 
            scripture.book_short_title.eq_ignore_ascii_case(book_name)) &&
           scripture.chapter_number == chapter && 
           scripture.verse_number == verse {
            return Some(scripture.clone());
        }
        
        // Try fuzzy matching for common variations
        if book_matches_fuzzy(&scripture.book_title, book_name) && 
           scripture.chapter_number == chapter && 
           scripture.verse_number == verse {
            return Some(scripture.clone());
        }
    }
    
    None
}

fn book_matches_fuzzy(db_book: &str, search_book: &str) -> bool {
    let db_lower = db_book.to_lowercase();
    let search_lower = search_book.to_lowercase();
    
    // Handle common variations
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
        ("1 kings", "kings"),
        ("2 kings", "kings"),
        ("1 samuel", "samuel"),
        ("2 samuel", "samuel"),
        ("1 chronicles", "chronicles"),
        ("2 chronicles", "chronicles"),
        ("doctrine and covenants", "covenants"),
    ];
    
    // Exact match
    if db_lower == search_lower {
        return true;
    }
    
    // Check variations
    for (full_name, short_name) in &variations {
        if (db_lower == *full_name && search_lower.contains(short_name)) ||
           (search_lower == *full_name && db_lower.contains(short_name)) {
            return true;
        }
    }
    
    // Contains match as fallback
    db_lower.contains(&search_lower) || search_lower.contains(&db_lower)
}

async fn display_scripture_with_context(db: &ScriptureDb, scripture: &Scripture) -> Result<()> {
    println!("\n{}", format!("📜 {} - {}", scripture.verse_title, scripture.book_title).bold().green());
    println!("{}", "=".repeat(60).dimmed());
    
    // Show the selected verse
    println!("\n{}  {}", 
             format!("{}:{}", scripture.chapter_number, scripture.verse_number).bold().yellow(),
             scripture.scripture_text.bold()
    );
    
    // Show surrounding verses for context
    let surrounding_verses = db.get_verses_for_chapter(&scripture.book_title, scripture.chapter_number);
    let current_index = surrounding_verses.iter().position(|v| v.verse_number == scripture.verse_number);
    
    if let Some(index) = current_index {
        println!("\n{}", "Context (surrounding verses):".dimmed());
        
        let start = if index >= 2 { index - 2 } else { 0 };
        let end = std::cmp::min(surrounding_verses.len(), index + 3);
        
        for i in start..end {
            let verse = &surrounding_verses[i];
            if verse.verse_number == scripture.verse_number {
                continue; // Skip the main verse we already showed
            }
            
            println!("  {}:{} {}",
                     verse.chapter_number.to_string().dimmed(),
                     verse.verse_number.to_string().dimmed(),
                     verse.scripture_text.dimmed()
            );
        }
    }
    
    println!("\n{}", "=".repeat(60).dimmed());
    
    Ok(())
}

async fn list_books(db: &ScriptureDb) -> Result<()> {
    let volumes = db.get_volumes();
    
    println!("\n{}", "📚 Available Scripture Collections".bold().blue());
    println!("{}", "=".repeat(40).dimmed());
    
    for volume in volumes {
        println!("\n{}", volume.bold().green());
        let books = db.get_books_for_volume(&volume);
        
        for book in books {
            let chapter_count = db.get_chapters_for_book(&book).len();
            println!("  • {} ({} chapters)", book, chapter_count.to_string().dimmed());
        }
    }
    
    Ok(())
}

async fn list_ollama_models() -> Result<()> {
    let ollama = OllamaClient::new("http://localhost:11434");
    
    println!("\n{}", "🤖 Available Ollama Models".bold().blue());
    println!("{}", "=".repeat(30).dimmed());
    
    match ollama.list_models().await {
        Ok(models) => {
            if models.is_empty() {
                println!("{}", "No models found. Pull a model with: ollama pull llama3.2".yellow());
            } else {
                for model in models {
                    println!("  • {}", model.green());
                }
            }
        },
        Err(e) => {
            println!("{}: {}", "Error connecting to Ollama".red(), e);
            println!("Make sure Ollama is running: {}", "ollama serve".bold());
            println!("Then pull a model: {}", "ollama pull llama3.2".bold());
        }
    }
    
    Ok(())
}