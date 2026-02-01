# Contributing to Scriptures CLI

This document is for maintainers and AI agents working on this project.

## Skills System

Scripture study skills live in `skills/scriptures-*/SKILL.md`. These are installed to users' `~/.claude/skills/` directories.

### Versioning

**Every skill has a version in its YAML front matter:**

```yaml
---
name: scriptures-scripture
version: 1.0.0
description: ...
---
```

### When to Bump Versions

**You MUST bump the version when modifying any SKILL.md file.**

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Bug fix, typo, clarification | Patch (0.0.X) | 1.0.0 → 1.0.1 |
| New workflow step, output format change | Minor (0.X.0) | 1.0.1 → 1.1.0 |
| Complete rewrite, breaking changes | Major (X.0.0) | 1.1.0 → 2.0.0 |

### How It Works

1. User runs `install.sh` or `scripts/install-skills.sh`
2. Script extracts `version:` from local and remote SKILL.md files
3. Compares using semver logic
4. If remote > local, prompts user to update

### Checklist When Modifying Skills

- [ ] Made changes to `skills/scriptures-*/SKILL.md`
- [ ] Bumped `version:` field in YAML front matter
- [ ] Changes will be available after next release

### Current Skills

| Skill | File |
|-------|------|
| `/scriptures-scripture` | `skills/scriptures-scripture/SKILL.md` |
| `/scriptures-topical` | `skills/scriptures-topical/SKILL.md` |
| `/scriptures-cross-ref` | `skills/scriptures-cross-ref/SKILL.md` |
| `/scriptures-compare` | `skills/scriptures-compare/SKILL.md` |
| `/scriptures-ponder` | `skills/scriptures-ponder/SKILL.md` |
| `/scriptures-journal` | `skills/scriptures-journal/SKILL.md` |
| `/scriptures-memorize` | `skills/scriptures-memorize/SKILL.md` |
| `/scriptures-daily` | `skills/scriptures-daily/SKILL.md` |

## MCP Server

The MCP server (`src/mcp.rs`) exposes tools that skills use:

- `mcp__scriptures__lookup_verse` - Get verse by reference
- `mcp__scriptures__lookup_chapter` - Get full chapter
- `mcp__scriptures__search_scriptures` - Keyword + semantic search
- `mcp__scriptures__get_context` - Get surrounding verses
- `mcp__scriptures__list_books` - List books/volumes

If you add/modify MCP tools, update the skill documentation to use them.

## Shell Script Guidelines

All shell scripts (`install.sh`, `scripts/*.sh`) must work in both **bash** and **zsh**.

macOS Catalina+ uses zsh as the default shell, so users run:
```bash
curl -sSL .../install.sh | zsh
```

### Patterns to Avoid

| Don't use | Use instead |
|-----------|-------------|
| `read -p "prompt" VAR` | `printf "prompt"; read VAR < /dev/tty` |
| `[[ $X =~ ^[Yy]$ ]]` | `[ "$X" = "y" ] \|\| [ "$X" = "Y" ]` |
| `[[ ":$PATH:" == *":$DIR:"* ]]` | `echo ":$PATH:" \| grep -qF ":$DIR:"` |

**Important:** When scripts are piped (e.g., `curl | zsh`), stdin comes from the pipe. Use `< /dev/tty` to read from the terminal instead.

### Safe Patterns (work in both)

- Arrays: `ARR=(a b c)`, `for x in "${ARR[@]}"`
- Arithmetic: `((count++)) || true` (the `|| true` prevents `set -e` exit when count=0)
- Here-strings: `read var <<< "$string"`
- Functions with `local`
- `[[ ]]` for non-regex conditionals

**Note:** `((count++))` returns the OLD value. When count=0, it returns 0 (falsy), which causes `set -e` to exit. Always use `|| true` with arithmetic increments.

## Release Checklist

- [ ] Bump Cargo.toml version
- [ ] Bump any modified skill versions
- [ ] Verify shell scripts work with both bash and zsh
- [ ] Tag release: `git tag v0.X.0 && git push --tags`
- [ ] GitHub Actions builds and uploads binaries
- [ ] Users get updates via `install.sh`
