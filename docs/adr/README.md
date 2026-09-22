# ADRs

| ADR | Decision |
| --- | --- |
| [0001](0001-release-checks-are-the-floor.md) | Exact versions, integrity, no lifecycle scripts, a two-day age window, no trust downgrade. |
| [0002](0002-fill-only-reachable-files.md) | Copy the files a named export reaches. Tripwires on that path block the package. |
| [0003](0003-stubs-then-a-human-approves.md) | Install writes throw-stubs. An agent may plan. A different principal approves the bytes. |
| [0004](0004-audit-subdeps-before-store.md) | Publish, download, and install stop, except a hard matured direct dependency whose subdependencies are already audited. |
| [0005](0005-authors-vendor-audited-code.md) | Library authors ship that audited extract inside the package and do not ask for `node_modules`. |

0001 is the floor. 0002 is the reachable copy. 0003 is how an agent is allowed to touch them. 0004 is what may be stored on the machine. 0005 is what a library author puts in the package so consumers never install those files.
