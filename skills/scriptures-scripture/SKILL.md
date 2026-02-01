---
name: scriptures-scripture
version: 1.0.0
description: Look up scripture verses by reference or search for verses by topic. Use when user mentions a scripture reference or asks to find scriptures.
---

# Scripture Lookup

Look up specific scripture references or search for scriptures on a topic.

## When to Use

Use this skill when the user:
- Mentions a scripture reference (e.g., "John 3:16", "1 Nephi 3:7")
- Asks to find scriptures on a topic
- Wants to see the text of a verse

## Workflow

### Step 1: Determine lookup type

If the argument looks like a reference (book + chapter:verse pattern):
- Use `mcp__scriptures__lookup_verse` with the reference

If the argument is a topic or phrase:
- Use `mcp__scriptures__search_scriptures` with query, limit: 5

### Step 2: Get context (for single verse lookups)

For single verse references, also call `mcp__scriptures__get_context` with before: 1, after: 1 to show surrounding verses.

### Step 3: Format output

Present the scripture(s) clearly:
- Bold the verse title
- Show the scripture text
- For context, mark the referenced verse with >>> and show surrounding verses indented

## Examples

```
/scripture Mosiah 2:17
/scripture "faith without works"
/scripture D&C 4
```
