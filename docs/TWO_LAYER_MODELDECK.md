# ModelDeck 两层管理

从 v0.3.0 开始，模型资源与 harness 配置分别管理。

## 资源管理

在「资源管理 → 提供商与模型」中配置真实提供商的地址、协议和凭据，然后添加它的模型、上下文、输出长度、推理档位、工具调用和图片输入能力。提供商凭据只在这里维护。

凭据支持直接保存的 Key、环境变量，以及 `{file:/绝对路径}`、`{file:~/路径}` 文件引用。文件引用在网关调用时读取，引用内容不会作为迁移副本写入资源文件。迁移 OpenCode 的 `{env:变量名}` 时会转成对应的环境变量来源；相对文件引用会按原 OpenCode 配置目录固定路径。新资源中手工填写的相对文件引用以 ModelDeck 配置目录为基准。环境变量必须能被 ModelDeck 进程读取，缺失或格式错误的引用会在发送上游请求前报告。

在「端点与路由」中创建供客户端使用的端点。一个端点可以提供多个协议和多个对外模型名，每个对外模型引用一个全局模型资源，并选择该模型支持的默认档位。例如：

```text
端点「编程」
  coding → 提供商 A / 模型 X / high
  fast   → 提供商 B / 模型 Y / 提供商默认
```

修改 `coding` 的后台资源或默认档位，会影响所有使用这个端点和模型名的调用。客户端发送的档位、预算、开关和其他原生推理配置优先透传；同协议不重写这些字段，跨协议只转换所需字段和枚举大小写。客户端未发送推理配置时才补路由默认，默认也留空则使用上游默认。目录能力只用于界面与默认选项，不在请求中强行升降档或过滤新值；不支持的值由上游返回错误。请求按当前全局资源解析，无需在各个 harness 中重新填写上游地址、密钥或真实模型 ID。

资源删除受到引用检查保护。先移除端点中的模型映射，再删除模型资源；先清除引用该端点的选择并恢复已应用配置，再删除端点。

## harness 选择

选择主切换栏中的客户端，然后选择端点、默认对外模型和可选的辅助模型。

- **保存选择**：保存引用关系，保留当前客户端配置。
- **保存并应用**：启动网关，为该客户端生成原生配置或启动命令。
- **恢复原生配置**：恢复首次应用前的配置。如果原生文件在应用后又被其他程序修改，恢复会停止并说明对应文件，以保留这些修改。

Claude Code、Antigravity、Muse、Cursor CLI、DeepSeek Harness 和 GitHub Copilot CLI 使用页面提供的托管启动命令；命令调用原来的客户端可执行文件，已应用连接的命令在重新打开页面后仍可查看。Claude Code 不支持在 settings.json 中持久保存 max，托管命令通过 --effort 设置所选模型的默认档位，不设置会锁定运行时选择的环境变量。显式 --effort 和会话中的 /effort 可覆盖初始值。当前 Muse 会把 ultra 和 xhigh 都发成 xhigh，网关保留该原生值，不将其升级为 max。该版本无法原生发送 max，托管启动默认使用其支持的 xhigh，实际请求仍以客户端发送的值为准。Cursor GUI 和 Claude Desktop 需要重启客户端加载配置。

每个 harness 保存的是端点 ID、默认模型名、辅助模型名和必要的原生适配信息。模型目录从全局资源生成。新增、删除对外模型或修改目录中的上下文信息后，再应用一次需要更新原生模型菜单的客户端。

## 兼容组合

「资源管理 → 兼容范围」列出每个端点与十二个 harness 的兼容模型数量。完全兼容时可以自由组合；数量按实际协议转换和客户端限制计算。

| harness 接入协议 | 可调用的上游协议 |
| --- | --- |
| OpenAI Responses | Responses、Chat Completions、Anthropic Messages |
| Anthropic Messages | Anthropic Messages、Chat Completions、Responses、Gemini |
| Gemini | Gemini、Chat Completions |
| OpenAI Chat Completions | Chat Completions |

OpenCode、Hermes、DSH 和 Copilot CLI 根据端点及模型自动选择 Anthropic Messages、OpenAI Chat Completions 或 OpenAI Responses 接入协议。已应用的协议会保留在绑定中；资源修改不能使已有绑定失去兼容性。

普通 Cursor 云端 CLI 与 Cursor GUI 使用公开可访问的 HTTPS 网关地址。端点的「公开网关地址」填写该端点的外部入口，例如 `https://gateway.example/deck/<端点 ID>`。选择经过校验的官方 `cursor-agent-local` 可执行文件后，Cursor CLI 使用本机网关和官方 authless 模式；该本地运行包需单独准备，当前验证的版本与路径见 [本地 CLI 验证](../harness-tests/cursor-direct/README.md)。客户端使用网关接入凭据，上游密钥仍留在 ModelDeck。

