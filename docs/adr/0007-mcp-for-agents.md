# ADR 0007: Agents call install, audit, extract, and overwrite

| Status | Accepted |
| Date | 2026-09-22 |

## Context

Peramanathan, 21 May 2026: https://x.com/Peramanathan/status/2057334401659535431

A package manager should not unpack a tree of placeholders and unused code. The ecosystem can work the other way. The direct dependency you named is installed as whole source. Every other use is an extract an agent can audit and overwrite.

Agents already speak MCP. imm should answer on that protocol, and on any protocol with the same four calls.

## Decision

`imm mcp` speaks MCP over stdio. The tools are:

| Tool | Does |
| --- | --- |
| `imm_install` | Copy the whole source of one direct dependency into the state directory. No dependency tree. |
| `imm_extract` | Keep the files one named function reaches. Record their hash. Wait for an audit. |
| `imm_audit` | Attach who audited that extract, and when. |
| `imm_overwrite` | Replace the extract with the agent's source and store the diff against the original bytes. The original hash stays. |

The server does not call a registry and does not call a model. The registry in this prototype is `fixtures/registry/`. A later host can put the same four calls behind MCP, a CLI, or another agent protocol.

## Consequences

An agent installs `pure-slug@1.0.0` as source, then extracts `slugify`, audits it, and overwrites it. `src/cli.js` never lands in the extract. The diff is the record of the agent's change.

This does not replace the release check. A fresh or untrusted direct install is still refused by `judge` before an agent should call `imm_install`.
