# imm

independent module maintainer.

imm keeps a trivial module in your repo. You maintain that copy. In the long run, publish, download, and install stop. The exception is a direct dependency that is hard to replace and already matured. Its subdependencies are audited before any of those bytes are stored on the machine. Until that audit exists, they stay off the machine.

npm, pnpm, PyPI, crates, and the other install trees are the thing that goes away for everything else. A few functions never become a dependency tree. The import is a file you own, pinned by hash. The release check still runs, the unused files stay out, and a human approves the bytes before they land. The prototype runs the loop on local fixtures and does not call a live registry yet.

Decisions, each with a command:

| Approach | ADR | Command |
| --- | --- | --- |
| Release check. Pin the version, wait out the age window, refuse scripts, refuse a trust downgrade. | [0001](docs/adr/0001-release-checks-are-the-floor.md) | `judge` |
| Copy only the files a named export reaches. Block the package if that path looks dangerous. | [0002](docs/adr/0002-fill-only-reachable-files.md) | `slice` |
| Install writes throw-stubs and a proposal. A human approves the bytes. The agent does not. | [0003](docs/adr/0003-stubs-then-a-human-approves.md) | `plan`, `approve`, `verify` |
| Publish, download, and install stop, except a hard matured direct dependency after its subdependencies are audited. | [0004](docs/adr/0004-audit-subdeps-before-store.md) | `store` |
| Library authors put that audited extract in the package. The package does not ask for `node_modules`. | [0005](docs/adr/0005-authors-vendor-audited-code.md) | `store` |

No network. No dependencies. The registry is `fixtures/registry/`. Nothing in plan imports package code.

## Run

Node 24 or newer.

```bash
node bin/imm.mjs demo
node --test
```

Judge one proposal:

```bash
node bin/imm.mjs judge \
  --proposal fixtures/proposals/fresh.json \
  --now 2026-09-22T12:00:00.000Z
```

Slice one package:

```bash
node bin/imm.mjs slice \
  --package fixtures/registry/pure-slug \
  --version 1.0.0 \
  --exports slugify \
  --now 2026-09-22T12:00:00.000Z
```

Plan, then approve. `agent` exits 2. `human` is the seam for a second principal, not a login.

```bash
node bin/imm.mjs plan \
  --manifest fixtures/agent-manifest.json \
  --registry fixtures/registry \
  --out .imm \
  --now 2026-09-22T12:00:00.000Z

node bin/imm.mjs approve --state .imm --approver human
node bin/imm.mjs verify --state .imm
```

Store decision. The first command holds `sharp` because `color@4.2.3` has no audit. The second stores it after that audit.

```bash
node bin/imm.mjs store --module fixtures/modules/sharp.json
node bin/imm.mjs store --module fixtures/modules/sharp.json --audits fixtures/audits/color.json
node bin/imm.mjs store --module fixtures/modules/slugify.json
node bin/imm.mjs store --module fixtures/modules/slug-kit-install.json
node bin/imm.mjs store --module fixtures/modules/slug-kit.json
```

`slug-kit-install` is held. It still wants `node_modules` and names `slugify` without vendoring the files. `slug-kit` ships. The audited extract is already in the package.

## What the agent may do

The agent may write a manifest of exact versions and export names, run `plan`, and read `stage/` plus `proposal.json`.

The agent may not approve, loosen a range, run a lifecycle script, or fetch when the application imports. Over budget, plan stops. A stub throws until the fill exists. Verify checks vendor hashes and does not open the registry again.

## What is still stored

A direct dependency that is hard and matured, and only after each subdependency has an audit record for that exact version. A subdependency with no audit is not stored. A module that is not both hard and matured is maintained in the repo, with no publish, download, or install.

A library follows the same rule in the package it publishes. The author vendors the audited, extracted files and sets `node_modules` to false. Consumers do not install those files again. A library that still asks for `node_modules` is held.

A facade import such as `import { z } from "zod"` names the whole library. The scanner is one line at a time, and a comment that mentions `child_process` will block a file.

The age window is 2880 minutes, the same number the sibling Adaptate project sets in `pnpm-workspace.yaml`. imm does not call pnpm.
