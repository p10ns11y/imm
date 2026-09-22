# Approach and engineering

The approach is the right product. The code is a decision table with three writers, and the sandbox claim is stronger than what the code enforces.

This note is an explanation of that judgment. It is not a procedure. Commands and records live in the README and the ADRs.

## What imm is for

An agent maintains dependencies for a person who owns a project. That person installs only the direct packages they named, and each install is the whole source of that package. Every other use is a small extract. The extract carries a hash of the original bytes, an audit, and a diff when the agent changed the bytes. A library author ships those extracts and does not ask the consumer to install `node_modules`.

A normal install tree is the wrong long-term shape for that agent. The agent will keep adding packages, and it will not read the unused files. The intermediate step is also the right migration. Lock a regular install, then copy out only the files a bundler would keep.

The next engineer inherits a prototype that can show those sentences on local fixtures. The prototype cannot yet enforce them on a real install.

## The words the code uses

A direct dependency is a package the project named. `decideRetention` in `src/store.mjs` returns `install-source` for that package and does not look at `subdeps`.

A use is one function taken from somewhere else. It needs a hash and an audit. It also needs a diff when the agent edited the bytes.

A locked install is a copy of a package tree under `sandbox/`. `bringSources` in `src/sandbox.mjs` copies the files one export reaches into `brought/`.

The release check in `src/policy.mjs` rejects a floating version, a missing integrity string, a lifecycle script, a publish that is too fresh, or a drop in trust.

## What actually runs

`imm agent` does not write files. It classifies a JSON manifest through `decideRetention` and returns one next action.

`imm_install`, `imm_extract`, `imm_audit`, and `imm_overwrite` do write files, through `src/ops.mjs`. Install copies a fixture tree. Extract runs the line scanner in `src/slice.mjs` and stores the reached text. Audit attaches a name and a time. Overwrite replaces stored text and keeps the original hash.

`imm sandbox` copies that kind of fixture into a directory, then runs the same scanner. The command never imports the package and never spawns a process. `executeOnHost` always returns a refusal, and nothing in the command calls it.

`plan` and `approve` are a third path. They write throw-stubs. A human then copies staged bytes into `vendor/`. An agent still cannot pass `--approver agent` on that path. The same agent can call `imm_overwrite` and write bytes with no second principal.

## Where a reader should start

Start with `src/slice.mjs` if you want the only reachability code. Start with `src/policy.mjs` for the release check. `src/store.mjs` is the pure decision table. `src/ops.mjs` and `src/sandbox.mjs` are the two writers. `src/agent.mjs` turns a decision into one next step. `src/mcp.mjs` is a thin JSON-RPC shell over the writers.

## What the tests prove

The scanner understands one-line `import` and `export`. A comment that mentions `child_process` blocks the file. A facade import such as `import { z } from "zod"` keeps the whole module. The tests prove those limits on fixtures. They do not prove the scanner against a real package.

`overwriteUse` builds the original hash from every reached file joined together, then writes the new text into the first file only. A multi-file extract is not what that diff describes.

`imm_install` does not call the release check. ADR 0007 says the check runs first. The code does not do that.

The sandbox tests show that this process did not import the fixture. They do not show that a later process cannot read the host or run a file in the sandbox. `hostAccess: false` is a field the code sets. It is not a boundary the operating system enforces.

## What to change, and what to leave

Change the records first. `decideRetention` and `src/ops.mjs` both decide what a use is, and they use different fields. Stop adding commands until a direct install and a use are one record each, and every writer updates that record. A use moves from extracted, to audited, to overwritten. A direct package moves from absent to installed.

Change the sandbox wording next. Describe the lock as a directory copy that this process does not execute. Do not describe `hostAccess: false` as containment.

Change who may write bytes. `imm_overwrite` and `approve --approver agent` disagree. If the agent may overwrite an extract, say that and require the stored diff. If a second person must approve bytes, `imm_overwrite` has to stop before the write.

Wait on replacing the line scanner with esbuild, Rollup, or webpack. The scanner is the right first check, and the tests cover it. Another bundler earns a place only after the record is one shape.

Leave the product story. The direct-source install, the per-use extract, and the locked install as a temporary teacher are the right sequence. The fixture registry is an honest limit for this stage. Eight ADRs and three stories make a new reader reconstruct the product. The first paragraph of the README is the product. Cut the code toward that paragraph. Do not extend another install mode beside it.
