# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build and run (defaults to TUI)
cargo build
cargo run                    # TUI mode
cargo run -- --mcp           # MCP server mode

# Build specific crates
cargo build -p escrituras-core
cargo build -p escrituras-tui
cargo build -p escrituras-tauri

# Tests
cargo test                   # All tests (core + TUI)
cargo test -p escrituras-core   # Core library tests only
cargo test test_name         # Single test
cargo test -- --nocapture    # With output

# Release build
cargo build --release -p escrituras-tui   # Binary at target/release/scriptures

# Test MCP server manually
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- --mcp
```

## Architecture

### Workspace Structure

The project is organized as a Cargo workspace with three crates:

```
crates/
├── escrituras-core/     # Shared library (no UI dependencies)
│   └── src/
│       ├── lib.rs       # Public exports
│       ├── scripture.rs # Scripture data, search, reference extraction
│       ├── embeddings.rs # Semantic search (ONNX model)
│       ├── config.rs    # Configuration persistence
│       ├── provider.rs  # AI provider enum
│       ├── mcp.rs       # MCP server implementation
│       ├── state.rs     # UI-agnostic types (ChatMessage, ChatRole)
│       └── ai/          # AI provider clients
│           ├── claude.rs
│           ├── openai.rs
│           └── ollama.rs
│
├── escrituras-tui/      # Terminal UI (ratatui)
│   └── src/
│       ├── main.rs      # Entry point, mode dispatch
│       ├── app.rs       # TUI-specific state (ListState, Rect)
│       ├── handler.rs   # Keyboard/mouse handling
│       ├── ui.rs        # Ratatui rendering
│       └── tui.rs       # Terminal setup/teardown
│
└── escrituras-tauri/    # Tauri desktop app (scaffolding)
    └── src/
        └── main.rs      # Tauri commands wrapping core
```

### Execution Modes

**Two modes from the TUI binary (`scriptures`):**
- `scriptures` → TUI mode (ratatui-based interactive interface)
- `scriptures --mcp` → MCP server mode (JSON-RPC over stdio for AI assistants)

### Core Data Flow

1. `scripture.rs` - Loads JSON scripture data, builds indexes, provides search with stemming
2. `embeddings.rs` - Loads precomputed embeddings (.npy), runs local ONNX model (BGE-small-en-v1.5) for semantic search
3. Combined results: MCP/TUI search merges semantic + keyword results, deduplicating by verse title

### Key Types (from escrituras-core)

- `ScriptureDb` - Scripture data and search
- `EmbeddingsDb` - Semantic search engine
- `Scripture`, `ScriptureRange` - Data structures
- `ChatMessage`, `ChatRole` - UI-agnostic chat types
- `ClaudeClient`, `OpenAIClient`, `OllamaClient` - AI providers
- `Config`, `Provider` - Configuration

## Skill Versioning

**Always bump the version in `skills/scriptures-*/SKILL.md` when modifying skills.**

| Change | Bump |
|--------|------|
| Bug fix, typo | Patch (0.0.X) |
| New step, format change | Minor (0.X.0) |
| Breaking change | Major (X.0.0) |

## Shell Script Compatibility

Scripts must work in both **bash** and **zsh** (macOS default).

**Avoid:**
- `read -p "prompt"` → use `printf "prompt"; read VAR < /dev/tty`
- `[[ $VAR =~ regex ]]` → use `[ "$VAR" = "y" ]` for simple checks
- `[[ string == *glob* ]]` → use `echo | grep -qF` instead

**Critical:** `((count++))` returns 0 when count=0, causing `set -e` exit. Use `((count++)) || true`.

## MCP Tools

The server exposes 5 tools via `crates/escrituras-core/src/mcp.rs`:
- `lookup_verse` - Get verse by reference (e.g., "John 3:16", "1 Nephi 3:7")
- `lookup_chapter` - Get all verses in a chapter
- `search_scriptures` - Combined semantic + keyword search
- `get_context` - Get surrounding verses
- `list_books` - List books/volumes

## Data Files

Scripture data and embeddings are loaded from:
1. Local `lds-scriptures-2020.12.08/` and `data/` (development)
2. `~/.config/escrituras/` (installed via `install.sh`)

To regenerate embeddings:
```bash
pip install fastembed numpy
python scripts/generate_embeddings.py
```
