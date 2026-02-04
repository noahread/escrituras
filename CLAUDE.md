# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
# Build and run
cargo build
cargo run                    # TUI mode
cargo run -- --mcp           # MCP server mode

# Tests
cargo test                   # All tests
cargo test test_name         # Single test
cargo test -- --nocapture    # With output

# Release build
cargo build --release        # Binary at target/release/scriptures

# Test MCP server manually
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- --mcp
```

## Architecture

**Two execution modes from a single binary:**
- `scriptures` → TUI mode (ratatui-based interactive interface)
- `scriptures --mcp` → MCP server mode (JSON-RPC over stdio for AI assistants)

**Core data flow:**
1. `scripture.rs` - Loads JSON scripture data, builds indexes, provides search with stemming
2. `embeddings.rs` - Loads precomputed embeddings (.npy), runs local ONNX model (BGE-small-en-v1.5) for semantic search
3. Combined results: MCP/TUI search merges semantic + keyword results, deduplicating by verse title

**TUI modules:**
- `app.rs` - Application state (screens, navigation, chat history, providers)
- `handler.rs` - Keyboard/event handling
- `ui.rs` - Ratatui rendering
- `tui.rs` - Terminal setup/teardown

**AI providers** (`claude.rs`, `openai.rs`, `ollama.rs`):
- All implement streaming responses
- Config stored at `~/.config/escrituras/config.json`

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

The server exposes 5 tools via `src/mcp.rs`:
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
