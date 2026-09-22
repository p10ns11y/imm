# ADR 0001: Release checks are the floor

| Status | Accepted |
| Date | 2026-09-22 |

## Context

Mitchell Hashimoto's position, restated on 20 May 2026, is that a dependency update is riskier than a known bug you can track. Update when you can point at the commit you need. Fork and trim when you must. Do not bump for its own sake.

https://x.com/mitchellh/status/2057171518027887035

Agents make that habit worse. An agent will propose updates all day, and it will treat "latest" as a normal argument. The package manager has to reject those proposals before any bytes are unpacked.

Adaptate, the sibling project, already runs this policy for humans. Its `pnpm-workspace.yaml` sets `minimumReleaseAge` to 2880 minutes, `trustPolicy` to `no-downgrade`, and blocks exotic subdependencies. Its `.npmrc` sets `ignore-scripts=true`. This ADR copies that floor into the prototype. It does not wrap pnpm.

## Decision

Every approach in this repo keeps the same release check. A proposal is acceptable only when all of these hold.

- The version is an exact `x.y.z`. A range, a tag, or a missing version is a reject.
- The record has an integrity string.
- The package has no `preinstall`, `install`, `postinstall`, or `prepare` script.
- The publish time is at least 2880 minutes before the clock passed to `judge`.
- Trust is one of `none`, `provenance`, or `trusted-publisher`. If a previous trust is recorded, the new rank must be greater than or equal to it.

An agent may submit the proposal. Acceptance of the proposal is not permission to write the files. That split is ADR 0003.

The prototype command is `judge`. Input fixtures live in `fixtures/proposals/`. The clock is passed in, so the age check does not depend on the machine clock.

## Consequences

A clean proposal can still contain a bad reachable file. The release check does not read the source. ADR 0002 does.

A poisoned release that is already older than two days, and that kept its trust rank, passes this check. Age and trust are delays and signals, not a review of the diff.

`judge` never fetches. A real registry client would still have to download the tarball later, by the integrity hash, into a cache that install does not execute.
