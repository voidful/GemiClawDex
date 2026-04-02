# GemiClawdex Architecture

## Design goal

Build a Rust-native coding agent that keeps the best interaction patterns from Gemini CLI, OpenAI Codex, and this repository's existing skill system, without inheriting the full complexity of the current TypeScript runtime.

## Core product bets

1. Use Rust for the orchestration layer.
2. Keep the product terminal-first and workspace-aware.
3. Treat repository instructions, custom commands, and skills as separate but composable layers.
4. Make trust and sandbox policy first-class, not bolt-ons.
5. Decouple prompt assembly from provider execution so the system stays testable offline.
6. Keep headless and scripted use cases first-class through stable JSON output and saved sessions.

## Crates

### `gemi-clawdex-core`

Owns the domain logic:

- workspace discovery
- provider capabilities
- provider registry and active-profile switching
- environment-driven provider overlays
- trust store evaluation
- custom command loading
- skill loading
- instruction loading
- prompt assembly
- checkpoint persistence
- saved sessions with resume/fork semantics
- structured JSON rendering support

### `gemi-clawdex-cli`

Owns argument parsing and text rendering:

- `providers`
- `commands reload`
- `trust`
- `checkpoints list`
- `sessions list/show/fork`
- `exec`

## Configuration model

`GemiClawdex` uses a blended filesystem layout:

- global home: `~/.gemi-clawdex/`
- workspace config: `<repo>/.gemi-clawdex/`
- provider profiles: `providers.conf`
- active provider pins: `active-provider.txt`
- saved sessions: `~/.gemi-clawdex/sessions/<session-id>/`
- repo instructions: `<repo>/AGENTS.md`
- optional persistent context: `<repo>/GEMINI.md`, `<repo>/CLAUDE.md`, `<repo>/GEMICLAWDEX.md`

This intentionally mirrors:

- Gemini CLI's workspace-level config and command discovery
- Codex's `AGENTS.md`
- Claude-style skill packs
- CC Switch's idea of managing multiple provider profiles without editing live API config by hand
- SDK-style agents that keep reusable session state instead of treating every prompt as isolated
- headless-compatible forks that prefer environment variables over hard-wired provider assumptions

## Execution pipeline

1. Discover workspace root.
2. Evaluate folder trust.
3. Load repo instructions if trust permits.
4. Load provider profiles and resolve the active provider.
5. Load commands and skills if trust permits.
6. Resolve any requested session resume/fork context.
7. Resolve the requested command or raw task.
8. Expand `@{...}` file or directory injections.
9. Substitute command arguments.
10. Detect `!{...}` shell blocks and convert them into approval requirements.
11. Assemble the final provider-ready prompt.
12. Inject summarized session lineage when resuming or forking.
13. Optionally checkpoint the assembled session.
14. Persist the session turn unless the run is explicitly ephemeral.

## Planned next steps

- Live provider adapters for Gemini, OpenAI Responses/Codex, and Anthropic
- interactive TUI
- tool registry and permission prompts
- richer session replay and transcript diffing
- MCP client integration
