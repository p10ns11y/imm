# Commands

Run these from the `imm` directory. You need Node 24 or newer. The sample registry is `fixtures/registry/`. None of these commands call a network.

## See the loop

```bash
node bin/imm.mjs demo
node --test
```

## Judge a proposed update

`judge` accepts an exact version, an integrity string, no lifecycle script, a publish at least 2880 minutes old, and no drop in trust. A fresh proposal exits 2.

```bash
node bin/imm.mjs judge \
  --proposal fixtures/proposals/fresh.json \
  --now 2026-09-22T12:00:00.000Z
```

## Copy the files one export reaches

`slice` keeps the files behind the named export. For `pure-slug`, `src/cli.js` is left out.

```bash
node bin/imm.mjs slice \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --now 2026-09-22T12:00:00.000Z
```

## Plan a fill, then approve the bytes

`plan` writes throw-stubs and, when nothing blocks the plan, a `stage/` directory. `approve --approver agent` exits 2. `approve --approver human` writes `vendor/` and a hash lock. `verify` checks that lock and does not open the registry.

```bash
node bin/imm.mjs plan \
  --manifest fixtures/agent-manifest.json \
  --registry fixtures/registry \
  --out .imm \
  --now 2026-09-22T12:00:00.000Z

node bin/imm.mjs approve --state .imm --approver human
node bin/imm.mjs verify --state .imm
```

## Decide what to install

`store` prints `install-source` for a direct dependency. It prints `hold` until a use has a hash, an audit, and a diff when an agent edited it. It prints `ship` when a library already contains that audited extract and sets `nodeModules` to false.

```bash
node bin/imm.mjs store --module fixtures/modules/sharp.json
node bin/imm.mjs store --module fixtures/modules/slugify.json
node bin/imm.mjs store --module fixtures/modules/color-use.json
node bin/imm.mjs store --module fixtures/modules/slug-kit-install.json
node bin/imm.mjs store --module fixtures/modules/slug-kit.json
```

`sharp` installs as whole source. `slugify` waits for a hash and an audit. `color-use` is a finished extract. `slug-kit-install` is held because it still wants `node_modules`. `slug-kit` ships.

## Ask an agent for one next step

`agent` prints JSON. Do `next` only. When `ok` is false, the command exits 2.

```bash
node bin/imm.mjs agent --module fixtures/modules/sharp.json
node bin/imm.mjs agent --proposal fixtures/proposals/fresh.json --now 2026-09-22T12:00:00.000Z
```

`sharp` installs as whole source and skips its subdependencies. `fresh-slug` keeps the pin and skips the bump.

## Lock an install, then bring the sources out

`sandbox` copies the package into `sandbox/` and does not run it. It then writes the reached files to `brought/`.

```bash
node bin/imm.mjs sandbox \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --state .imm \
  --now 2026-09-22T12:00:00.000Z
```

## Serve the same calls over MCP

```bash
node bin/imm.mjs mcp --registry fixtures/registry --state .imm
```

```json
{
  "mcpServers": {
    "imm": {
      "command": "node",
      "args": ["bin/imm.mjs", "mcp", "--registry", "fixtures/registry", "--state", ".imm"]
    }
  }
}
```

The tools are `imm_install`, `imm_extract`, `imm_audit`, `imm_sandbox`, and `imm_overwrite`.
