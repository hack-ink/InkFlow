# Documentation Rules

This document defines the required format and conventions for adding documentation under `spec/`, including `spec/` and `spec/context/`.

## Global Requirements

- Write all documentation in clear, grammatically correct English.
- Start sentences with a capital letter and end with proper punctuation.
- Do not include secrets (API keys, tokens), personal data, or machine-specific paths.
- Prefer concise, retrieval-friendly summaries over long transcripts or logs.
- Use copy-pasteable command blocks when documenting commands.
- When renaming or moving documentation files, update all references in other documentation.

## Directory Layout

- `spec/` contains the AiR planning pack (architecture, contracts, configuration, and work plans).
- `spec/archive/` contains historical or development-process documents that should be retained but are not actively maintained.
- `spec/context/` contains compressed context digests designed for fast recall after context limits.

## File Naming

### Sparse-Numbered Documents (`spec/`)

- Use `NN_title.md`, where `NN` is a two-digit sparse number that controls reading order.
- Prefer increments of 10 (for example, `10`, `20`, `30`) to leave room for future inserts (for example, `25`).
- Use lowercase `snake_case` for `title`.

Index convention:

- Use `spec/00_index.md` as the canonical entry point for readers.
- If `spec/README.md` exists, keep it small and point it to `spec/00_index.md`.

Recommended ranges:

- `10`–`39`: Architecture and contracts.
- `40`–`69`: Implementation notes and configuration.
- `70`–`79`: Milestones and acceptance criteria.
- `80`–`89`: Working checklists that may drift over time.
- Prefer placing archived documents under `spec/archive/` instead of using `90`–`99` prefixes.

### Context Digests (`spec/context/`)

- Use `NN_YYYY-MM-DD_to_YYYY-MM-DD_digest.md`.
- `NN` is a zero-based, two-digit index (`00`, `01`, `02`, ...).
- Windows are rolling 7-day ranges starting from the first recorded entry date in the series.

Window selection algorithm:

1. Determine `first_date` as the earliest date covered by the first digest file.
2. For a new entry on `entry_date`, compute `index = floor((entry_date - first_date) / 7)`.
3. The window range is:
   - `start = first_date + index * 7 days`.
   - `end = start + 6 days`.
4. If the digest file for that window does not exist, create it with the next `NN` index.

## Cross-Linking

- Prefer repo-root-relative paths in Markdown links and references (for example, `spec/50_speech_to_text.md`).
- Avoid relative links that depend on the current file location.

## Templates

### New `spec/NN_title.md`

Use a consistent structure:

- One-sentence purpose at the top.
- Short sections with clear headings.
- If the document may drift, add a status line near the top:
  - `Status: Working document. It may drift over time.`

### New Context Entry (Append to the Correct Digest Window)

Add the entry under the appropriate day section (for example, `## 2025-12-22`) with a short heading and the following bullets:

- Goal: One or two sentences describing what changed and why.
- Summary: Three to six bullets describing the changes at a high level.
- Key files: A short list of file paths that were modified or are most relevant.
- Validation: List the commands that were run, if any.

Keep the entry small and prefer linking to canonical docs instead of duplicating large content.
