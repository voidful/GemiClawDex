---
name: systematic-debugging
description: "Use when encountering any bug, test failure, or unexpected behavior. Four-phase root cause investigation. No fixes without understanding the problem first."
version: 1.0.0
author: GCD (adapted from Hermes Agent / obra/superpowers)
license: Apache-2.0
metadata:
  gcd:
    tags: [debugging, troubleshooting, root-cause, investigation]
    related_skills: [tdd, code-review]
    origin: "Hermes Agent systematic-debugging skill"
---

# Systematic Debugging

## Core Principle

Random fixes waste time and create new bugs. **Always find the root cause before attempting fixes.**

## Phase 1 — Reproduce

Before anything else, reproduce the failure:

```bash
# Run the failing test or trigger the bug
cargo test <failing_test_name>
```

Record: exact error message, stack trace, which input triggers it.

## Phase 2 — Isolate

Narrow the scope:

- Is it a single function, a module boundary, or a data flow issue?
- Add `dbg!()` or `eprintln!()` at suspected boundaries.
- Check: does the bug appear in the most recent commit? Use `git bisect` if needed.

## Phase 3 — Understand

Before writing any fix:

- State the root cause in one sentence.
- Explain why the current code produces the wrong behavior.
- Predict what a correct fix would change.

If you cannot do all three, you have not finished Phase 3.

## Phase 4 — Fix and Verify

- Write a test that captures the root cause (this test should fail before the fix).
- Apply the minimal fix.
- Run the full test suite to check for regressions.

## When NOT to Use

- Typos and trivial syntax errors. Just fix them.
- Build configuration issues. Check `Cargo.toml` first.
