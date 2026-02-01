---
name: scriptures-ponder
version: 1.0.0
description: Deep scripture study with analysis, context, and reflection questions. Use for personal study and meditation.
---

# Deep Scripture Study

Facilitate deep study of a scripture passage with historical context, doctrinal analysis, and personal reflection.

## When to Use

Use when the user wants to:
- Study a passage in depth
- Understand difficult verses
- Meditate on scripture meaning
- Prepare for personal revelation

## Workflow

### Step 1: Get the full passage

Use `mcp__scriptures__lookup_chapter` or `mcp__scriptures__lookup_verse` to get the text.

For chapters, use `mcp__scriptures__get_context` with larger before/after values to see flow.

### Step 2: Provide context

Explain:
- Who is speaking/writing
- Historical setting
- Audience being addressed
- Place in larger narrative

### Step 3: Identify key elements

Break down the passage:
- Key vocabulary (Hebrew/Greek roots if relevant)
- Literary structure (chiasmus, parallelism)
- Doctrinal teachings
- Symbolic elements

### Step 4: Find cross-references

Use `mcp__scriptures__search_scriptures` to find:
- Related teachings elsewhere in scripture
- Modern revelation that expands on the topic
- Christ-centered connections

### Step 5: Reflection questions

Provide 3-5 pondering questions that:
- Invite personal application
- Encourage spiritual insight
- Connect to current life situations

## Output Format

# Pondering: [Reference]

## The Text
> [Full scripture text]

## Context
**Speaker/Author**: [Who]
**Setting**: [When and where]
**Audience**: [To whom]
**Purpose**: [Why this was recorded]

## Analysis

### Key Phrases
- **"[phrase]"**: [Meaning and significance]
- **"[phrase]"**: [Meaning and significance]

### Structure
[Literary elements, patterns]

### Doctrines Taught
1. [Doctrine 1]
2. [Doctrine 2]

## Cross-References
- [Reference]: [Connection]
- [Reference]: [Connection]

## Pondering Questions
1. [Personal reflection question]
2. [Application question]
3. [Invitation to revelation question]

## Invitation
[Suggestion for meditation, prayer, or action]
