# ADR 0006: Agents judge the next maintenance step

| Status | Accepted |
| Date | 2026-09-22 |

## Context

The release check, the reachable copy, the store rule, and the vendored library are separate commands. An agent that wants to maintain a module has to reassemble them, and it will do extra work if the answer is a list.

The agent is the maintainer. It should receive one next action, the things it must not do, and the reasons. A model may call this. The command itself does not call a model and does not use the network.

## Decision

`imm agent` prints JSON.

- `next` is the only action to take now.
- `later` is work that waits until `next` is done. One unaudited subdependency is audited before the rest. The missing extract is vendored before `nodeModules` is cleared.
- `skip` is closed. A failed release check skips the bump. A direct dependency skips its subdependency tree. A use that is not direct skips install and keeps the extract. A dangerous reached file skips vendor.
- `why` is the check that fired.
- `ok` is true only when the module is already in the shape imm wants: local source, a shipped library, a store that is allowed, an extract that dropped unused files, or an update whose release check passed and still needs a human to read the diff.

Exit 2 when `ok` is false. The agent stops instead of installing.

Landing bytes stays on `approve`. An agent still cannot pass `--approver agent`. Judgment and the write are different steps.

## Consequences

Maintenance gets smaller because the agent does not open a tree to answer a question. It keeps a pin, vendors one extract, or audits one subdependency.

The judgment is only as smart as the records it is given. A missing audit is not invented. A comment that mentions `child_process` still blocks a file, as ADR 0002 says.
