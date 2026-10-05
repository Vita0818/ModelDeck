# ModelDeck

**Your model connections, across your coding clients.**

ModelDeck is a personal desktop application for managing model providers, endpoints, independent API keys, model catalogs, context limits and reasoning preferences across AI coding clients.

## Version 0.3.0

ModelDeck separates two management layers:

- **Resource management:** shared upstream providers, credentials, models, capabilities and reasoning tiers, exposed through endpoint aliases.
- **Harness selection:** each client references an endpoint and public models. Native configuration and catalogs are generated from the shared graph.

Requests resolve the current resource mapping, so a route edit applies to every harness using it. Harness bindings contain references rather than another complete upstream catalog.

The supported set remains **Codex CLI, Claude Code, Claude Desktop, Antigravity CLI, Cursor CLI, Cursor GUI, Grok, Muse Code, OpenCode and Hermes**. Existing connections are migrated without applying native configurations. MiniMax Code, OpenClaw, Pi and Gemini CLI remain excluded.

See [the two-layer guide](docs/TWO_LAYER_MODELDECK.md) and [v0.3.0](docs/releases/v0.3.0.md). The `v0.1.0` and `v0.2.0` snapshots remain available.

## First version: 0.1.0

The first version starts from the complete working implementation already built on CC Switch v3.20.4. The preserved source baseline is `10d0af62b0f5991eab61db5bf07811dfbd347660`, tagged `modeldeck-baseline-2026-10-04`.

This release adds ModelDeck's own application identity, name, icon and version. It includes the existing Antigravity CLI, Cursor IDE, Cursor Agent CLI and Muse Code model connections, alongside the inherited clients. Application updates are local builds; there is no upstream auto-update channel.

- **Antigravity CLI:** custom model catalog, Gemini and OpenAI Chat routes; tested with 1.2.16.
- **Cursor IDE:** native BYOK and model settings; configuration structure verified for 3.9.16.
- **Cursor Agent CLI:** managed model catalog and native BYOK request credentials; verified against the installed June build and the October 1 build.
- **Muse Code:** Responses, OpenAI Chat and Anthropic adapters, catalog and streaming compatibility; tested with 0.1.0-R708.1.

See [client connection usage](docs/CLIENT_MODELS.md) and the [v0.1.0 release notes](docs/releases/v0.1.0.md) for the exact scope and verification limits.

## Development

Requirements: Node.js 18+, pnpm 10.12.3, Rust 1.85+ and the native Tauri 2 platform dependencies.

```sh
pnpm install --frozen-lockfile
pnpm dev
```

```sh
pnpm typecheck
pnpm format:check
pnpm test:unit
cd src-tauri
cargo fmt --check
cargo clippy
cargo test
```

Build with `pnpm build`. On this Mac, the documented helper also handles the installed Homebrew Rust toolchain: `./scripts/build-modeldeck-macos.sh`. Build products are kept under `release/` and are not committed.

## First release compatibility

To preserve the current working configuration contract, v0.1.0 retains the existing `.cc-switch` data directory, database filename and internal Rust library name. These are compatibility details; the installed product name is **ModelDeck** and its bundle identifier is **com.vita.modeldeck**. Do not run ModelDeck and CC Switch concurrently against the same configuration directory. A different configuration directory can be selected in Settings.

The registered URL scheme is `modeldeck://`. Pasted legacy `ccswitch://` imports remain accepted. The original CC Switch update endpoints and update signing key are not configured in ModelDeck.

API keys and personal runtime configuration are not part of this repository. Upstream credentials belong to global providers; harnesses use gateway credentials. Cursor uses its own account services and requires a publicly reachable HTTPS endpoint; its server-side context and reasoning limits remain authoritative.

## Source and license

ModelDeck is derived from [CC Switch](https://github.com/farion1231/cc-switch), originally by Jason Young and contributors, under the MIT license. Upstream copyright notices are preserved in [LICENSE](LICENSE). See [source provenance](docs/UPSTREAM.md). Historical upstream documentation is retained in `docs/upstream/` as reference material.

The Git remote `upstream` points to CC Switch. No personal publishing remote or hosted release channel has been configured.
