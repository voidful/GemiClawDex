# GemiClawDex (GCD)

> Learn Harness Engineering by building an AI coding agent from scratch in Rust.

[繁體中文版 README](README.md)

## What This Project Teaches

You've probably used Claude Code, Gemini CLI, or OpenAI Codex to write code. But have you ever wondered: **how do they actually work?**

GCD is a Rust-native AI coding agent. It is not meant to replace those tools. Instead, it takes the best design patterns from all four major sources and makes them readable, so you can learn how the "harness" around an AI model works.

**Harness** is the software layer that wraps around an AI model. It handles prompt assembly, tool management, permission control, session persistence, and provider routing. The model is the engine. The harness is the steering wheel, brakes, and dashboard.

## Why Learn Here

| Feature | Description |
|---------|-------------|
| **Not a lecture about other products** | GCD itself IS the product. 13,000+ lines of Rust, fully open source |
| **Not locked to one vendor** | Supports Gemini / OpenAI / Anthropic / OpenRouter / local models |
| **Combines four sources** | Every design decision is traceable to its origin product |
| **Interactive documentation** | 61 HTML chapters, from beginner to source code walkthrough |
| **Skill learning loop** | Inspired by [Hermes Agent](https://github.com/NousResearch/hermes-agent), agents create reusable skills from experience |
| **Dual memory system** | MEMORY.md (environment knowledge) + USER.md (user preferences), with injection safety scanning |

## Interactive Documentation (Traditional Chinese)

🌐 **[eric-lam.com/GemiClawDex](http://eric-lam.com/GemiClawDex/)**

No Rust knowledge required. The documentation is a standalone harness engineering learning resource.

### Learning Paths

| Time | Path | For |
|------|------|-----|
| 3 min | Home → Intro → Quick Start | First contact with agent concepts |
| 10 min | + Glossary → Context Engineering → Architecture | Understanding prompt / context / harness differences |
| 30 min | + Pipeline → Detail chapters | Full input-to-execution flow |
| 1 hour | + Comparison → Source code | Ready to modify code or build your own agent |
| Hands-on | AGENTS.md Workshop + Exercises | Writing harnesses for your own projects |

## What GCD Learned From Each Source

| Source | Design Patterns | Source Code |
|--------|----------------|-------------|
| **Claude Code** | Prompt assembly, Tool trait, Trust boundaries, Skill system | `prompt.rs`, `tools.rs`, `trust.rs`, `skills.rs` |
| **Gemini CLI** | Terminal-first REPL, MCP client, Token caching, Streaming | `main.rs`, `mcp.rs`, `cache.rs`, `output.rs` |
| **OpenAI Codex** | Sandbox levels, Permission model, apply-patch | `tools/container.rs`, `agent/permissions.rs`, `tools/apply_patch.rs` |
| **Hermes Agent** | Skill learning loop, Dual memory, Memory safety scan, Session search | `skills.rs`, `tools/memory_tool.rs`, `tools/skill_manager.rs` |

## Quick Start

```bash
git clone https://github.com/voidful/GemiClawDex.git && cd GemiClawDex
cargo build --release

# Set at least one API key
export GEMINI_API_KEY="AIza..."      # Highest free quota

# Start interactive REPL
./target/release/gcd

# Or run a single task
gcd exec "Explain the architecture of this codebase"
```

## Built-in Skills (10)

GCD's `.gcd/skills/` directory contains reusable agent skills in YAML frontmatter format (compatible with [Hermes Agent](https://github.com/NousResearch/hermes-agent)):

code-review, systematic-debugging, tdd, writing-plans, security-audit, refactoring, documentation-review, git-workflow, performance-analysis, architecture-review

## Architecture

```
crates/
├── gcd-core/          # Core logic library (~12,000 lines)
│   ├── agent.rs       # Agent execution loop + Permission + Streaming + Memory
│   ├── tools/         # 11 built-in tools + coordinator (930-line DAG scheduler)
│   ├── providers.rs   # Multi-provider management (Gemini / OpenAI / Anthropic)
│   ├── prompt.rs      # Prompt assembly engine
│   ├── session.rs     # Session persistence
│   ├── trust.rs       # Three-level trust model
│   ├── skills.rs      # Skill system + YAML frontmatter + Progressive Disclosure
│   ├── mcp.rs         # MCP client
│   └── hooks.rs       # PreToolUse / PostToolUse lifecycle hooks
├── gcd-cli/           # CLI entry point
│   └── main.rs        # clap 4 + rustyline REPL
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Contributions in both Traditional Chinese and English are welcome.

## License

Apache-2.0
