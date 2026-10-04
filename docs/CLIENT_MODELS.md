# 客户端模型扩展

ModelDeck **v0.1.0** 继承基于 CC Switch **v3.20.4** 的当前实现，包含「客户端模型」入口。每个客户端可以保存多个命名连接，分别管理端点、密钥来源、模型目录、默认模型和可用的推理设置。

## 支持范围

| 客户端 | 接入方式 | 已验证版本 | 运行条件 |
| --- | --- | --- | --- |
| Antigravity CLI | 原生自定义模型配置；Gemini 直通或 Gemini → OpenAI Chat 转换 | 1.2.16 | 自定义模型需要 1.2.16 或更新版；受管启动命令和 ModelDeck 路由服务 |
| Cursor IDE | 原生 SQLite 中的 OpenAI BYOK、模型目录、Agent/Plan 默认模型 | 3.9.16 的配置结构 | 应用配置时退出 Cursor；公网 HTTPS 端点；Cursor 自己的账户 |
| Cursor Agent CLI | 在启动进程中接入模型目录，并通过原生 `RequestedModel.apiKeyCredentials` 发送 BYOK 连接 | 2026.06.12-19-59-36-f6aba9a、2026.10.01-e373342 | 受管启动命令；公网 HTTPS 端点；Cursor 自己的账户 |
| Muse Code | 原生模型目录、Responses 协议与 SSE 适配；可连接 Responses、Chat 或 Anthropic 后端 | 0.1.0-R708.1 | 受管启动命令和 ModelDeck 路由服务 |

Cursor 的两种客户端均经过 Cursor 的服务转发。模型上下文、输出预算和推理能力受 Cursor 及上游提供商约束；目录中的数值不会伪装为已经覆盖 Cursor 的运行时限制。OpenAI BYOK 连接应使用提供商限定的模型 ID，例如 `anthropic/claude-opus-5.5`；裸 `claude-*`、`gemini-*` 名称属于 Cursor 的其他原生提供商槽位。

Cursor Agent 适配器会校验原始入口及模型模块的 SHA-256。已验证的版本自动使用对应适配；未知结构会在启动前报错。适配器在内存中补上原生 CLI 未传递的 BYOK 字段，账户验证和服务端权限仍由 Cursor 处理。更新 CLI 后如校验未通过，需要重新验证并更新兼容清单。

## 使用

1. 打开「客户端模型」，选择客户端。
2. 填写连接名称、端点和协议。每个客户端使用自己的密钥或环境变量。
3. 可以从一个已有 OpenCode 提供商导入模型目录。导入只取模型信息，不取该提供商的密钥。选择当前端点实际提供的模型。
4. 选择默认模型和适用的推理档位。Antigravity 可以单独选择标题等后台请求使用的辅助模型。
5. 「保存连接」保存草稿和命名连接；「应用到客户端」才激活它。
6. Cursor IDE 重新打开后读取配置。三个 CLI 使用页面给出的受管启动命令；命令仍执行现有安装的客户端。
7. 「恢复原生默认」撤销这次受管的模型选择和连接字段，之后使用原客户端命令。它保留命名连接，便于再次应用。

默认的独立变量名：

| 客户端 | 变量 |
| --- | --- |
| Antigravity CLI | `AGY_OR_KEY` |
| Cursor IDE | `CURSOR_OR_KEY` |
| Cursor Agent CLI | `CURSOR_CLI_OR_KEY` |
| Muse Code | `MUSE_OR_KEY` |

通过环境变量提供密钥时，Antigravity/Muse 的上游变量必须能被 **ModelDeck 进程**读到；从 Finder 打开的应用通常不继承终端的 shell 配置。也可以在界面选择独立 API 密钥。Cursor Agent 的环境变量在启动 CLI 时读取。

