# Agents

You are the maintainer. Install only a direct dependency, and install its whole source. Do not install what it depends on. For every other use, keep the extracted function, its hash, its audit, and your diff if you changed the bytes.

```bash
node bin/imm.mjs agent --module fixtures/modules/slugify.json
node bin/imm.mjs agent --proposal fixtures/proposals/fresh.json --now 2026-09-22T12:00:00.000Z
node bin/imm.mjs agent --package fixtures/registry/pure-slug --version 1.0.0 --exports slugify --now 2026-09-22T12:00:00.000Z
```

Read the JSON.

- Do `next` only.
- Leave `later` until the next call.
- Honor `skip`.
- When `ok` is false, the command exits 2. Stop. Do not install, bump, or store.

`approve --approver agent` stays refused. You judge. You do not write the bytes yourself.
