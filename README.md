# ModelDeck

**Your model connections, across your coding clients.**

ModelDeck is an independently maintained personal desktop application for model providers, endpoints, dedicated API keys, model catalogs, context limits and reasoning tiers. It reuses selected native configuration and protocol adapters from CC Switch within its own product scope.

## Version 0.4.0

The application keeps three areas:

- **Resources:** shared upstream providers, credentials, model capabilities and reasoning tiers, exposed through endpoint aliases and routes.
- **Client connections:** select an endpoint, default model and auxiliary model, then explicitly apply or restore native configuration.
- **Usage and settings:** read actual client usage records and manage pricing, the gateway, upstream network proxy, window behavior, directories, logging and local database archives.

Supported clients are **Codex CLI, Claude Code, Claude Desktop, Antigravity CLI, Cursor CLI, Cursor GUI, Grok, Muse Code, OpenCode, Hermes, DeepSeek Harness (DSH) and GitHub Copilot CLI**. DSH and Copilot CLI connect to shared endpoints through managed launch commands; see [the setup guide](docs/TWO_LAYER_MODELDECK.md#deepseek-harness-与-github-copilot-cli). Native usage collectors cover Claude Code, Codex CLI, OpenCode and Grok. Other clients retain historical records; consult the usage page's data-source descriptions for coverage.

Memory, MCP, Skills, prompts, session browsing, OpenClaw workspaces, the OAuth account center, cloud sync, automatic failover, scripted quota queries, tool installers and application auto-updates have had their management UI and runtime implementations removed. Native management and usage collectors for MiniMax Code, OpenClaw, Pi and Gemini CLI are excluded from the build. The Gemini model protocol remains supported for model routing and Antigravity.

Provider-specific authentication headers, URL construction, request fields, reasoning parameters and streaming conversions remain in the model transport layer. A route edit affects all clients referencing it; bindings store references instead of copying upstream credentials and catalogs.

See [the two-layer guide](docs/TWO_LAYER_MODELDECK.md), [client adapters](docs/CLIENT_MODELS.md) and [v0.4.0 changes](docs/releases/v0.4.0.md).

## Native configuration and data

Resources live in `model-deck.json`. Migration and provider-link import save resources and selections; native files change only after an explicit apply. Managed files are snapshotted before application, and restoration checks for external edits.

Client-owned memory files, MCP settings and unrelated native preferences remain with each client. The ChatGPT desktop application and official OAuth login are exempt. Codex CLI apply and restore manage model configuration and generated catalogs, preserving official login files even when older backups contain login snapshots.

The existing `.cc-switch` directory and `cc-switch.db` filename remain for data compatibility. Legacy tables and historical records are retained for migration and local archives; they do not activate removed features. Avoid concurrent applications writing the same database. A separate data directory can be selected in Settings.

SQL exports and database snapshots cover database records. The resource graph, device settings and native configuration snapshots are separate files; preserve the complete data directory when backing up the full setup. Personal keys, login state, runtime logs and native client sessions are not part of this repository.

The registered URL scheme is `modeldeck://`; pasted legacy `ccswitch://` provider links remain accepted. Cursor GUI and the regular cloud CLI use Cursor account services and a publicly reachable HTTPS endpoint. A verified official `cursor-agent-local` executable can use the local gateway. Consult the client guide for version constraints.

## Development

Requirements: Node.js 22.12+, pnpm 10.12.3, Rust 1.85+ and Tauri 2 platform dependencies.

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
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Build with `pnpm build`. On this Mac, `./scripts/build-modeldeck-macos.sh` creates the application, verifies its signature and archives it under `release/v<version>/`. Build products are not committed.

`pnpm dev`, `pnpm build` and `pnpm tauri` use `scripts/tauri.mjs`. On macOS it checks Rust and, when necessary, uses an installed Homebrew Z3 library. The renderer build loads its emitted library chunks to check initialization cycles that could otherwise produce a blank window.

Development and test profiles retain file and line information without full debugger symbols or incremental object caches. `src-tauri/target` is rebuildable compiler output and can be removed with `cargo clean`. Installed applications, native configuration, usage records and `release/` archives do not depend on it.

## Source and license

Project repository: [Vita0818/ModelDeck](https://github.com/Vita0818/ModelDeck). Reused code originates from [CC Switch](https://github.com/farion1231/cc-switch), originally by Jason Young and contributors. MIT notices remain in [LICENSE](LICENSE); provenance and historical baseline references are documented in [docs/UPSTREAM.md](docs/UPSTREAM.md).

Prior release notes remain under [docs/releases](docs/releases), and upstream documentation under `docs/upstream/`. Historical materials do not describe the current feature set.
