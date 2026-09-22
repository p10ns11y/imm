# Limits

`imm` reads local fixtures. It does not call npm, and it does not call a registry.

## The scanner

`src/slice.mjs` reads one line at a time. It follows `export { name } from "./file.js"`, `export function`, `export const`, and a one-line `import`.

A multi-line import stops the scan. `export *` stops the scan. A comment that contains `child_process`, `require(`, `eval(`, `Function(`, an `http` URL, or `process.binding` blocks that file when the walk reaches it.

`import { z } from "zod"` names one export, and that export is the whole library. The walk then keeps almost every file. imm does not call that import a small extract.

## The age check

`judge` refuses a version published less than 2880 minutes before the clock you pass. The sibling Adaptate project uses the same number in `pnpm-workspace.yaml`. imm does not call pnpm.

## The lock

`imm sandbox` copies files into a directory and does not import them. It does not start a container. A later process can still read the host or run a file from that directory. `hostAccess: false` is a field in the lock record. It is not an operating-system boundary.

## Who writes bytes

`approve --approver agent` exits 2. `imm_overwrite` still writes the new extract. Those two commands do not agree. [Approach and engineering](evaluation.md) says to pick one rule.
