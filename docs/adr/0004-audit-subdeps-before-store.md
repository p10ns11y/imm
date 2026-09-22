# ADR 0004: Install direct source, extract every other use

| Status | Accepted as the long run |
| Date | 2026-09-22 |

## Context

An earlier draft said publish, download, and install stop, except a hard matured direct dependency, and that its subdependencies are stored only after an audit. That mixed two different actions.

The install is the direct dependency, and it is the whole source of that package. The packages and functions underneath it are not installed. Each use of that other code is its own record.

## Decision

| What the project named | What imm does |
| --- | --- |
| A direct dependency | Install the whole source of that package. Do not install its dependency tree. |
| Any other use, including a subdependency or a single function | Keep an extract of the code that use needs. |

An extract has three records.

- A hash of the original bytes.
- An audit of that extract.
- The diff, when an agent changed those bytes. No change means no diff. A change with no diff is incomplete.

`imm store` prints `install-source` for a direct dependency. It prints `extract` for a complete per-usage record. It prints `hold` until the hash, the audit, or the agent diff is present. `hold` exits 2.

Hard and matured are reasons you might name something as a direct dependency. They are not a second install rule. If the project named it, the whole source is the install.

## Consequences

`node_modules` does not grow from subdependencies. Those uses sit in the repo as extracted functions, each pinned by hash, each audited, each carrying the agent's diff when the agent edited it.

Library authors do the same inside the package they ship. ADR 0005.

The release check in ADR 0001 still applies to a direct source install and to an extract that came from a registry. The prototype reads the records. It does not perform the audit, and it does not call a registry.
