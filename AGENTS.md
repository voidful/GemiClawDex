# GemiClawDex — Development Instructions

Instructions for AI coding agents working on this codebase.

## Project Purpose

GCD is a **teaching project** for Harness Engineering. It is a Rust-native AI coding agent that combines design patterns from Claude Code, Gemini CLI, OpenAI Codex, and Hermes Agent. The primary audience is engineers learning how AI agent harnesses work.

## Project Structure

```
GemiClawDex/
├── crates/
│   ├── gcd-core/          # Domain logic library (~12,000 lines)
│   │   ├── src/
│   │   │   ├── agent.rs           # Agent execution loop (main orchestrator)
│   │   │   ├── agent/             # Adapters, memory, permissions, runtime support
│   │   │   ├── tools.rs           # Tool trait definition
│   │   │   ├── tools/             # 11 built-in tools + coordinator (930-line DAG scheduler)
│   │   │   ├── providers.rs       # Multi-provider management
│   │   │   ├── providers/         # Config loader, profiles, types
│   │   │   ├── prompt.rs          # Prompt assembly engine
│   │   │   ├── prompt/            # Command invocation, expansion
│   │   │   ├── session.rs         # Session persistence
│   │   │   ├── session/           # Model, render, storage
│   │   │   ├── trust.rs           # Three-level trust model
│   │   │   ├── skills.rs          # Skill system (YAML frontmatter + progressive disclosure)
│   │   │   ├── hooks.rs           # PreToolUse / PostToolUse lifecycle hooks
│   │   │   ├── plugins.rs         # Plugin JSON tool extension
│   │   │   ├── mcp.rs             # MCP (Model Context Protocol) client
│   │   │   ├── cache.rs           # Token cache (hash-based, TTL expiry)
│   │   │   ├── worktree.rs        # Git worktree execution isolation
│   │   │   ├── instructions.rs    # AGENTS.md / GEMINI.md / CLAUDE.md / GCD.md loader
│   │   │   ├── config.rs          # Path detection and preferences
│   │   │   ├── commands.rs        # Custom command loading
│   │   │   ├── workspace.rs       # Workspace detection
│   │   │   ├── output.rs          # Output rendering
│   │   │   └── app.rs             # Command routing facade
│   │   └── Cargo.toml
│   └── gcd-cli/           # CLI entry point → binary name: gcd
│       ├── src/main.rs     # clap 4 + rustyline REPL + colored output
│       └── Cargo.toml
├── docs/                   # Interactive teaching website
│   ├── index.html          # SPA entry point
│   ├── chapters/           # 57 HTML chapter files
│   ├── style.css           # Full CSS
│   └── script.js           # Navigation + animations
├── .gcd/                   # GCD's own harness configuration (self-referential example)
│   ├── skills/             # Reusable agent skills
│   ├── commands/           # Custom slash commands
│   ├── providers.conf      # Provider configuration
│   └── active-provider.txt # Current active provider
├── exercises/              # Hands-on exercises for learners
├── AGENTS.md               # This file
└── README.md
```

## File Dependency Chain

```
tools/registry.rs  (no deps — defines Tool trait)
       ↑
tools/*.rs  (each implements Tool trait)
       ↑
agent.rs  (orchestrates tool calls in the agent loop)
       ↑
prompt.rs  (assembles system prompt + context)
       ↑
app.rs  (command routing facade)
       ↑
gcd-cli/main.rs  (CLI entry point)
```

## Coding Conventions

1. **Keep gcd-core free of unnecessary dependencies.** Every new dependency must justify its inclusion. Prefer standard library solutions.
2. **Prefer plain-text, testable domain logic over framework-heavy abstractions.** The codebase should be readable without IDE support.
3. **Make trust and sandbox decisions explicit in the output.** When an operation is denied or requires approval, the user must see why.
4. **Treat command loading, skill loading, and prompt assembly as separate layers.** These three concerns must not be entangled.
5. **Preserve offline testability.** Core logic must be testable without network access or API keys.
6. **Document the "why", not just the "what".** Since this is a teaching project, code comments should explain design rationale, not just behavior.
7. **Reference the source product.** When a design pattern comes from Claude Code, Gemini CLI, Codex, or Hermes, note it in a comment.

## Key Design Decisions

- **Why Rust?** Single binary, memory safety, fast startup. Not because Rust is better than Python for agents, but because rebuilding in a different language forces you to understand every design choice instead of copying code.
- **Why multi-provider?** Teaching project must not be locked to one vendor. Students should compare how Gemini, OpenAI, and Anthropic handle tool calling differently.
- **Why YAML frontmatter for skills?** Compatibility with Hermes Agent's skill format. Progressive disclosure (metadata → full body → linked files) saves tokens.
- **Why separate MEMORY.md and USER.md?** MEMORY.md is environment knowledge (project facts). USER.md is personal preferences. Different update frequencies, different security concerns.

## What NOT to Do

- Do not add features just because they are "cool." Every feature must either (a) teach a harness engineering concept or (b) make the agent more reliable.
- Do not introduce breaking changes to the skill format without checking Hermes compatibility.
- Do not bypass the trust system. If a test needs trusted mode, mark it explicitly.
- Do not write documentation that claims a feature is "fully implemented" when it is a stub or partial implementation. Use honest status labels: Implemented, Partially Wired, Stub, Planned.

## Testing

```bash
cargo test                    # Run all tests
cargo test -p gcd-core        # Core library only
cargo test -p gcd-cli         # CLI only
cargo clippy                  # Lint
cargo fmt --check             # Format check
```

## Documentation Website

The `docs/` directory is a single-page app. To preview locally:

```bash
# Any static server works
python3 -m http.server 8000 --directory docs
# Then open http://localhost:8000
```

Chapter files are in `docs/chapters/`. The chapter order is defined in `docs/chapters/manifest.json`. When adding a new chapter, always update the manifest.
