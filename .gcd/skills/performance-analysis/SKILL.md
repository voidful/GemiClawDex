---
name: performance-analysis
description: "Use when investigating slow execution, high memory usage, or excessive token consumption. Systematic measurement before optimization."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [performance, optimization, profiling, tokens]
    related_skills: [systematic-debugging, code-review]
---

# Performance Analysis

## Core Principle

Measure first, optimize second. Never optimize based on intuition. Profiling data decides what to fix.

## Two Performance Dimensions in Agent Systems

### 1. Runtime Performance (Rust code)

Standard profiling applies:

```bash
# Compile with debug symbols for profiling
cargo build --release
# Use system profiler (Linux)
perf record ./target/release/gcd exec "task"
perf report
```

### 2. Token Performance (LLM usage)

This is unique to agent systems. Every token costs money and latency.

Metrics to track:
- **Prompt tokens per turn** — Is the context bloated?
- **Completion tokens per turn** — Is the model over-generating?
- **Tool calls per task** — Are there wasted tool calls (reading irrelevant files)?
- **Total cost per task** — GCD tracks this automatically per session.

## Common Token Waste Patterns

- **Context stuffing**: Injecting too many files via `@{...}` when only one is needed.
- **Skill catalog bloat**: Too many skills listed in the prompt. Use Progressive Disclosure.
- **History accumulation**: Long conversations without auto-compact triggering.
- **Redundant tool calls**: Agent reads the same file multiple times across turns.

## When to Use

- When a task takes noticeably more tokens than expected.
- When `auto-compact` triggers frequently (sign of context pressure).
- Before adding new context sources to the prompt assembly.
