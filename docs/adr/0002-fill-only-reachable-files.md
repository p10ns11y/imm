# ADR 0002: Fill only the files an export reaches

| Status | Accepted for this prototype |
| Date | 2026-09-22 |

## Context

Forking a direct dependency does not shrink the packages it imports. A full install also keeps files the program never calls. Those unused files are where a lot of `node_modules` bulk, and a lot of unreviewed code, sits.

The useful size of "trim to the use" is the file graph behind the exports you named. It is not a new installer, and it is not a fork of every package in the lockfile.

## Decision

`slice` starts at the package entry and follows only the named exports.

- `export { name } from "./file.js"` and `export { local as name }` are hops. The local name is what the next file must export.
- `export function` and `export const` mark the file that implements the name.
- From that file, static `import` lines are walked. Side-effect `import "./file.js"` counts.
- Files that the walk never reaches are omitted. In the `pure-slug` fixture, `src/cli.js` calls `child_process` and is omitted, because `slugify` never imports it.
- If a reached file contains a lexical tripwire, the whole package is blocked. The tripwires are `child_process`, `require(`, `eval(`, `Function(`, an `http` URL, and `process.binding`. One tripwire on the reached path is enough. The omitted cli does not block `pure-slug`. The same call in `shell-out`'s entry blocks `shell-out`.
- `export *` and a multi-line import are `star-reexport` and `parse-limit`. The scanner refuses them instead of guessing.
- A relative path that leaves the package directory is `path-escape`.
- Depth above `maxDepth` is `budget-depth`. The deep file is not copied.

The scanner reads text. It does not import the module. `fixtures/registry/side-effect/src/boom.js` throws at top level. A scan that executed it would crash the process. The test expects a normal result that still lists `boom.js`.

This is a line scanner for one-line import and export forms. It is not a JavaScript parser. A comment that mentions a tripwire blocks the file. That bias is deliberate in a prototype this small.

## Consequences

`import { z } from "zod"` names one export, and that export is the facade. The walk then keeps almost the whole package. Export-level fill does not shrink that kind of import. Call-level trimming is a different tool, closer to a bundler, and it is out of scope here.

Packages with native addons, generated builds, or computed `require` calls fail closed. The prototype leaves them blocked. A project that needs them keeps a whole pinned package and the release check from ADR 0001.

The kept files are the review. Hashes are SHA-256 of the bytes. Approval compares the staged bytes to those hashes.
