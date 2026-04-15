---
name: documentation-review
description: "Use when reviewing documentation for accuracy, completeness, and alignment with source code. Catches doc-code drift before it confuses readers."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [documentation, review, quality, accuracy]
    related_skills: [code-review, writing-plans]
---

# Documentation Review

## Core Principle

Documentation that disagrees with the source code is worse than no documentation. It actively misleads.

## Checklist

For every documentation page, verify:

1. **Code references exist** — Every `code block` that references a file path, function name, or CLI command must correspond to something that actually exists in the repo.
2. **Feature status is honest** — Does the doc say "supports X"? Check if X is implemented, partially wired, or just a stub. Use honest labels.
3. **Examples are runnable** — Can a reader copy-paste the example and get the described result?
4. **Numbers are current** — Tool counts, file counts, test counts, line counts. These drift fast.
5. **Links work** — Internal cross-references between chapters. External URLs.

## Severity Levels

- **CRITICAL**: Doc claims a feature exists but the code does not implement it.
- **WARNING**: Doc example would fail if a reader tried it (wrong path, missing flag).
- **INFO**: Minor wording issues, outdated screenshots, style inconsistencies.

## In the GCD Codebase

Key areas prone to doc-code drift:
- `docs/chapters/home.html` — stats (tool count, test count)
- `docs/chapters/u1.html` — install instructions, git clone URL
- `README.md` — feature list, architecture diagram
- `docs/chapters/comp.html` — comparison claims
