# Source provenance

- Upstream project: [farion1231/cc-switch](https://github.com/farion1231/cc-switch)
- Upstream release: `v3.20.4`
- Upstream commit: `43e1d99084ed9b2f5dc252fd35c5adaf29d6876e`
- Client model connections: `0718f57`
- Native model page visibility fix: `10d0af62b0f5991eab61db5bf07811dfbd347660`
- ModelDeck source baseline tag: `modeldeck-baseline-2026-10-04`
- ModelDeck first release: `v0.1.0`
- License: MIT, with upstream and downstream copyright notices retained.

The source baseline was cloned with its Git history into the user-owned ModelDeck project. Personal application databases, keys, settings, logs and client sessions were not imported into the source tree. The upstream remote is retained for future review and merges; it is not a ModelDeck publishing destination.

Internal compatibility names such as `cc_switch_lib`, `.cc-switch` and `cc-switch.db` remain in the first release to preserve the established runtime data contract. User-facing product identity and OS registration use ModelDeck.

The original release and R2 mirror workflows are archived under `docs/upstream/workflows/`; they do not run as ModelDeck release jobs. The inherited CI checks remain available.

## ModelDeck 0.4.0

ModelDeck is maintained as its own model connection application. Version 0.4.0 removes the inherited account managers, cloud synchronization, automatic failover and general client-management services from the source and build. It retains the protocol adapters, native model configuration writers and actual usage collectors required by the supported client set.

The internal Rust library is now `modeldeck_lib`, with only an `rlib` target. `.cc-switch` and `cc-switch.db` remain storage compatibility names so existing resources, archives and usage records remain readable. The primary resource graph is `model-deck.json`.

The publishing repository is [Vita0818/ModelDeck](https://github.com/Vita0818/ModelDeck). Its current Git history includes the inherited source history; this release does not rewrite that history. Source attribution and MIT notices remain part of the project regardless of repository history organization.
