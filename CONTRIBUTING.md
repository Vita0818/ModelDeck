# Contributing to ModelDeck

ModelDeck is a personal fork built from the current working client model manager. The `main` branch is the development branch. Use conventional commit messages and keep changes focused.

The model management scope covers providers, endpoints, independent credentials, model catalogs, context limits and reasoning settings. Preserve existing client authentication and unrelated native settings when implementing model connections.

Run the frontend type check, formatting check and unit tests. For Rust changes, run formatting, Clippy and relevant unit/integration tests. Validate any client transport change against the real native client with temporary configuration and local mock services before claiming live compatibility.

Never commit API keys, local databases, runtime profiles or build caches. Keep upstream attribution and the MIT license. ModelDeck releases must use ModelDeck's own identity and must not enable CC Switch's automatic update channel.

See [README](README.md) for commands. [Historical upstream contribution guidance](docs/upstream/CONTRIBUTING.md) is retained as reference.
