---
name: writing-plans
description: "Use when you have a spec or requirements for a multi-step task. Creates implementation plans with bite-sized tasks, exact file paths, and verification steps."
version: 1.0.0
author: GCD (adapted from Hermes Agent / obra/superpowers)
license: Apache-2.0
metadata:
  gcd:
    tags: [planning, design, implementation, workflow]
    related_skills: [tdd, code-review]
    origin: "Hermes Agent writing-plans skill"
---

# Writing Implementation Plans

## Core Principle

A good plan makes implementation obvious. If someone has to guess, the plan is incomplete.

## When to Use

Before implementing any multi-step feature. Before delegating to subagents via `spawn_agent`. Even when the task seems simple, because assumptions cause bugs.

## Plan Structure

Every plan must include:

1. **Goal** — One sentence. What does "done" look like?
2. **Current state** — What exists now? What files are relevant?
3. **Steps** — Each step is one action (2-5 minutes of focused work).
4. **Files to change** — Exact paths relative to repo root.
5. **Verification** — How to confirm each step worked.
6. **Risks** — What could go wrong? What assumptions are we making?

## Bite-Sized Task Granularity

Each step must be a single action:
- "Write the failing test" is a step.
- "Implement the feature" is NOT a step. Break it down.
- "Add the struct and write tests" is NOT a step. That is two steps.

## Output Format

Save the plan as markdown. If using GCD's command system:

```toml
# .gcd/commands/plan/feature-name.toml
description = "Plan for implementing feature X"
prompt = """Study @{docs/architecture.md} and create an implementation plan for {{args}}."""
```
