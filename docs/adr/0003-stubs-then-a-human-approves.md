# ADR 0003: Stubs first, then a human approves

| Status | Accepted. imm replaces the install ecosystem for a trivial module with no deep dependencies. |
| Date | 2026-09-22 |

## Context

The request that started this repo:

https://x.com/Peramanathan/status/2057334401659535431

Install should not unpack a tree. It should record bare exports and empty placeholders. A later CLI pulls the utility code and checks it. `node_modules` is the thing to get rid of.

imm is that tool. The letters stand for independent module maintainer. The pull is still the moment untrusted code enters the repo, so the release check and the human approval stay.

In the agent era the installer is not a patient human with a diff open. The agent can write manifests in a loop, add packages to get past an error, and call approve on its own proposal. The ecosystem has to assume that.

## Decision

The ideal loop, and the loop `plan`, `approve`, and `verify` implement on local fixtures:

1. The agent writes a manifest. Each need has a specifier, an exact version, and the export names it will call. Budgets cap package count, file count, bytes, and import depth.
2. Plan runs the ADR 0001 release check and the ADR 0002 slice. It writes throw-stubs. It writes a `proposal.json` and, only when nothing is blocked, a `stage/` directory. It does not write `vendor/`.
3. Importing a stub throws `not filled`. A stub that returned a dummy value would let the agent continue on missing code.
4. Approve requires `approver` to be `human`. `agent` throws `AGENT_APPROVER` and leaves no vendor directory. A blocked proposal throws `BLOCKED` for a human too.
5. Approve hashes the staged bytes and refuses `HASH_MISMATCH` if they differ from the proposal.
6. Verify hashes `vendor/` against `lock.json`. It does not open the registry. A later edit to the fixture registry does not affect a lock that already passed.

Roles, if this contract were ever a real ecosystem:

| Role | Does | Does not |
| Registry | Store immutable bytes addressed by hash | Run package code |
| Plan | Write stubs and a file list | Unpack the whole package, run scripts, or fetch at import time |
| Approver | A different principal from the agent that wrote the manifest | The same process the agent controls |
| Runtime | Import vendor or stub | Talk to the registry |

The word `human` on the command line is the seam where that second principal plugs in. It is not a login. An agent that can pass `--approver human` can defeat this prototype. The tests lock the seam so the default agent path fails closed. A real approver would be a signature or a review system the agent cannot mint.

Auto-approve is allowed only for a future case this code does not implement. The proposal contains no new bytes because the lock already matches. New bytes always stop.

`maxPackages` below the need list returns `budget-packages` and does not slice those packages. The agent does not get to grow the graph by retrying past the budget.

## Consequences

imm is the replacement for that package's ecosystem. npm, PyPI, crates, and the others do the same waste when the package is a few functions and nothing under it matters. The reachable source is the dependency. You do not install a tree, and you do not keep a package entry.

Yarn Plug'n'Play and pnpm's content-addressable store still store whole packages. imm does not. The loop above is the product. Named exports, a reviewed file list, stubs that throw, and a second principal.

This prototype has no network client and no loader that patches a language runtime. The fixtures are the loop, imported by file URL in tests. Which bytes are a whole-source install, and which are a per-usage extract, is ADR 0004. `import { z } from "zod"` names a facade, so the slice stays large, as ADR 0002 says. imm does not call that package trivial.
