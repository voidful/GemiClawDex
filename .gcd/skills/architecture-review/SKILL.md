---
name: architecture-review
description: "Use when evaluating whether a proposed change fits the existing architecture. Prevents layer violations, dependency cycles, and accidental coupling."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [architecture, design, review, modularity]
    related_skills: [code-review, refactoring, writing-plans]
---

# Architecture Review

## Core Principle

Every module should have one reason to change. If a proposed change touches three unrelated modules, the architecture has a coupling problem.

## GCD Architecture Rules

These are the rules from AGENTS.md, enforced during architecture review:

1. **gcd-core must stay dependency-light.** Every new dependency needs justification.
2. **Command loading, skill loading, and prompt assembly are separate layers.** Never entangle them.
3. **Trust and sandbox decisions must be explicit.** No silent bypasses.
4. **Offline testability is mandatory.** Core logic must work without API keys.

## Dependency Direction

```
gcd-cli → gcd-core (one-way only)
gcd-core never imports from gcd-cli
```

Within gcd-core:
```
tools/registry.rs ← tools/*.rs ← agent.rs ← prompt.rs ← app.rs
```

A change that reverses any arrow is an architecture violation.

## Review Questions

For any proposed change, ask:

1. Does this change respect the dependency direction?
2. Does this add a new dependency to gcd-core? If yes, is it justified?
3. Does this mix concerns across layers (e.g., prompt logic in a tool file)?
4. Can this be tested offline (without API keys)?
5. Is the trust/permission impact explicit in the output?

## When to Use

- Before any PR that adds a new module or significantly changes existing module boundaries.
- When a refactoring plan proposes moving code between crates.
- When adding a new tool that might need special permissions.
