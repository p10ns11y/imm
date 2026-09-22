# ADRs

| ADR | Decision |
| --- | --- |
| [0001](0001-release-checks-are-the-floor.md) | Exact versions, integrity, no lifecycle scripts, a two-day age window, no trust downgrade. |
| [0002](0002-fill-only-reachable-files.md) | Copy the files a named export reaches. Tripwires on that path block the package. |
| [0003](0003-stubs-then-a-human-approves.md) | Install writes throw-stubs. An agent may plan. A different principal approves the bytes. |
| [0004](0004-audit-subdeps-before-store.md) | Install the whole source of each direct dependency. Every other use is an audited extract with a hash and the agent diff. |
| [0005](0005-authors-vendor-audited-code.md) | Library authors ship that audited extract inside the package and do not ask for `node_modules`. |
| [0006](0006-agents-judge-the-next-step.md) | The agent judges one next maintenance step and skips install, bump, and store until that step is done. |
| [0007](0007-mcp-for-agents.md) | Agents install, extract, audit, and overwrite through MCP. The same four calls fit any similar protocol. |
| [0008](0008-locked-install-then-bundle.md) | A regular install stays in a locked sandbox. A bundler-style pass brings out only the reached sources. The sandbox cannot read or execute on the host. |

0001 is the floor. 0002 is the reachable copy. 0003 is who may write bytes. 0004 is the install: whole source for a direct dependency, and an extract for every other use. 0005 is what a library author puts in the package. 0006 is how an agent maintains it. 0007 is the protocol those agents call. 0008 is the intermediate step that still starts from a regular install. The idea starts from [Peramanathan's post](https://x.com/Peramanathan/status/2057334401659535431).
