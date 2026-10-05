# ModelDeck

**Your model connections, across your coding clients.**

ModelDeck is a personal desktop application for managing model providers, endpoints, independent API keys, model catalogs, context limits and reasoning preferences across AI coding clients.

## Version 0.2.0

The main client switcher now covers exactly **Codex CLI, Claude Code, Claude Desktop, Antigravity CLI, Cursor CLI, Cursor GUI, Grok, Muse Code, OpenCode and Hermes**. The four model adapters share the main navigation and retain unsaved drafts when visiting other client pages or settings. Visibility settings, tray navigation and automatic startup synchronization follow this supported set. MiniMax Code, OpenClaw, Pi and Gemini CLI are excluded from the product's client scope.

This is a local Tauri/Rust/React desktop application built directly from the CC Switch source. See [v0.2.0](docs/releases/v0.2.0.md). The original `v0.1.0` snapshot remains available in Git.

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

API keys and personal runtime configuration are not part of this repository. Each newly managed client has its own credential binding. Cursor uses its own account services and requires a publicly reachable HTTPS endpoint; its server-side context and reasoning limits remain authoritative.

## Source and license

ModelDeck is derived from [CC Switch](https://github.com/farion1231/cc-switch), originally by Jason Young and contributors, under the MIT license. Upstream copyright notices are preserved in [LICENSE](LICENSE). See [source provenance](docs/UPSTREAM.md). Historical upstream documentation is retained in `docs/upstream/` as reference material.

The Git remote `upstream` points to CC Switch. No personal publishing remote or hosted release channel has been configured.
