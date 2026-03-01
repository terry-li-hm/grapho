---
title: "feat: grapho — memory system write-side CLI"
type: feat
status: active
date: 2026-03-02
---

# feat: grapho — Memory System Write-Side CLI

## Overview

Rust CLI to manage the write-side of the personal memory system: `MEMORY.md`, `memory-overflow.md`, and `~/docs/solutions/` scaffolding. Pairs with `cerno` (read) — grapho writes, cerno searches.

## Problem Statement

Currently all MEMORY.md mutations happen manually in an editor: adding entries, demoting over-budget entries to overflow, promoting them back, reviewing stale overflow content. This is friction-heavy, error-prone (wrong section, missed dedup), and not scriptable by agents (e.g. morning briefing could surface overflow entries needing promotion).

## Proposed Solution

Six subcommands covering the full write lifecycle:

| Command | Action |
|---|---|
| `grapho status` | Line count, budget remaining, section list |
| `grapho add` | Interactive: pick section, enter content, append to MEMORY.md |
| `grapho demote <search>` | Move matching entry: MEMORY.md → overflow |
| `grapho promote <search>` | Move matching entry: overflow → MEMORY.md (pick section) |
| `grapho review` | List overflow entries by age, prompt promote/keep/delete |
| `grapho solution <name>` | Scaffold `~/docs/solutions/<name>.md` with template + dedup check |

## Technical Approach

### File Paths (hardcoded defaults, overridable via env)

```
MEMORY_PATH   = ~/.claude/projects/-Users-terry/memory/MEMORY.md
OVERFLOW_PATH = ~/docs/solutions/memory-overflow.md
SOLUTIONS_DIR = ~/docs/solutions/
BUDGET        = 150
```

### Module Structure

```
src/
  main.rs     — Cli struct, subcommand dispatch, ExitCode return
  models.rs   — Section, MemoryEntry, GraphoReport structs + Serialize
  parse.rs    — MEMORY.md / overflow parser; round-trip write
  output.rs   — OutputFormat (ValueEnum), render() dispatch
  paths.rs    — resolve_memory_path(), resolve_overflow_path(), resolve_solutions_dir()
```

Single binary, no workspace. Start in one file if Codex prefers, split after.

### MEMORY.md Format (confirmed by inspection)

```markdown
# Claude Code Auto-Memory

<prose header lines>

## Section Name
- **Bold key.** Rule sentence.
- **Key:** value — description

## Another Section
- entry
```

- No YAML frontmatter
- H2 (`## ...`) = section headers only; H1 = file title
- All entries are flat bullets under H2
- No H3, no code blocks, no timestamps in entries
- Current line count: 153 (over 150 budget — live signal for status command)

Parse into: `Vec<Section { name: String, entries: Vec<String> }>`. Round-trip preserves header prose (lines before first `##`).

### Key Design Decisions

**Atomic writes:** Write to `<file>.tmp`, then `std::fs::rename()`. Never truncate-then-write — MEMORY.md corruption during a crash would be bad.

**Matching (demote/promote):** Substring match on entry content. If >1 match, print numbered list and prompt user to pick. Never silently pick wrong entry.

**Section preservation on demote:** When moving to overflow, write under `## <original-section>` in overflow (create section if missing). Preserve source provenance.

**TTY / agent-first output:** Use `std::io::IsTerminal` on stdout.
- **TTY = true (human):** coloured output via `owo-colors`, box-drawing chars for panels
- **TTY = false (piped/agent):** plain markdown — no ANSI codes, no box art
This is critical: MEMORY.md is consumed by Claude Code sessions (morning briefing, context loading). Grapho output piped to agents must be clean markdown.

**Exit codes:**
- `0` — success, no issues
- `1` — budget exceeded (scriptable: `grapho status || notify`)
- `2` — fatal error (file not found, parse failure)

Use `ExitCode` return from `main()` with inner `run() -> Result<()>` for clean error propagation.

**`--format` flag:** Use `ValueEnum` (clap-native validation, better error messages than `String + from_str`). Values: `human` (default), `json`.

