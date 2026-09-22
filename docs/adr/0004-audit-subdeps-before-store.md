# ADR 0004: Audit subdependencies before anything is stored

| Status | Accepted as the long run |
| Date | 2026-09-22 |

## Context

imm exists so a trivial module does not need a registry. The long run is stronger than that. Publish, download, and install drop out of the normal path. The remaining exception is a dependency the project names itself, that is hard to replace, and that has already matured.

A hard dependency is one you cannot honestly keep as a few functions in the repo. Image decoding, a crypto primitive, a runtime binding. A matured dependency is one you already trust enough to pin, not a release from this week. Direct means the project named it. Anything that dependency needs, and the project did not name, is a subdependency.

Those subdependencies are how a small direct choice becomes a large unreviewed tree. imm does not store that tree because the direct dependency was allowed.

## Decision

| Kind | Publish, download, install | On the machine |
| --- | --- | --- |
| Trivial module | Never. You maintain the source in the repo. | The repo copy only. |
| Direct, but not both hard and matured | Never. Treat it as a trivial module. | The repo copy only. |
| Subdependency | No install. An audit record comes first. | Stored only after the audit names that exact version. |
| Direct, hard, and matured | This is the exception that still exists. | Stored only after every subdependency has an audit record. |

An audit record names the subdependency version, who audited it, and when. The prototype reads that record. It does not perform the audit. Missing record means the bytes stay off the machine. The direct dependency waits with them.

`imm store` prints one of `local`, `hold`, or `store`. `hold` exits 2.

## Consequences

The registry stops being the place trivial code lives. It remains, for this exception, a source of hard matured direct dependencies and of the audited subdependency bytes those require. Library authors do this work inside the package they ship. ADR 0005.

A subdependency audit is per version. A new version is a new audit. The release check in ADR 0001 still applies to anything that is allowed onto the machine.

`import { z } from "zod"` is not made trivial by this rule. If the project names zod as a hard matured direct dependency, zod may be stored, and only after each subdependency zod needs has an audit. If the project does not need that, zod stays out and the few calls live in the repo.
