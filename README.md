# Scripture CLI

A command-line tool for browsing LDS scriptures and querying with Ollama.

## Features

- 📖 **Browse scriptures** by volume → book → chapter
- 🔍 **Search scripture text** with flexible queries  
- 🤖 **Query Ollama** with scripture context for AI-powered insights
- 📚 **List all available** books and chapters

## Installation

```bash
# Build the CLI
cargo build --release

# Optional: Install globally  
cargo install --path .
```

## Usage

### List all available scriptures
```bash
./target/debug/scripture list
```

### Search scripture text
```bash
# Basic search
./target/debug/scripture search "faith"

# Limit results
./target/debug/scripture search "faith" --limit 3
```

### Browse scriptures interactively
```bash
./target/debug/scripture browse
```
This opens an interactive menu to navigate: Volume → Book → Chapter

### Query with Ollama
```bash
# First, make sure Ollama is running
ollama serve

# Query with scripture context
./target/debug/scripture query "What does faith mean?" --context "faith" --model llama2

# Query without context
./target/debug/scripture query "Explain the creation story" --model llama2
```

## Data Source

Uses the comprehensive LDS Scripture Database (2020.12.08) containing:
- **Old Testament** (39 books)
- **New Testament** (27 books) 
- **Book of Mormon** (15 books)
- **Doctrine and Covenants** (138 sections)
- **Pearl of Great Price** (5 books)

**Total: 41,995+ verses** across all standard works.

## Ollama Integration

The CLI integrates with [Ollama](https://ollama.ai) for AI-powered scripture study:

1. **Install Ollama**: Follow instructions at https://ollama.ai
2. **Start Ollama**: Run `ollama serve`
3. **Pull a model**: `ollama pull llama2` (or any preferred model)
4. **Query with context**: Use `--context` to include relevant scriptures

### Example Workflow
```bash
# Search for verses about faith
./target/debug/scripture search "faith" --limit 3

# Ask AI about faith with scripture context
./target/debug/scripture query "How can I increase my faith?" --context "faith" --model llama2
```

## Available Commands

| Command | Description | Example |
|---------|-------------|---------|
| `list` | Show all volumes, books, and chapters | `scripture list` |
| `search <query>` | Search scripture text | `scripture search "love"` |
| `browse` | Interactive scripture browser | `scripture browse` |
| `query <question>` | Ask Ollama with optional context | `scripture query "What is charity?" --context "charity"` |

## Tips

- Use quotes for multi-word searches: `"love one another"`
- The `--context` flag finds relevant verses to include with your question
- Interactive browsing (`browse`) is great for reading complete chapters
- Search is case-insensitive and matches verse text, titles, and book names