# imm

independent module maintainer.

Install a direct dependency as its whole source. Do not install the packages under it.

Keep every other use as an extract. The extract is the function you call, a hash of the original bytes, an audit, and a diff if an agent changed the code. Library authors ship those extracts. They do not ask you for `node_modules`.

[Peramanathan described this on 21 May 2026](https://x.com/Peramanathan/status/2057334401659535431).

A regular install can come first. `imm` copies it into a lock and does not run it. The agent then copies out only the files that use reaches.

## Try it

```bash
cargo test
cargo run --quiet -- package-flow
```

The binary does not run package code. A direct install is a copy that has already passed the release check. A use is one record: the reached files, a hash, an audit, and a diff when a human changed the bytes. `approve --approver agent` exits 2.

The sample registry is `fixtures/registry/`. These commands do not use the network.

## More

- [Commands](docs/commands.md) lists each `imm` call.
- [Decisions](docs/adr/README.md) records why each command exists.
- [Limits](docs/limits.md) records what the scanner will not do.
- [Approach and engineering](docs/evaluation.md) records what the code does not yet enforce.
- [AGENTS.md](AGENTS.md) tells an agent to take one next step.
