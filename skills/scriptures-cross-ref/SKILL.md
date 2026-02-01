---
name: scriptures-cross-ref
version: 1.0.0
description: Find scriptures related to a specific verse. Use to discover cross-references and parallel passages.
---

# Cross-Reference Finder

Find scriptures that relate to a specific verse - parallel passages, supporting scriptures, and thematic connections.

## When to Use

Use when the user has a specific verse and wants to find:
- Parallel passages in other books
- Supporting scriptures on the same theme
- Contrasting perspectives

## Workflow

### Step 1: Look up the source verse

Use `mcp__scriptures__lookup_verse` to get the full text of the reference.

### Step 2: Extract key themes

Identify 2-3 key concepts/themes from the verse text.

### Step 3: Search for related scriptures

For each key theme, use `mcp__scriptures__search_scriptures` with:
- query: The theme/concept (phrase from verse or derived concept)
- limit: 5

### Step 4: Deduplicate and categorize

Remove duplicates and categorize results:
- **Parallel Passages**: Same story/event from different books
- **Supporting Scriptures**: Verses that reinforce the message
- **Related Themes**: Verses on connected topics

### Step 5: Present with connections

For each cross-reference, briefly explain how it relates to the source verse.

## Output Format

**Cross-References for [Reference]**

*"[Source verse text]"*

**Key Themes**: [Theme 1], [Theme 2], [Theme 3]

**Parallel Passages**
- [Reference] - [Brief connection]

**Supporting Scriptures**
- [Reference] - [Brief connection]

**Related Themes**
- [Reference] - [Brief connection]