Muse 0.1.0 的原生推理选项没有 `max`；选择 `max` 时受管启动命令使用 `xhigh`。无推理能力的模型不会收到 Muse 默认附带的推理参数。Antigravity 的 Chat 桥接会使用配置中的上游推理档位；Gemini 3 原生协议使用原生 `minimal/low/medium/high` 级别。Gemini 2.5 的预算机制不同，应使用提供商默认配置。

## 配置与生命周期

- 连接、草稿与生效状态保存在 ModelDeck 配置目录的 `model-clients.json`，Unix 权限为 `0600`。
- 启动脚本位于该目录的 `bin/`，权限为 `0700`。脚本包含客户端本地路由凭据，不包含上游 API 密钥。
- Cursor Agent 的适配器和兼容清单位于 `adapters/`。
- Antigravity/Muse 通过现有 ModelDeck 服务的独立路径运行，每个客户端有独立本地鉴权令牌。
- `model-gateway-url` 记录当前监听地址。启动脚本读取它以跟随端口变化；Muse 的已受管端点 pin 同步更新。
- 生效的 Antigravity/Muse 连接，以及 Claude Desktop 的代理模式，会在 ModelDeck 启动时恢复所需服务。
- 保存连接不改变当前路由；应用失败会回滚原生文件。Cursor 的原生修改使用 SQLite 事务。
- 切换连接会移除上一连接引入的模型条目，保留其他手动配置的模型和非模型设置。

本扩展不把新客户端加入 ModelDeck 的 Skills、MCP、提示词或订阅管理流程。Cursor 的账户令牌、订阅状态不参与 BYOK 配置更新。

Claude Desktop 同时修复了重新应用配置时的模型家族顺序：Fable → Opus → Sonnet → Haiku。网关配置只更新推理字段，并保留已有的其他偏好。

## 验证

常规检查：

```sh
pnpm typecheck
pnpm format:check
pnpm test:unit
cd src-tauri
cargo fmt --check
cargo clippy
cargo test
```

原生客户端测试使用临时配置目录、本地模拟服务和测试专用密钥，不产生真实模型费用：

```sh
CC_SWITCH_NATIVE_AGY=/absolute/path/to/agy \
CC_SWITCH_NATIVE_MUSE=/absolute/path/to/muse-native-binary \
cargo test --lib model_clients::gateway::native_tests -- --ignored --nocapture
```

Cursor 原生 RPC 请求检查（在仓库根目录执行）：

```sh
python3 tests/native-cursor-probe.py \
  --client /absolute/path/to/cursor-agent \
  --adapter src-tauri/resources/cursor-models.cjs
```

Cursor 测试使用内存凭据存储、临时配置和本地 HTTP/2 服务，检查原生请求中的模型、端点和专用密钥，并确认控制台没有输出密钥。这个检查验证客户端到 Cursor 协议边界，不代表已经执行真实提供商调用；实际调用还需要有效的独立上游密钥和 Cursor 账户权限。

## 构建维护

这是 ModelDeck 的第一个独立版本。应用更新通道已与上游分离，v0.1.0 使用本地构建更新。第一版继续兼容 `.cc-switch` 数据目录；后续开发以本仓库和 `v0.1.0` 标签为起点。

本机 Homebrew Rust 1.97.1 构建时需要保留宏依赖的元数据。优化构建已使用 `CARGO_PROFILE_RELEASE_STRIP=none` 验证；这仍是 release 优化，并非 debug 构建。若该 Homebrew LLVM 安装指向旧 Z3 动态库，可在构建进程中设置 `DYLD_FALLBACK_LIBRARY_PATH` 指向已安装的对应 Z3 库目录，无需改动系统链接。

参考：[Antigravity CLI 文档](https://www.antigravity.google/docs/cli/install/)、[Gemini thinking 配置](https://ai.google.dev/gemini-api/docs/generate-content/thinking)、[Cursor API keys](https://docs.cursor.com/settings/api-keys)、[Muse Code 配置](https://dev.meta.ai/docs/muse-code/configuration)。
