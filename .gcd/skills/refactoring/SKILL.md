---
name: refactoring
description: "Use when improving code structure without changing behavior. Ensures each refactoring step preserves all existing tests."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [refactoring, code-quality, maintainability]
    related_skills: [tdd, code-review, writing-plans]
---

# Refactoring

## Core Principle

Refactoring changes structure, not behavior. If any test breaks, you changed behavior. Stop and revert.

## Before You Start

1. Run the full test suite. All tests must pass.
2. Commit the current state. You need a clean revert point.
3. State the goal in one sentence: "After this refactoring, X will be easier to Y."

## One Step at a Time

Each refactoring step follows this sequence:

1. Make one structural change (extract function, rename, move, inline).
2. Run tests. All must pass.
3. Commit with a message that describes the structural change, not the motivation.

Do NOT combine multiple refactoring steps in one commit.

## Common Patterns in GCD

- **Extract module**: When a file exceeds ~500 lines, extract a submodule. Example: `tools.rs` → `tools/` directory.
- **Extract trait**: When two implementations share an interface, define the trait in the parent module.
- **Reduce pub surface**: If a function is only used within the crate, remove `pub`.
- **Separate layers**: Command loading, skill loading, and prompt assembly must stay in separate modules.

## When NOT to Refactor

- During a feature implementation. Finish the feature first, then refactor.
- When tests are failing. Fix the tests first.
- When you do not understand what the code does. Read it first, add comments, then refactor.
