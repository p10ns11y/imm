# ADR 0005: Library authors vendor the audited extract

| Status | Accepted |
| Date | 2026-09-22 |

## Context

ADR 0004 stops publish, download, and install for trivial code, and it stores a hard matured direct dependency only after that dependency's subdependencies are audited. If library authors keep declaring those subdependencies the old way, every consumer grows a `node_modules` tree again.

The same work belongs in the library. The author already knows which files the package calls. They can audit that extract once and put the files in the package they ship.

## Decision

A library package qualifies when all of these hold.

- `nodeModules` is false. The package does not ask the consumer to install a tree.
- Every subdependency it still names is present inside the package as vendored files.
- Those files are the extract, not the whole upstream package. An empty file list does not count.
- Each vendored entry has an audit record: who audited that version, and when.

`imm store` then prints `ship` with `vendored audited extracted no-node-modules`.

Anything short of that is `hold`. A package that still wants `node_modules` gets `needs-no-node-modules`. A named subdependency with no vendored extract gets `needs-vendor:<id>`. Vendored bytes with no audit get `vendored-unaudited:<id>`. Vendored bytes with no files get `not-extracted:<id>`.

The hold is the pressure on authors. The package consumers want is the one that already contains the audited extract.

## Consequences

Downstream, those utilities are not subdependencies anymore. They are files in the library, so ADR 0004 does not ask the consumer to store them again.

A hard matured direct dependency can still be stored under ADR 0004. A library author who needs one names it as direct, hard, and matured, and still vendors every trivial subdependency instead of leaving it for `node_modules`.

The prototype checks the manifest. It does not copy the author's files, and it does not audit them. The record is the claim the author is making. A later review still reads the diff.
