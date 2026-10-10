# Arena Router 真实交互式复测

2026-10-07 至 2026-10-08，macOS arm64。**OpenCode、Claude Code、Codex CLI 三项完整通过。** 每项都在真实终端中启动原生交互界面，经实际 ModelDeck apply 与生产协议网关调用 Arena，使用工具读取 `[17, 29, 43]`，写入 `89`，最终由助手回复 `MD_TEST_OK`。没有用配置检查、模拟响应或用户提示词中的标记替代完成证据。

| 客户端 | ModelDeck 入口 / Arena 上游协议 | 结果与原生证据 |
| --- | --- | --- |
| OpenCode 1.18.35 | 默认 Messages 入口 → Chat Completions；另验证了 Chat 入口 | [读取、写入 89、MD_TEST_OK](results/opencode.json)；[默认配置原生会话](opencode-modeldeck-default-native.json) |
| Claude Code 2.1.280 | Messages → Messages | [读取、Edit 写入 89、MD_TEST_OK](results/claude.json)；[原生会话](native/claude.json) |
| Codex CLI 0.159.2 | Responses → Responses | [exec_command 读取、apply_patch 写入 89、MD_TEST_OK](results/codex.json)；[原生会话](native/codex-01a11991-9eba-7020-8c28-a13cc5280585.json) |

三项使用 `coding-router-preview`。结果文件分别保存在 `results/<client>/result.txt`；[summary.json](summary.json) 汇总三项完成状态。它们是此前 20 项 OpenRouter 免费模型矩阵之外的 Arena 补充测试。

## 历史 401 的更正

之前脚本直接用 `json.loads()` 读取 OpenCode 配置，将 `apiKey` 的 `{file:/Users/vita/.arena/token}` 引用原样拼接成 Bearer Key，真实 token 没有进入请求。这属于测试凭据读取错误，不能据此判断真实 Key 无效。OpenCode 原生支持这种[文件与环境变量引用](https://opencode.ai/docs/config/#files)。

已在 `run.py` 中解析 `{file:...}` 与 `{env:...}`，支持相对配置目录、用户目录和文件尾换行；缺失、空值、未解析引用和多行凭据会在网络请求前失败。六项回归测试全部通过。撤回此前“真实 Key 已无效”的结论，并恢复原先授权的 Arena 测试。你当时要求先跳过，是基于这次错误诊断，不能作为修复后继续跳过的理由。

真实凭据的 `/v1/models` 请求返回 200。普通 Python 推理探测返回 `organization_client_restricted` / 403，明确限制普通 headless 与 SDK 客户端；它同样不能证明 Key 无效。[provider-probes.json](provider-probes.json) 保留该区别。随后 OpenCode 原生终端直连完成了工具任务，直接证明真实 token 可用。

## 转发与客户端配置修复

OpenCode 直连时携带的 `x-opencode-session-id`、`x-session-affinity` 和 `x-session-id` 被网关原先的白名单丢弃，导致经 ModelDeck 的相同真实交互请求返回 403。保留其已有字段后，同一工作流返回 200 并完成。原生 User-Agent 保持原样，没有冒充其他客户端或伪造交互模式。

Claude Code 与 Codex 还需要保留原生会话和客户端来源字段，并选用各自的原生 Arena 上游协议。本次分别配置 Messages 与 Responses 资源；将它们转换为 Chat Completions 的尝试仍返回 403，不能用于推导 Key 失效。凭据与 Cookie 始终由各自认证机制处理，不通过这些上下文头透传。

Codex 的另一次 400 有明确原始错误：Arena Router 选中的 `anthropic.direct` 后端无法执行 `openai.responses/server_tool`。请求当时包含云端 `tool_search` / `web_search`。ModelDeck 的模型图声明客户端函数工具，因此其生成的 Codex 目录现在将 `supports_search_tool` 设为 false，并关闭云端 `web_search`，保留本地执行和文件编辑能力。旧版普通供应商目录在没有显式能力声明时保留其原有值。最终复测直接使用实际生成配置，已移除测试脚本对这些能力字段的额外修改。

Codex 最终任务遇到一次 502 后自行恢复，最终完成，重试证据仍保留。[request-context.json](request-context.json) 保存脱敏的原生与上游请求上下文、响应状态和已捕获的 Arena 原始错误；[gateway-records.json](gateway-records.json) 保存 ModelDeck 路径、工具类型与状态。设备指纹、安装标识与认证内容未进入公开上下文记录。

## 验证与清理

最终 Rust 回归：**2703 passed、0 failed、14 ignored**；凭据解析回归：**6 passed**。此前前端 **374 tests / 65 files passed**，本次未修改前端代码。正常 macOS release 构建、TypeScript 检查、签名与压缩包校验见 [发布验证](../../release/v0.3.0/verification.json)。受限制环境中的一次 Rust 运行有 37 个本地监听被拒绝的失败；在允许 localhost 的环境复跑后全部通过，未将该环境问题归为产品回归。

没有触发付费回退，没有观测到免费模型额度 429。Arena 调用限于你明确授权的免费 Router。上游 Key 留在网关进程中，原生客户端配置只持有隔离入口 token。

官方 Codex/ChatGPT 桌面 `auth.json` 哈希未变。测试期间正式 Codex 的当前模型设置发生了变更；清理仅删除本次原生终端新增的测试文件夹信任项，保留其余当前字段，未用旧配置覆盖并发变更。[protected-config-check.json](protected-config-check.json) 与 [codex-trust-cleanup.json](codex-trust-cleanup.json) 记录验证。

本次七个 Codex 测试会话已做可恢复归档；OpenCode 和 Claude Code 的数据位于临时目录，完成证据导出后清理。测试网关、诊断代理及入口 token 的最终清理见 [cleanup.json](cleanup.json)，公开文件的凭据扫描见 [artifact-secret-check.json](artifact-secret-check.json)。

## 保留的复测代码

`interactive.py` 启动各客户端真实 TUI，不使用 `exec`、`run`、`-p` 等 headless 模式。`capture-proxy.py` 在仅本机监听的诊断入口观察原生请求，保留实际请求头和流式正文，脱敏后记录诊断。`export.py` 从已完成会话读取工具与助手最终回复，不再调用推理 API。私有运行目录已清理；复跑需要重新准备隔离资源图与网关，不能直接依赖已删除的入口 token。
