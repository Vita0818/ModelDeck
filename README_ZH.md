# ModelDeck

**把不同编程客户端的模型连接放在一起管理。**

ModelDeck 是独立维护的个人桌面项目，用于管理模型提供商、端点、独立密钥、模型目录、上下文限制与推理档位。它复用 CC Switch 的部分原生配置与协议适配代码，并按自己的产品范围维护。

## 当前版本：v0.4.0

产品保留三个入口：

- **资源管理**：统一配置提供商、凭据、模型能力和推理档位，通过端点提供模型别名与路由。
- **客户端连接**：为客户端选择端点、默认模型和辅助模型，明确应用或恢复原生配置。
- **用量与设置**：读取支持客户端的实际用量记录，管理计价、网关、网络代理、窗口、配置目录、日志和本地数据库备份。

支持 **Codex CLI、Claude Code、Claude Desktop、Antigravity CLI、Cursor CLI、Cursor GUI、Grok、Muse Code、OpenCode、Hermes**。原生用量采集保留 Claude Code、Codex CLI、OpenCode、Grok 的解析器；其他客户端继续显示已有记录，覆盖范围以用量页的数据源说明为准。

Memory、MCP、Skills、提示词、会话浏览、OpenClaw 工作区、OAuth 账号中心、云同步、自动故障切换、脚本额度查询、工具安装器和应用自动更新的管理入口与运行时实现已移除。MiniMax Code、OpenClaw、Pi、Gemini CLI 的原生管理与用量采集也不参与构建。Gemini 模型协议仍保留，供模型连接与 Antigravity 使用。

保留各提供商的认证头、端点拼接、请求字段、推理参数和流式协议转换。编辑资源路由会影响所有引用它的连接；连接只保存引用，不复制另一套上游凭据与模型目录。

[两层管理使用说明](docs/TWO_LAYER_MODELDECK.md) · [客户端适配说明](docs/CLIENT_MODELS.md) · [v0.4.0 变更](docs/releases/v0.4.0.md)

## 原生配置与数据

资源保存在 `model-deck.json`。首次迁移和链接导入只保存模型资源及选择；使用「保存并应用」才会写入客户端配置。应用前保存所管理文件的快照，恢复时检查文件是否被外部修改。

客户端已有的记忆文件、MCP 配置和其他非模型设置由客户端自己维护。ChatGPT 桌面端及其官方 OAuth 登录属于豁免范围。Codex CLI 的应用与恢复只管理模型配置和生成目录，保留官方登录文件；旧备份中的登录快照也不会被回写。

为了读取已有数据，默认仍使用 `.cc-switch` 目录和 `cc-switch.db` 文件名。旧数据库表和历史记录保留用于迁移与本地归档，不会启用已移除的功能。不要让 ModelDeck 与其他应用同时操作同一个数据库；可在设置中选择独立数据目录。

设置中的 SQL 导出和数据库快照覆盖数据库记录。模型资源图、设备设置及原生配置快照是独立文件，备份整套配置时需要一并保留数据目录。仓库不包含个人密钥、登录状态、运行日志或原生客户端会话。

新链接使用 `modeldeck://`；手动导入兼容旧的 `ccswitch://` 提供商链接。Cursor GUI 和普通云端 CLI 仍使用 Cursor 的账户服务与公网 HTTPS 端点；已验证的官方 `cursor-agent-local` 可执行文件可使用本机模型网关。版本限制见客户端说明。

## 开发与构建

需要 Node.js 22.12+、pnpm 10.12.3、Rust 1.85+ 和 Tauri 2 的系统依赖。

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

使用 `pnpm build` 构建。本机可运行 `./scripts/build-modeldeck-macos.sh`，生成签名检查通过的应用与 ZIP，保存在 `release/v<版本>/`。构建产物不纳入 Git。

`pnpm dev`、`pnpm build` 和 `pnpm tauri` 共用 `scripts/tauri.mjs`。macOS 上该入口会检查 Rust，并在需要时使用 Homebrew 已安装的 Z3 库。前端构建会加载生成的库分块，检查会导致空白窗口的初始化循环。

开发和测试构建保留文件与行号信息，关闭完整调试符号和增量对象缓存。`src-tauri/target` 是可重建的编译目录，可用 `cargo clean` 清理；已安装应用、本地配置、用量记录和 `release/` 归档不依赖该目录。

## 来源与许可

项目仓库：[Vita0818/ModelDeck](https://github.com/Vita0818/ModelDeck)。复用代码来源：[CC Switch](https://github.com/farion1231/cc-switch)，原作者 Jason Young 及其贡献者。MIT 版权声明保留在 [LICENSE](LICENSE)，代码来源和历史基线记录在 [docs/UPSTREAM.md](docs/UPSTREAM.md)。

历史版本说明保存在 [docs/releases](docs/releases)，上游文档保存在 `docs/upstream/`。这些历史材料不代表当前产品功能。
