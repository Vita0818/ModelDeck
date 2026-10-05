# ModelDeck

**把不同编程客户端的模型连接放在一起管理。**

ModelDeck 是你的个人桌面项目，用于管理模型提供商、端点、独立密钥、模型目录、上下文与推理设置。

## 当前版本：v0.3.0

ModelDeck 现在分为两个入口：

- **资源管理**：统一配置真实提供商、密钥、模型、能力和档位，并通过端点建立对外模型映射。
- **harness 选择**：为每个客户端选择端点、默认模型和辅助模型；原生配置与模型目录由适配器生成。

同一个后台资源可以供多个 harness 使用，同一个 harness 可以选择不同的兼容资源。修改后台路由后，所有引用它的调用使用新的资源。每个客户端不再维护另一份完整的上游模型配置。

支持 **Codex CLI、Claude Code、Claude Desktop、Antigravity CLI、Cursor CLI、Cursor GUI、Grok、Muse Code、OpenCode、Hermes**。MiniMax Code、OpenClaw、Pi、Gemini CLI 不列入工具支持范围。

首次打开会迁移旧连接并保留原配置。迁移本身不应用客户端配置；使用「保存并应用」启用共享端点。

[使用说明：两层管理](docs/TWO_LAYER_MODELDECK.md) · [v0.3.0 说明](docs/releases/v0.3.0.md)。`v0.1.0` 和 `v0.2.0` 保留为既有版本快照。

## 第一版：v0.1.0

第一版完整继承当前已实现的代码，以 CC Switch v3.20.4 为上游基础，保留 Antigravity CLI、Cursor IDE、Cursor Agent CLI、Muse Code 的新增模型接入以及已有客户端功能。

原始实现的基线提交为 `10d0af62b0f5991eab61db5bf07811dfbd347660`，已保留为 `modeldeck-baseline-2026-10-04`。ModelDeck 第一版增加独立名称、图标、版本和应用标识，并关闭上游自动更新通道。

- 应用名称：**ModelDeck**
- 当前应用版本：**0.3.0**
- 应用标识：`com.vita.modeldeck`
- 主分支：`main`
- 使用说明：[客户端模型连接](docs/CLIENT_MODELS.md)
- 第一版说明：[v0.1.0](docs/releases/v0.1.0.md)

## 开发与构建

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

使用 `pnpm build` 构建；本机也可运行 `./scripts/build-modeldeck-macos.sh`，脚本会处理当前 Homebrew Rust 的构建环境。构建产物放在 `release/`，不纳入 Git。

## 第一版兼容约定

为了保留现有配置契约，v0.1.0 暂时继续使用 `.cc-switch` 数据目录、数据库文件名与内部 Rust 库名。产品本身已经使用 ModelDeck 的独立应用标识。ModelDeck 与 CC Switch 不应同时使用同一个配置目录；需要隔离时可在设置中选择不同目录。

新链接使用 `modeldeck://`；手动导入仍兼容旧的 `ccswitch://` 链接。ModelDeck 没有配置 CC Switch 的更新地址或更新签名公钥，更新使用自己的本地构建。

仓库不包含个人 API 密钥、客户端登录状态或运行时配置。Cursor 仍需自己的账户服务及公网 HTTPS 端点，上游密钥统一绑定到提供商，harness 使用网关接入凭据。

## 来源与许可

本项目基于 [CC Switch](https://github.com/farion1231/cc-switch) 开发，保留 Jason Young 及上游贡献者的 MIT 版权声明。详见 [LICENSE](LICENSE) 与 [来源记录](docs/UPSTREAM.md)。上游历史文档保存在 `docs/upstream/`。

`upstream` 远程指向 CC Switch；个人发布仓库与在线更新通道尚未配置。
