---
name: security-audit
description: "Use when reviewing code for security vulnerabilities. Covers prompt injection, path traversal, command injection, and agent-specific attack vectors."
version: 1.0.0
author: GCD
license: Apache-2.0
metadata:
  gcd:
    tags: [security, audit, review, safety]
    related_skills: [code-review, systematic-debugging]
---

# Security Audit

## Agent-Specific Threats

AI coding agents face unique security threats beyond traditional application security:

1. **Prompt injection via MEMORY.md / USER.md** — Malicious content written to memory files can alter agent behavior. GCD's `memory_tool.rs` includes pattern scanning for this.
2. **Path traversal via tool calls** — Agent requests `read_file("../../etc/passwd")`. GCD's trust system restricts this.
3. **Command injection via shell tool** — Agent constructs shell commands from untrusted input. The permission system is the primary defense.
4. **Exfiltration via fetch_url** — Agent sends sensitive data to external URLs. Sandbox policy controls this.

## Audit Checklist

For every code change, check:

- Does this introduce a new path where user input reaches a shell command?
- Does this bypass the trust / permission system?
- Does this write to MEMORY.md / USER.md without the safety scan?
- Does this expose API keys or credentials in output / logs?
- Does this fetch external URLs without sandbox policy check?

## In GCD Source Code

Key security boundaries:
- `trust.rs` — Three-level trust model
- `agent/permissions.rs` — Permission prompt logic
- `tools/memory_tool.rs` — Memory safety scanning
- `tools/container.rs` — Container sandbox
