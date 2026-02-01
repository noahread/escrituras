---
name: scriptures-topical
version: 1.0.0
description: Find scriptures related to a concept or topic using semantic search. Use for deep topical study.
---

# Topical Scripture Study

Find scriptures conceptually related to a topic, even if they don't contain the exact words.

## When to Use

Use when the user wants to:
- Study a gospel topic in depth
- Find scriptures related to an idea or concept
- Explore themes across the standard works

## Workflow

### Step 1: Semantic search

Use `mcp__scriptures__search_scriptures` with:
- query: The topic/concept
- limit: 15

This combines semantic and keyword search, returning conceptually related verses first.

### Step 2: Group by volume

Organize results by volume (Old Testament, New Testament, Book of Mormon, D&C, Pearl of Great Price) to show breadth of topic coverage.

### Step 3: Provide insights

After listing scriptures:
- Identify common themes across the results
- Note any interesting connections between books
- Suggest related topics to explore

## Output Format

**Scriptures on [Topic]**

**Book of Mormon**
- 1 Nephi 3:7 - "I will go and do..."
- Mosiah 4:9 - "Believe in God..."

**New Testament**
- Matthew 5:3 - "Blessed are the poor in spirit..."

**Themes**: [Brief analysis of common threads]

**Related Topics**: [2-3 related topics to explore]
