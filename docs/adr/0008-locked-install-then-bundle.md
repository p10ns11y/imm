# ADR 0008: Locked install, then a bundler brings the sources

| Status | Accepted as an intermediate step |
| Date | 2026-09-22 |

## Context

The long run installs only the whole source of a direct dependency, and keeps every other use as an audited extract. Getting there in one cut is a new ecosystem.

An intermediate step uses a normal install, then throws most of it away. The install runs in a locked sandbox. An agent, working like a bundler, copies out only the sources a use reaches. The sandbox cannot read the host, and it cannot execute anything on the host.

## Decision

`imm sandbox` does two steps and then stops.

1. Lock. Copy a regular package tree into `sandbox/<name>/<version>`. Lifecycle scripts are not run. The lock records `network: false`, `hostAccess: false`, and `executed: false`.
2. Bring. From inside that tree only, follow the named export the way a bundler follows imports. Write those files to `brought/`. Leave the rest in the sandbox.

A path that resolves outside the sandbox is `sandbox-escape` and is not read. A request to execute sandbox code on the host is `host-execution` and does nothing. A reached tripwire, such as `child_process`, stays in the sandbox and is not brought out.

This prototype does not start a container, a chroot, or a package manager. The lock is the boundary the real sandbox has to keep. esbuild, Rollup, or webpack can be the bring step later. The contract is the same: read inside the lock, emit reached sources, never execute the install on the host.

## Consequences

The host project receives a small set of source files and their hashes. It does not receive `node_modules`, and it does not run the package. The unused files, including a dangerous file that nothing imports, stay behind the lock.

The long run in ADR 0004 can skip the regular install once the extract is known. Until then, the locked install is how an agent learns which sources to keep.
