use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{Select, theme::ColorfulTheme};
use anyhow::Result;

mod scripture;
mod ollama;

use scripture::{ScriptureDb, Scripture};
use ollama::OllamaClient;

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
            query_ollama(&db, &question, context.as_deref(), &model).await?
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