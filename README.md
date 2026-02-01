# Stick of Joseph, Stick of Judah

A terminal user interface (TUI) for scripture study with AI-powered insights. Browse, search, and explore the scriptures with support for multiple AI providers.

## Features

- **Browse Scriptures**: Navigate by volume, book, and chapter with verse selection
- **Full-text Search**: Search across all 41,995+ verses instantly
- **AI Chat Mode**: Ask questions with scripture context using Claude, OpenAI, or Ollama
- **Multi-Provider AI**: Switch between AI providers seamlessly
- **Context Building**: Add verses to your session context for AI-informed responses
- **Scripture References**: AI responses include clickable scripture references

## Installation

### Quick Install (macOS/Linux)

```bash
curl -sSL https://raw.githubusercontent.com/noahread/escrituras/main/install.sh | bash
```

This installs the `scriptures` binary to `~/.local/bin`. Make sure it's in your PATH.

### Manual Install

Download the latest release from [GitHub Releases](https://github.com/noahread/escrituras/releases) and extract it to a directory in your PATH.

### From Source

```bash
git clone https://github.com/noahread/escrituras
cd escrituras
cargo build --release
cp target/release/scriptures ~/.local/bin/
```

## AI Provider Setup

The app supports three AI providers. Configure at least one:

### Ollama (Local, Free)
```bash
# Install Ollama from https://ollama.ai
ollama pull llama3.2

# No API key needed - runs locally
```

### Claude (Anthropic)
```bash
# Set environment variable
export ANTHROPIC_API_KEY="your-api-key"

# Or enter the key in the app when prompted
```

### OpenAI
```bash
# Set environment variable
export OPENAI_API_KEY="your-api-key"

# Or enter the key in the app when prompted
```

## Usage

Launch the app:
```bash
scriptures
```

### Modes

| Key | Mode | Description |
|-----|------|-------------|
| `b` | Browse | Navigate volumes, books, chapters, and verses |
| `s` | Search | Full-text search across all scriptures |
| `a` | AI Chat | Ask questions with AI and scripture context |

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | Select / Expand |
| `Backspace` | Go back |
| `Tab` | Cycle focus between panels |
| `q` | Quit |

### AI Mode

| Key | Action |
|-----|--------|
| `i` | Enter input mode |
| `Esc` | Exit input mode |
| `Enter` | Submit question |
| `x` | Add selected verse to context |
| `X` | View/manage context |
| `M` | Change AI model |
| `P` | Change AI provider |

### Scripture Selection

| Key | Action |
|-----|--------|
| `v` | Start verse selection (in Browse mode) |
| `v` | End selection and add range to context |
| `Esc` | Cancel selection |

## Scripture Database

Includes the complete LDS Standard Works:

- **Old Testament** (39 books)
- **New Testament** (27 books)
- **Book of Mormon** (15 books)
- **Doctrine and Covenants** (138 sections)
- **Pearl of Great Price** (5 books)

**Total: 41,995+ verses**

## Configuration

Settings are stored in `~/.config/escrituras/config.json`:

```json
{
  "provider": "claude",
  "default_model": "claude-sonnet-4-20250514",
  "claude_api_key": "...",
  "openai_api_key": "..."
}
```

Environment variables take precedence over config file values.

## Building from Source

```bash
git clone https://github.com/noahread/escrituras
cd escrituras
cargo build --release

# Binary will be at ./target/release/scriptures
```

## License

MIT