Claude Desktop 的对外模型名需要符合其原生菜单要求，例如 `claude-sonnet-5`。这个名称可以映射到任意兼容的后台模型。Antigravity 固定的后台辅助请求使用所选辅助模型，未指定时使用默认模型。原生档位分别读取 Responses 的 reasoning.effort、Chat 的 reasoning.effort/reasoning_effort、Messages 的 output_config.effort 和 Gemini 的 thinkingLevel。同协议完整保留原生推理配置。Gemini 转 OR Chat 时将档位和预算转换为 reasoning.effort/max_tokens，显式关闭和自动预算保留相应含义，不按预算猜测档位。

托管客户端配置使用固定的网关监听端口。在设置中配置端口后再应用选择。

## DeepSeek Harness 与 GitHub Copilot CLI

在客户端列表中选择 **DeepSeek Harness** 或 **GitHub Copilot CLI**，选择端点和默认模型后点击「保存并应用」。需要已安装对应客户端；可执行文件留空时会从 PATH 和常见安装目录查找，也可以指定原始可执行文件的绝对路径。受管启动命令目前支持 macOS 和 Linux。

DSH 使用官方 profile 的 `--patch` 配置层，生成 `deck-clients/dsh.patch.yml`，配置 `llm-pi-ai` 模型目录、默认模型和标题辅助模型。目录保留上下文、输出上限、图片输入能力及 DSH 支持的推理档位；不支持的原生档位不会改名为其他档位。默认打开 Web 界面；在启动命令后添加 `headless "任务"`、`web --port 3080` 或 `--profile <名称>` 可切换运行方式。原有 profile、插件、凭据和会话继续由 DSH 管理。此适配面向支持 `--profile` / `--patch` 的新版 DSH，不使用旧版 `settings.yaml` 导入机制。

Copilot CLI 使用官方 BYOK 接口。启动命令设置独立的提供商配置路径和当前模型的上下文、输出上限；可以通过 `--model <对外模型名>` 选择同一连接中的其他模型。Copilot CLI 当前没有独立辅助模型选择，连接页不提供该字段。原生登录、用户提供商目录和其他偏好继续由 Copilot 管理。

两个启动命令都会在运行时读取 `deck-gateway-url`，跟随当前网关端口；它们只持有端点的本地访问令牌，上游密钥仍由 ModelDeck 解析。配置和命令保存在 ModelDeck 数据目录的 `deck-clients/` 与 `bin/` 下，应用失败会回滚，恢复操作会检查外部修改。修改资源目录后重新应用以刷新原生目录；上游路由和凭据修改在后续网关请求中生效。

参考：[DSH 启动命令](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/cli-help.md)、[DSH 自定义模型 API](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/user/guide/providers.md)、[Copilot CLI BYOK](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-byok-models)、[Copilot CLI 提供商配置路径](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference#providersjson)。

## 现有数据

首次打开会读取旧版本的提供商和客户端连接，按地址、协议、凭据合并资源，再建立引用关系。同一凭据下的同一模型复用全局模型资源；不同账号或密钥保持独立。

迁移会保留原数据库、连接文件和原生登录配置。迁移得到的选择需要点击「保存并应用」才会使用新的共享端点。原生登录和专用配置的保留记录可以在资源管理页面查看。缺少明确模型限制的旧记录使用可编辑的初始限制；请按提供商实际支持的能力调整。

统一资源文件为当前配置目录中的 `model-deck.json`，默认位于 `~/.cc-switch/model-deck.json`。它包含提供商、模型、端点和引用关系；原生配置恢复记录位于同目录的 `deck-backups/`。备份统一资源时请包含这些文件。旧版本的数据库导出继续用于旧数据，不能单独替代统一资源文件的备份。

## 开发约定

- `src-tauri/src/deck/types.rs` 定义唯一资源结构和 harness 引用结构。
- `deck/mod.rs` 负责验证、兼容性、版本并发检查和管理命令。
- `deck/gateway.rs` 在每次调用时解析当前引用并调用已有协议转换器。
- `deck/native.rs` 生成原生配置；生成目录和恢复快照均不作为资源维护入口。
- 旧 `providers` 表和 `model-clients.json` 用于迁移、兼容和恢复，不作为新界面的模型资源来源。

共享路由的目录和响应仅返回对外模型名。上游地址、密钥与真实模型 ID 不写入新的 harness 接入配置。
