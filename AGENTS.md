# GemiClawdex Instructions

This repository is building a Rust-native coding agent that blends:

- Gemini-style command ergonomics
- Codex-style repository instructions and agent workflow
- Claude-style skill composition

When extending the project:

1. Keep the core crate free of unnecessary dependencies.
2. Prefer plain-text, testable domain logic over framework-heavy abstractions.
3. Make trust and sandbox decisions explicit in the output.
4. Treat command loading, skill loading, and prompt assembly as separate layers.
5. Preserve offline testability even before network adapters are implemented.
