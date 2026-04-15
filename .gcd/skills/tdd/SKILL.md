---
name: tdd
description: "Use when implementing any feature or bugfix. Enforces RED-GREEN-REFACTOR cycle with test-first approach."
version: 1.0.0
author: GCD (adapted from Hermes Agent / obra/superpowers)
license: Apache-2.0
metadata:
  gcd:
    tags: [testing, tdd, development, quality]
    related_skills: [systematic-debugging, writing-plans]
    origin: "Hermes Agent test-driven-development skill"
---

# Test-Driven Development (TDD)

## The Iron Law

```
NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST
```

Write code before the test? Delete it. Start over.

## The Cycle

1. **RED** — Write a test that fails. Run it. Confirm it fails for the right reason.
2. **GREEN** — Write the minimum code to make the test pass. Nothing more.
3. **REFACTOR** — Clean up. Both test and production code. Tests must still pass.

## In the GCD Codebase

```bash
# Run a specific test
cargo test -p gcd-core test_name

# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

## What Counts as "Minimum Code"

- If the test expects a return value, hardcode it first. Then generalize.
- If the test expects an error, return the error. Do not add error handling for cases no test covers yet.
- Resist the urge to "finish" the function. Let the next test drive the next behavior.

## When to Skip (Ask the User First)

- Throwaway prototypes that will be deleted within the session.
- Generated code (e.g., provider adapter boilerplate).
- Configuration files.
