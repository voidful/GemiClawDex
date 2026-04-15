---
name: git-workflow
description: "Use when making changes that should be committed. Enforces atomic commits, meaningful messages, and clean history."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [git, workflow, version-control, commits]
    related_skills: [code-review, tdd]
---

# Git Workflow

## Core Principle

Each commit should represent one logical change. If you cannot describe the commit in one sentence, it is too large.

## Commit Message Format

```
<type>: <description>

<optional body explaining why, not what>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.

## Before Committing

1. Run `cargo test`. All tests must pass.
2. Run `cargo clippy`. No warnings.
3. Run `cargo fmt --check`. Code is formatted.
4. Review the diff: `git diff --cached`. Every changed line should relate to the commit message.

## Atomic Commits

- Separate refactoring from feature work. Refactor first, commit, then add the feature.
- Separate test additions from implementation. Add the test (it fails), commit. Implement (it passes), commit.
- Never mix documentation changes with code changes in the same commit.

## GCD-Specific Rules

- The `--git` flag in `gcd exec` auto-commits after the session. Use it only when the task is self-contained.
- For multi-step tasks, prefer manual commits between steps.
