# imm

independent module maintainer. Install only the whole source of direct dependencies. Every other use is an audited extract with a hash and the agent diff. Library authors ship those extracts with no node_modules.

You install only the direct dependencies you named. Each install is the whole source of that package. You do not install the packages under it.

Every other use is an extract. That is the function or code you actually call, an audit of that extract, the hash of the original bytes, and the diff if an agent changed it. A few functions never become a dependency tree.

This is the package ecosystem [Peramanathan described on 21 May 2026](https://x.com/Peramanathan/status/2057334401659535431). A registry still exists for the direct package. Agents do the rest through MCP, or any protocol with the same four calls: install the whole source, extract the function you use, audit that extract, and overwrite it while keeping the original hash and the diff.

The release check still runs, unused files stay out, and a human approves the bytes before they land. The prototype runs the loop on local fixtures and does not call a live registry yet.

One intermediate step still uses a regular install. That install is locked in a sandbox: it cannot read the host, and it cannot execute anything on the host. The agent then works like a bundler. It copies out only the sources the use reaches, and leaves the rest inside the lock. esbuild, Rollup, or webpack can do that copy later. `imm sandbox` is the same contract without running the package.

Decisions, each with a command:

| Approach | ADR | Command |
| --- | --- | --- |
| Release check. Pin the version, wait out the age window, refuse scripts, refuse a trust downgrade. | [0001](docs/adr/0001-release-checks-are-the-floor.md) | `judge` |
| Copy only the files a named export reaches. Block the package if that path looks dangerous. | [0002](docs/adr/0002-fill-only-reachable-files.md) | `slice` |
| Install writes throw-stubs and a proposal. A human approves the bytes. The agent does not. | [0003](docs/adr/0003-stubs-then-a-human-approves.md) | `plan`, `approve`, `verify` |
| Install the whole source of each direct dependency. Every other use is an audited extract with a hash and, when an agent edited it, a diff. | [0004](docs/adr/0004-audit-subdeps-before-store.md) | `store` |
| Library authors put that audited extract in the package. The package does not ask for `node_modules`. | [0005](docs/adr/0005-authors-vendor-audited-code.md) | `store` |
| The agent judges one next maintenance step. | [0006](docs/adr/0006-agents-judge-the-next-step.md) | `agent` |
| Agents install, extract, audit, and overwrite over MCP. | [0007](docs/adr/0007-mcp-for-agents.md) | `mcp` |
| A regular install stays locked in a sandbox. A bundler-style pass brings out only the reached sources. | [0008](docs/adr/0008-locked-install-then-bundle.md) | `sandbox` |

No network. No dependencies. The registry is `fixtures/registry/`. Nothing in plan imports package code.

An agent connects with MCP:

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

The tools are `imm_install`, `imm_extract`, `imm_audit`, and `imm_overwrite`.

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

Store decision. `sharp` is a direct dependency, so it installs as whole source. `color` is a use under it, so it stays an extract until it has a hash, an audit, and the agent diff when the agent edited it.

```bash
node bin/imm.mjs store --module fixtures/modules/sharp.json
node bin/imm.mjs store --module fixtures/modules/sharp.json --audits fixtures/audits/color.json
node bin/imm.mjs store --module fixtures/modules/slugify.json
node bin/imm.mjs store --module fixtures/modules/slug-kit-install.json
node bin/imm.mjs store --module fixtures/modules/slug-kit.json
```

`slug-kit-install` is held. It still wants `node_modules` and names `slugify` without vendoring the files. `slug-kit` ships. The audited extract is already in the package.

## What the agent does

The agent maintains the module. It runs `agent`, reads the JSON, and does `next` only.

```bash
node bin/imm.mjs agent --module fixtures/modules/sharp.json
node bin/imm.mjs agent --proposal fixtures/proposals/fresh.json --now 2026-09-22T12:00:00.000Z
```

`sharp` installs as whole source and skips its subdependencies. `fresh-slug` keeps the pin and skips the bump. A function that has no hash yet is told to record the hash, and the audit waits. A library that still wants `node_modules` is told to vendor one extract.

The agent may also write a manifest of exact versions and export names, run `plan`, and read `stage/` plus `proposal.json`. It may not approve its own byte write, loosen a range, run a lifecycle script, or fetch when the application imports. `AGENTS.md` is the short contract.

## What is installed

The whole source of each direct dependency. Nothing under that dependency is installed with it.

A use that is not a direct dependency stays in the repo as an extract: the code, the audit, the hash, and the agent's diff when the agent changed the code.

A library follows the same rule in the package it publishes. The author vendors those audited extracts and sets `node_modules` to false. Consumers do not install those files again. A library that still asks for `node_modules` is held.

A facade import such as `import { z } from "zod"` names the whole library. The scanner is one line at a time, and a comment that mentions `child_process` will block a file.

The age window is 2880 minutes, the same number the sibling Adaptate project sets in `pnpm-workspace.yaml`. imm does not call pnpm.
