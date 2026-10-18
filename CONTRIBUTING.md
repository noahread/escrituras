# Contributing to Scriptures CLI

This document is for maintainers and AI agents working on this project.

## Workspace Architecture

The project is organized as a Cargo workspace with three crates:

| Crate | Path | Purpose |
|-------|------|---------|
| `escrituras-core` | `crates/escrituras-core/` | Shared library (scripture data, search, AI providers). No UI dependencies. |
| `escrituras-tui` | `crates/escrituras-tui/` | Terminal UI using ratatui |
| `escrituras-tauri` | `crates/escrituras-tauri/` | Tauri desktop app (scaffolding) |

### Adding New Features

- **Core logic** (data, search, AI): Add to `escrituras-core`
- **TUI-specific code** (rendering, keyboard): Add to `escrituras-tui`
- **Desktop app features**: Add to `escrituras-tauri`

### Key Principle

Business logic belongs in `escrituras-core` so both TUI and Tauri can use it. The TUI and Tauri crates should be thin wrappers over core functionality.

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

The MCP server (`crates/escrituras-core/src/mcp.rs`) exposes tools that skills use:

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

- [ ] Bump version in root `Cargo.toml` (`[workspace.package]` section)
- [ ] Bump any modified skill versions
- [ ] Verify shell scripts work with both bash and zsh
- [ ] CI is green on the release commit (or run `cargo test -p escrituras-core -p escrituras-tui`; plain `cargo test` only runs the TUI's tests)
- [ ] Tag release: `git tag v0.X.0 && git push --tags` (a tag like `v0.X.0-beta.1` publishes a prerelease)
- [ ] The Release workflow builds the terminal app for Linux (x86_64, ARM), macOS (Apple Silicon, Intel) and Windows, plus the Mac app's disk image, and publishes them with checksums and generated release notes
- [ ] Users get updates via `install.sh`

Pull requests that change the release workflow or the build and packaging scripts (`scripts/build-*.sh`, `scripts/package-*.sh`) run the same builds without publishing.

### Signing the Mac app

Without signing secrets the release still includes the Mac app, signed ad hoc: it works, but macOS blocks it on first open until the user allows it in System Settings. To ship a signed and notarized app, add these repository secrets (Settings → Secrets and variables → Actions). They need an [Apple Developer Program](https://developer.apple.com/programs/) membership.

| Secret | Value |
|--------|-------|
| `MACOS_SIGNING_IDENTITY` | The certificate's name, e.g. `Developer ID Application: Jane Doe (TEAMID1234)` |
| `MACOS_CERTIFICATE` | The Developer ID Application certificate and its private key, exported from Keychain Access as a `.p12` and base64-encoded: `base64 -i certificate.p12 \| pbcopy` |
| `MACOS_CERTIFICATE_PASSWORD` | The password chosen when exporting the `.p12` |
| `APPLE_ID` | The Apple Account email used for notarization |
| `APPLE_TEAM_ID` | The 10-character team ID from developer.apple.com/account |
| `APPLE_APP_PASSWORD` | An app-specific password from account.apple.com (Sign-In and Security → App-Specific Passwords) |

`scripts/package-mac-app.sh` reads the same variables, so it can also sign and notarize locally.
