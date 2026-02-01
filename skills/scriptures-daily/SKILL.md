---
name: scriptures-daily
version: 1.0.0
description: Suggest a scripture for daily inspiration. Use for morning devotional or daily scripture selection.
---

# Daily Scripture

Provide a thoughtful scripture for daily inspiration and brief reflection.

## When to Use

Use when the user wants:
- A scripture to start their day
- Daily inspiration
- A verse for a specific need

## Workflow

### Step 1: Determine focus

If user provides a topic:
- Use that topic for search

If no topic provided:
- Consider current context (day of week, season)
- Default to uplifting, encouraging themes

### Step 2: Find scripture

Use `mcp__scriptures__search_scriptures` with:
- query: The topic or "encouragement hope strength"
- limit: 5

Select one scripture that:
- Is memorable and quotable
- Provides practical encouragement
- Invites action or reflection

### Step 3: Get minimal context

Use `mcp__scriptures__get_context` with before: 1, after: 0 to understand setting.

### Step 4: Provide brief devotional

Create a short devotional (2-3 min read) with:
- The scripture
- Brief context (1-2 sentences)
- One reflection thought
- One invitation for the day

## Output Format

# Daily Scripture

**[Reference]**

> [Scripture text]

**Context**: [1-2 sentence setting]

**Reflection**: [Brief thought on meaning]

**Today's Invitation**: [One simple thing to do or ponder]

---

*"[Short memorable phrase from verse]"*
