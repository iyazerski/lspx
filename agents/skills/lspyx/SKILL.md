---
name: lspyx
description: "Use `lspyx` CLI for semantic Python navigation with task-shaped commands: find-symbol, goto, usages, inspect, and outline"
---

# Lspyx

Use `lspyx` for precise Python symbol navigation.

## Workflow

1. If you only know a name, start with `find-symbol <query>`.
2. Then run the narrow semantic command that answers the question.
3. Fall back to `rg` only when `lspyx` is unavailable, unsupported, or the task is not semantic navigation.

## Command Choice

- `find-symbol`: find candidate symbols by name across the workspace.
- `goto`: jump to a definition, declaration, or type from a position.
- `usages`: find usages from a position.
- `inspect`: identify the symbol under a cursor and read hover details.
- `outline`: inspect file structure, either bounded or full.

## Rules

- If you are targeting a different repo than the current working directory, pass `--workspace /abs/path/to/repo`.
- Use `--format count` for cardinality questions.
- Use `--format paths` when file names are enough.
- Use `outline --depth N` for structure and `outline --full` only when the complete symbol tree matters.
- Keep queries narrow: resolve the symbol first, then inspect exact locations.
