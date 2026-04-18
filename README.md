# Symbiote

**Single Source of Truth for AI coding agent instructions.**

Symbiote discovers, syncs, and translates project rules across AI coding agents. Write your instructions once — Symbiote keeps every agent in sync.

## Supported Agents

| Agent | Config Files |
|-------|-------------|
| **GitHub Copilot** | `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md` |
| **Claude Code** | `CLAUDE.md`, `.claude/rules/*.md` |
| **Cursor** | `.cursorrules` (legacy, read-only), `.cursor/rules/*.mdc` |
| **Windsurf** | `.windsurf/rules/*.md` |
| **Gemini CLI** | `GEMINI.md` |
| **OpenCode** | `OpenCode.md` |

## Installation

### From GitHub Releases

Download the latest binary for your platform from the [Releases](https://github.com/sathyanarrayanan/symbiote/releases) page.

```sh
# macOS (Apple Silicon)
curl -LO https://github.com/sathyanarrayanan/symbiote/releases/latest/download/symbiote-aarch64-apple-darwin.tar.gz
tar xzf symbiote-aarch64-apple-darwin.tar.gz
sudo mv symbiote /usr/local/bin/

# macOS (Intel)
curl -LO https://github.com/sathyanarrayanan/symbiote/releases/latest/download/symbiote-x86_64-apple-darwin.tar.gz
tar xzf symbiote-x86_64-apple-darwin.tar.gz
sudo mv symbiote /usr/local/bin/

# Linux (x86_64)
curl -LO https://github.com/sathyanarrayanan/symbiote/releases/latest/download/symbiote-x86_64-unknown-linux-gnu.tar.gz
tar xzf symbiote-x86_64-unknown-linux-gnu.tar.gz
sudo mv symbiote /usr/local/bin/
```

### From Source

Requires [Rust](https://rustup.rs/) 1.85+ (edition 2024).

```sh
git clone https://github.com/sathyanarrayanan/symbiote.git
cd symbiote
cargo install --path .
```

## Quick Start

```sh
# 1. Initialize checksum tracking
symbiote init

# 2. See which agent configs exist in your project
symbiote detect

# 3. Sync the leader (most recently edited) to all other agents
symbiote sync

# 4. Verify all agents have the same instructions
symbiote diff
```

## CLI Reference

### `symbiote detect`

Scans the project for all known agent configuration files and identifies the **leader** — the most recently modified config.

```
$ symbiote detect
Detected agent configurations:

  ★ CLAUDE.md (claude) — just now [LEADER]
  ● .github/copilot-instructions.md (copilot) — 2h ago
  ● .cursor/rules/general.mdc (cursor) — 3d ago

The leader is the most recently modified config and will be used as the source of truth.
```

### `symbiote sync`

Syncs the leader config to all follower agents. Each agent's output is written in its native format.

```sh
# Sync leader to all other agents
symbiote sync

# Sync from a specific agent to another
symbiote sync --from claude --to copilot
```

```
$ symbiote sync
Leader: CLAUDE.md (claude)
  write .github/copilot-instructions.md (copilot)
  write .cursor/rules/general.mdc (cursor)
  write .windsurf/rules/general.md (windsurf)
  write GEMINI.md (gemini)
  write OpenCode.md (opencode)

Done: 5 written, 0 skipped, 0 errors
```

Files that haven't changed since the last sync are skipped automatically (checksum-based).

### `symbiote diff`

Shows how instructions differ between agents. Compares the parsed rules in a canonical format, not raw file contents.

```sh
# Diff all detected agents pairwise
symbiote diff

# Diff two specific agents
symbiote diff claude copilot
```

```
$ symbiote diff claude copilot
--- Diff: claude vs copilot
  Identical.
```

### `symbiote init`

Initializes the `.symbiote/` directory for checksum tracking. Run this once per project.

```sh
$ symbiote init
✓ Initialized .symbiote directory.
  Add .symbiote/ to your .gitignore if desired.
```

## How It Works

1. **Discovery** — Symbiote scans the project root for all known agent config file patterns.

2. **Leader Election** — The most recently modified config file is designated the **leader**. This is the source of truth for syncing.

3. **Universal IR** — The leader's content is parsed into a Universal Intermediate Representation that captures:
   - **Project context** — high-level project description
   - **Coding standards** — list of rules/guidelines
   - **Scoped rules** — file-pattern-specific instructions (e.g., "for `*.test.ts`, use vitest")

4. **Translation** — The IR is serialized into each target agent's native format:
   - Copilot gets Markdown with `applyTo` frontmatter for scoped rules
   - Cursor gets `.mdc` files with `globs`/`alwaysApply` frontmatter
   - Windsurf gets `.md` files with `trigger`/`globs` frontmatter
   - Gemini and OpenCode get plain Markdown
   - Claude gets Markdown with `paths` frontmatter for scoped rules

5. **Checksum Skip** — SHA-256 checksums of written files are stored in `.symbiote/checksums.json`. On subsequent syncs, unchanged files are skipped to prevent unnecessary writes and circular updates.

## Project Structure

```
src/
├── main.rs          # CLI entry point and command handlers
├── cli.rs           # Clap command definitions
├── ir.rs            # Universal Intermediate Representation
├── agent.rs         # AgentConfig trait and registry
├── frontmatter.rs   # YAML frontmatter parser/serializer
├── discovery.rs     # Project scanning and leader election
├── checksums.rs     # SHA-256 checksum registry
├── sync.rs          # Sync engine
├── diff.rs          # Diff engine
└── agents/
    ├── copilot.rs   # GitHub Copilot
    ├── claude.rs    # Claude Code
    ├── cursor.rs    # Cursor
    ├── windsurf.rs  # Windsurf
    ├── gemini.rs    # Gemini CLI
    └── opencode.rs  # OpenCode
```

## License

MIT