**JSON output:** Separate `GraphoReport` struct (Serialize) from internal `MemoryState`. `status --format json` returns machine-readable budget info.

### Dependencies

```toml
[dependencies]
clap      = { version = "4", features = ["derive"] }
anyhow    = "1"
dirs      = "6"          # note: 6 not 5
chrono    = "0.4"
owo-colors = "4"
serde     = { version = "1", features = ["derive"] }
serde_json = "1"
```

### Rust Edition

Use `edition = "2024"` — new project, no downside.

### Regex Caveat

If parsing MEMORY.md with regex: **no lookahead/lookbehind in Rust's regex crate.** Use capture groups instead. Section headers: `^## (.+)$` is sufficient — no lookahead needed.

## Acceptance Criteria

- [ ] `grapho status` prints line count, budget remaining (150 − count), list of sections
- [ ] `grapho status --format json` returns machine-readable struct with same info; exit 1 when over budget
- [ ] `grapho add` prompts for section (numbered list), entry text; appends to correct section; writes atomically
- [ ] `grapho demote "query"` finds matching entries, disambiguates if >1, moves to overflow under correct section header
- [ ] `grapho promote "query"` finds entry in overflow, prompts for target section, moves to MEMORY.md
- [ ] `grapho review` lists all overflow entries with age (file mtime), prompts [p]romote/[k]eep/[d]elete per entry
- [ ] `grapho solution <name>` checks for existing `<name>.md` in solutions dir, scaffolds with template if absent
- [ ] All writes are atomic (temp + rename)
- [ ] Piped output (TTY=false) is clean markdown — no ANSI codes
- [ ] Exit codes: 0 success, 1 budget exceeded, 2 fatal error
- [ ] `cargo clippy` clean after implementation
- [ ] Unit tests for MEMORY.md round-trip parser (parse → write → parse = identity)

## System-Wide Impact

- **Read paths unaffected:** `cerno` reads MEMORY.md — grapho writes must preserve exact format (same H2 headings, bullet style). Round-trip test is the safety net.
- **Claude Code session context:** MEMORY.md is auto-loaded into every Claude Code session. Grapho must never corrupt this file.
- **Morning briefing script:** May call `grapho status` to surface budget alerts — must be pipe-safe.

## Dependencies & Risks

- **Codex sandbox:** cannot build in sandbox (crates.io DNS blocked). Workflow: delegate source writing to Codex, then `cargo build --release` locally.
- **Stale builds:** `cargo build` may say "Finished" without recompiling. Fix: `cargo clean -p grapho` if behaviour unchanged after edits.
- **Parser fragility:** MEMORY.md format is prose-defined, not schema-enforced. The round-trip parser must preserve all non-entry lines (header prose, blank lines between sections). Unit test against the live file.

## Implementation Order

1. `paths.rs` — path resolution (foundation for everything)
2. `parse.rs` — round-trip parser + unit tests against live MEMORY.md
3. `models.rs` — Section, GraphoReport structs
4. `output.rs` — OutputFormat, TTY detection, render()
5. `main.rs` — Cli/Command structs, dispatch to stub fns
6. `status` command
7. `add` command
8. `demote` + `promote` commands
9. `review` command
10. `solution` command

## Verification

```bash
cd ~/code/grapho && cargo build
grapho status                          # line count + sections
grapho status --format json            # machine-readable
grapho add                             # interactive
grapho demote "cargo publish"          # test with real overflow entry
grapho promote "cargo publish"         # reverse it
grapho solution test-solution          # check ~/docs/solutions/test-solution.md created
grapho review                          # list overflow
```

## Sources

- Pattern reference: `~/code/pondus/src/main.rs`, `~/code/nexis/src/main.rs`
- Agent-first CLI design: `~/docs/solutions/patterns/agent-first-cli.md`
- Rust gotchas: `~/docs/solutions/rust-gotchas.md`
- Codex delegation: `~/docs/solutions/memory-overflow.md` (Rust/Codex section)
- Toolchain setup: `~/docs/solutions/rust-toolchain-setup.md`
