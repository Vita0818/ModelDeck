# ModelDeck

**把不同编程客户端的模型连接放在一起管理。**

ModelDeck 是你的个人桌面项目，用于管理模型提供商、端点、独立密钥、模型目录、上下文与推理设置。

## 当前版本：v0.2.0

基于 CC Switch 源码直接整合以下 10 个客户端：

| 客户端 | 入口 |
| --- | --- |
| Codex CLI | 主切换栏 |
| Claude Code | 主切换栏 |
| Claude Desktop | 主切换栏 |
| Antigravity CLI | 主切换栏 |
| Cursor CLI | 主切换栏 |
| Cursor GUI | 主切换栏 |
| Grok | 主切换栏 |
| Muse Code | 主切换栏 |
| OpenCode | 主切换栏 |
| Hermes | 主切换栏 |

切换栏、可见性设置、托盘和启动同步使用同一支持范围。新接入的四个客户端直接在主界面编辑连接；切换客户端或打开设置会保留当前应用会话里的未保存草稿。MiniMax Code、OpenClaw、Pi、Gemini CLI 不列入工具支持范围。

[本版说明](docs/releases/v0.2.0.md)。`v0.1.0` 继续保留为第一版代码快照。

## 第一版：v0.1.0

第一版完整继承当前已实现的代码，以 CC Switch v3.20.4 为上游基础，保留 Antigravity CLI、Cursor IDE、Cursor Agent CLI、Muse Code 的新增模型接入以及已有客户端功能。

原始实现的基线提交为 `10d0af62b0f5991eab61db5bf07811dfbd347660`，已保留为 `modeldeck-baseline-2026-10-04`。ModelDeck 第一版增加独立名称、图标、版本和应用标识，并关闭上游自动更新通道。

- 应用名称：**ModelDeck**
- 当前应用版本：**0.2.0**
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

仓库不包含个人 API 密钥、客户端登录状态或运行时配置。Cursor 仍需自己的账户服务及公网 HTTPS 端点，真实模型调用也需要各客户端的独立密钥。

## 来源与许可

本项目基于 [CC Switch](https://github.com/farion1231/cc-switch) 开发，保留 Jason Young 及上游贡献者的 MIT 版权声明。详见 [LICENSE](LICENSE) 与 [来源记录](docs/UPSTREAM.md)。上游历史文档保存在 `docs/upstream/`。

`upstream` 远程指向 CC Switch；个人发布仓库与在线更新通道尚未配置。
