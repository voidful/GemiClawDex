---
name: code-review
description: "Use when the user wants a code review instead of implementation. Prioritizes correctness bugs, behavioral regressions, missing tests, and risky assumptions."
version: 1.1.0
author: GCD (adapted from Hermes Agent requesting-code-review)
license: Apache-2.0
metadata:
  gcd:
    tags: [code-review, quality, verification, pre-commit]
    related_skills: [tdd, systematic-debugging, security-audit]
    origin: "Hermes Agent requesting-code-review skill"
---

# Code Review

## Core Principle

No agent should verify its own work. A code review uses a fresh perspective to find what the author missed.

## When to Use

- After implementing a feature or bug fix, before committing.
- When user says "review", "check", "look at", or "what's wrong with".
- After completing a task with 2+ file edits.

## Review Priorities (in order)

1. **Correctness bugs** — Does the code do what it claims to do?
2. **Behavioral regressions** — Does it break existing functionality?
3. **Missing tests** — Are there untested code paths?
4. **Risky assumptions** — Are there hardcoded values, unchecked errors, or race conditions?
5. **Security** — Prompt injection, path traversal, credential exposure.

## Review Process

1. Read the diff or file(s) under review.
2. Check if tests exist for the changed code. If not, flag it.
3. Look for edge cases the author likely did not consider.
4. Keep the summary short after the findings. Focus on actionable items.

## Output Format

For each finding:
```
[SEVERITY] Description
  File: path/to/file.rs:line
  Why: explanation of the risk
  Fix: suggested change (if obvious)
```

Severity levels: `CRITICAL`, `WARNING`, `INFO`.

## What NOT to Do

- Do not nitpick style issues that a linter should catch.
- Do not rewrite the code. This is a review, not an implementation.
- Do not praise code just to be polite. Be direct and useful.
