# Scriptures CLI - Agent Instructions

## Important: Skill Versioning

When modifying any skill file in `skills/scriptures-*/SKILL.md`:

1. **Always bump the `version:` field** in the YAML front matter
2. Use semver: patch for fixes, minor for features, major for breaking changes
3. See `CONTRIBUTING.md` for full versioning guidelines

## Project Structure

```
src/
  main.rs      - Entry point, --mcp flag handling
  mcp.rs       - MCP server implementation
  scripture.rs - Scripture database and search
  embeddings.rs - Semantic search with local ONNX model
  app.rs       - TUI application state
  handler.rs   - TUI event handling
  ui.rs        - TUI rendering

skills/
  scriptures-*/SKILL.md - Claude Code skills (versioned)

scripts/
  install-skills.sh     - Local skill installer with version checking
  generate_embeddings.py - Python script to regenerate embeddings

install.sh - Main installer (binary + embeddings + skills)
```

## MCP Tools

The scriptures MCP server provides:
- `lookup_verse` - Get verse by reference
- `lookup_chapter` - Get full chapter
- `search_scriptures` - Combined semantic + keyword search
- `get_context` - Get surrounding verses
- `list_books` - List books/volumes

## Testing

```bash
# Test MCP server
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cargo run -- --mcp

# Run tests
cargo test
```
