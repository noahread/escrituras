---
name: scriptures-compare
version: 1.0.0
description: Compare how a topic is addressed across different scripture volumes. Use for comparative scripture study.
---

# Comparative Scripture Study

Compare how a gospel topic is taught across different scripture volumes.

## When to Use

Use when the user wants to:
- See how Old and New Testament approach a topic
- Compare Bible and Book of Mormon teachings
- Understand how doctrine develops across dispensations

## Workflow

### Step 1: Identify the topic

Extract the main topic from the user's request.

### Step 2: Search each volume

Use `mcp__scriptures__search_scriptures` separately for each volume focus:
1. Search with query: "[topic] Old Testament" (limit: 3)
2. Search with query: "[topic] New Testament" (limit: 3)
3. Search with query: "[topic] Book of Mormon" (limit: 3)
4. Search with query: "[topic] Doctrine and Covenants" (limit: 3)

Note: The semantic search will prioritize verses from the mentioned volume.

### Step 3: Analyze differences and similarities

For each volume with results:
- Quote the most relevant verse
- Note the unique emphasis or perspective
- Identify vocabulary differences

### Step 4: Synthesize

Provide a brief synthesis:
- Common threads across all volumes
- Unique contributions from each
- How later revelations build on earlier ones

## Output Format

**[Topic] Across the Standard Works**

**Old Testament**
> [Key verse with reference]
*Emphasis*: [How OT approaches this topic]

**New Testament**
> [Key verse with reference]
*Emphasis*: [How NT approaches this topic]

**Book of Mormon**
> [Key verse with reference]
*Emphasis*: [How BoM approaches this topic]

**Doctrine and Covenants**
> [Key verse with reference]
*Emphasis*: [How D&C approaches this topic]

**Synthesis**: [How the volumes complement each other]
