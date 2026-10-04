# Commands

Run these from the `imm` directory. The sample registry is `fixtures/registry/`. None of these commands call a network. None of them execute a package.

## Package flow

```bash
cargo run --quiet -- package-flow
cargo test
```

## Judge a proposed update

`judge` accepts an exact version, an integrity string, no lifecycle script, a publish at least 2880 minutes old, and no drop in trust. A fresh proposal exits 2.

```bash
cargo run --quiet -- judge \
  --proposal fixtures/proposals/fresh.json \
  --now 2026-09-22T12:00:00.000Z
```

## Copy the files one export reaches

`slice` keeps the files behind the named export. For `pure-slug`, `src/cli.js` is left out.

```bash
cargo run --quiet -- slice \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --now 2026-09-22T12:00:00.000Z
```

## Plan a fill, then approve the bytes

`plan` writes throw-stubs and, when nothing blocks the plan, a `stage/` directory. `approve --approver agent` exits 2. `approve --approver human` writes `vendor/` and a hash lock. `verify` checks that lock and does not open the registry.

```bash
cargo run --quiet -- plan \
  --manifest fixtures/agent-manifest.json \
  --registry fixtures/registry \
  --out .imm \
  --now 2026-09-22T12:00:00.000Z

cargo run --quiet -- approve --state .imm --approver human
cargo run --quiet -- verify --state .imm
```

## Decide what to install

`store` prints `install-source` for a direct dependency. It prints `hold` until a use has a hash, an audit, and a diff when an agent edited it. It prints `ship` when a library already contains that audited extract and sets `nodeModules` to false.

```bash
cargo run --quiet -- store --module fixtures/modules/sharp.json
cargo run --quiet -- store --module fixtures/modules/slugify.json
cargo run --quiet -- store --module fixtures/modules/color-use.json
cargo run --quiet -- store --module fixtures/modules/slug-kit-install.json
cargo run --quiet -- store --module fixtures/modules/slug-kit.json
```

`sharp` installs as whole source. `slugify` waits for a hash and an audit. `color-use` is a finished extract. `slug-kit-install` is held because it still wants `node_modules`. `slug-kit` ships.

## Ask an agent for one next step

`agent` prints JSON. Do `next` only. When `ok` is false, the command exits 2.

```bash
cargo run --quiet -- agent --module fixtures/modules/sharp.json
cargo run --quiet -- agent --proposal fixtures/proposals/fresh.json --now 2026-09-22T12:00:00.000Z
```

`sharp` installs as whole source and skips its subdependencies. `fresh-slug` keeps the pin and skips the bump.

## Lock an install, then bring the sources out

`sandbox` copies the package into `sandbox/` and writes `.imm-lock.json` with `executed` false. It does not run the package. `extract` is the reached-file copy.

```bash
cargo run --quiet -- sandbox \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --state .imm \
  --now 2026-09-22T12:00:00.000Z
```

## Install, extract, audit

v1 does not serve MCP.

```bash
cargo run --quiet -- install \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --state .imm \
  --now 2026-09-22T12:00:00.000Z

cargo run --quiet -- extract \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --state .imm \
  --now 2026-09-22T12:00:00.000Z

cargo run --quiet -- audit \
  --state .imm \
  --id 'pure-slug@1.0.0#slugify' \
  --by human \
  --at 2026-09-22T12:00:00.000Z
```

`install` runs the release check, then copies the whole tree. `extract` writes one use record and holds it until `audit`. An agent cannot approve bytes.
