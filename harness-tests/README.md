# ModelDeck 真实 Harness 测试

2026-10-07，macOS arm64。10 个 harness × 2 个指定免费模型，共 20 个计划项。
**12 项完整通过、6 项上游失败、2 项 Cursor 登录阻塞。** 未把配置投影或 mock 结果计作真实模型通过。后续 Cursor 本地 CLI 试验见 [cursor-direct](cursor-direct/README.md)。

每个可执行会话的任务是：使用工具读取 `fixture.json` 中的 `[17, 29, 43]`，计算总和，把 `89` 写到 `result.txt`，最终回复 `MD_TEST_OK`。CLI 还要求正常退出。每次重试前清空结果文件；桌面端另以原生界面核对任务完成。

模型 ID：

- `nvidia/nemotron-3-ultra-550b-a55b:free`
- `thinkingmachines/inkling:free`（平台实际拼写为 Inkling）

两者在本轮 OpenRouter 模型目录中的输入、输出价格均为 0。模型目录快照保存在 [model-metadata.json](model-metadata.json)。本轮没有观察到明确的免费模型额度 429，付费回退调用为 **0**。

| Harness | Nemotron 3 Ultra free | Inkling free |
| --- | --- | --- |
| Codex CLI | [通过：工具读取、写回 89、MD_TEST_OK](results/codex/nemotron.json) | [通过：工具读取、写回 89、MD_TEST_OK](results/codex/inkling.json) |
| Claude Code | [通过：工具读取、写回 89、MD_TEST_OK](results/claude/nemotron.json) | [通过：工具读取、写回 89、MD_TEST_OK](results/claude/inkling.json) |
| Claude Desktop | [通过：工具读取、写回 89、MD_TEST_OK](results/claude-desktop/nemotron.json) | [通过：工具读取、写回 89、MD_TEST_OK](results/claude-desktop/inkling.json) |
| Antigravity CLI | [通过：工具读取、写回 89、MD_TEST_OK](results/antigravity/nemotron.json) | [上游 403：与 Inkling 应用准入限制一致](results/antigravity/inkling.json) |
| Cursor CLI | [上游 503 / 流内服务过载，未完成](results/cursor-agent/nemotron.json) | [通过：工具读取、写回 89、MD_TEST_OK](results/cursor-agent/inkling.json) |
| Cursor GUI | [阻塞：Cursor 账号未登录，未启动模型会话](results/cursor/nemotron.json) | [阻塞：Cursor 账号未登录，未启动模型会话](results/cursor/inkling.json) |
| Grok | [通过：工具读取、写回 89、MD_TEST_OK](results/grokbuild/nemotron.json) | [上游 403：与 Inkling 应用准入限制一致](results/grokbuild/inkling.json) |
| Muse Code | [上游 503 / 流内服务过载，未完成](results/muse/nemotron.json) | [上游 403：与 Inkling 应用准入限制一致](results/muse/inkling.json) |
| OpenCode | [通过：工具读取、写回 89、MD_TEST_OK](results/opencode/nemotron.json) | [通过：工具读取、写回 89、MD_TEST_OK](results/opencode/inkling.json) |
| Hermes | [通过：工具读取、写回 89、MD_TEST_OK](results/hermes/nemotron.json) | [上游 403：与 Inkling 应用准入限制一致](results/hermes/inkling.json) |

## 如何读结果

每个表格单元格链接到相应的结构化结果；[summary.json](summary.json) 提供完整矩阵，[logs](logs) 保存脱敏输出，[attempts](attempts) 保存重试前的失败证据，[results](results) 保存最终记录和测试文件。网关的状态、路径、原生 User-Agent 与工具名称保存在 [gateway-records.jsonl](gateway-records.jsonl)。请求正文和凭据未保存到这份网关记录。

Antigravity、Grok、Muse、Hermes 的 Inkling 请求返回 403。独立 API 探测返回“仅开放给 agentic harnesses、编程或生产力应用”的信息，因此把这些 403 与应用准入限制关联是有依据的推断；网关返回给这些客户端的正文只保留 HTTP 状态，未逐个取得它们对应的原始上游错误正文。证据见 [provider-inference-probes.json](provider-inference-probes.json)。

Muse 的 Nemotron 重试仍遇到 Nvidia 的流内 503 / service temporarily overloaded。更早一次尝试还出现了模型把整数工具参数输出为字符串的错误。两类失败均未触发付费模型回退。

Claude Desktop 的两项均通过其真实 Code 原生界面执行。原生菜单显示厂商模型名称；临时 ModelDeck 路由把该连接的所有别名映射到当次指定的免费资源，结果文件记录了真实上游 ID。它自己的已保存 OpenRouter 凭据有效。早期辅助模型误指向 Arena，并受到测试脚本未解析 Key 文件引用的错误影响；修正临时路由后通过。该次 401 不能用于判断真实 Arena Key 是否有效。辅助 `HEAD /api/hello` 的 401 和 `messages/count_tokens` 的 404 仍出现在记录中，完整工具任务已完成。

Cursor CLI 后续使用官方 `agent-cli-local` 包，通过原生 authless 本地 API 调用实际 ModelDeck 网关。Inkling 完成了原生 Read、Shell 写入 `89`、再次读取校验和 `MD_TEST_OK`，正常退出；Nemotron 的后续请求返回 Nvidia 流内 503，未完成。原始云端登录失败记录已保留在 [attempts/cursor-agent](attempts/cursor-agent)。Cursor GUI 的当前 3.9.16 构建包含本地引擎，但编译开关 `localMode` 为 false，尚未验证无登录 GUI 会话。原有 GUI 登录态及窗口未被改动，GUI 凭据未用于 CLI。详见 [本地试验记录](cursor-direct/README.md)。

Arena 的历史 401 来自测试脚本把 `{file:/Users/vita/.arena/token}` 原样当作 Bearer Key 发送，真实 token 当时没有被读取。已修复文件与环境变量引用解析，撤回“真实 Key 无效”的判断，并按你原先授权继续真实交互式复测。最新结果和具体路径见 [Arena 复测](arena-retest/README.md) 与 [结构化结果](arena-retest/summary.json)；历史失败记录仍保留，不计入原有 20 项免费模型矩阵。

## 本轮修复

- 网关保留真实 harness 的 User-Agent 和 OpenRouter 应用归属头，使 Codex、Claude Code、OpenCode 的 Inkling 请求能够通过；认证头、Cookie 和本地路由头由原有凭据机制处理。
- Chat → Responses 的 `response.failed` 事件补齐字符串 `error.code`。真实 Grok 和 Muse 在遇到上游过载时能够解析错误，Grok 的 Nemotron 工具工作流随后通过。
- 增加了实际原生配置到共享网关的头部/凭据隔离断言，以及上游仅给出错误消息时的 Responses 反序列化回归测试。
- Arena 复测补齐原生会话与客户端标识透传，保留真实客户端已有字段，避免网关丢弃上游识别交互会话所需的信息；本地凭据与 Cookie 仍不透传。Arena 为 Claude Code 使用 Messages、为 Codex 使用 Responses、为 OpenCode 使用 Chat Completions。
- 正式旧配置迁移与网关现已解析 OpenCode 的文件和环境变量凭据引用，保留动态来源，不把引用当作真实 Key。已用现有 OpenRouter、Arena、LM Studio 记录做只读导入与解析验证，原数据库、配置及官方 OAuth 文件保持原状。详见 [迁移验证](credential-migration/README.md)。

凭据迁移修复后的完整 Rust 回归通过：**2710 passed、0 failed、15 ignored**。另有 **6 项凭据解析回归测试通过**。此前前端已执行 **374 tests / 65 files passed**，本次未改动前端代码；macOS release 构建与包校验以 [verification.json](../release/v0.3.0/verification.json) 为准。

## 凭据与会话清理

临时测试网关为 harness 使用独立入口 token，上游 key 只在网关进程中解析。Codex CLI、Claude Code、Claude Desktop 使用各自现有凭据；其余测试网关入口使用 OC_OR_KEY，其中 Antigravity、Cursor GUI、Cursor CLI、Muse 的本轮临时复用由你明确授权。所有测试结束后已移除私有运行目录和临时依赖。

CLI 使用临时配置/数据目录，或原生的 ephemeral / no-session-persistence / no-session-log 选项。清理临时目录移除了本轮 CLI 会话和重试的数据。两个 Claude Desktop 会话以可恢复方式归档，原生 UI 已验证；现有 Cowork 会话保持原状。临时隧道、网关、HTTPS 入口和隔离 Cursor 进程均已结束，原有 Cursor 窗口保留。

[cleanup.json](cleanup.json) 记录进程归属、端口关闭与会话归档结果；[protected-config-check.json](protected-config-check.json) 确认正式配置与 ChatGPT/Codex 桌面 OAuth 文件在测试及 Rust 回归后保持原状；[artifact-secret-check.json](artifact-secret-check.json) 记录保留文件的凭据检查。

## 复跑入口

`run.py` 是本机已有凭据下的真实客户端驱动；`report.py` 仅从已保留结果生成报告，不调用 API。私有运行环境已清理，复跑时需先重新准备：

```sh
python3 harness-tests/run.py prepare
python3 harness-tests/run.py gateway
# 在另一个终端选择客户端和免费模型：
python3 harness-tests/run.py run codex nemotron
python3 harness-tests/run.py run codex inkling
python3 harness-tests/report.py
```

`gateway` 使用忽略的 Rust 实时测试 fixture，调用实际的 ModelDeck apply 和协议网关。其余 CLI 名称见 `run.py` 的 `CLIENTS`。Hermes 依赖已移除；如需复跑，可按 [hermes-runtime-source.json](hermes-runtime-source.json) 的官方仓库和 commit 重建临时环境。Cursor 本地 CLI 的独立复跑入口和版本信息见 [cursor-direct](cursor-direct/README.md)。GUI 项需重新执行原生界面流程；普通 Cursor 云端 CLI 与 GUI 仍使用账号和公网 HTTPS。`public_ingress.py` 是只开放经 token 认证的 Cursor 测试路径的临时入口。

`MD_TEST_INCLUDE_ARENA=1` 会启用 Arena 的目录预检。Arena 的本轮授权交互复测见 `arena-retest/interactive.py`，需在真实终端执行各客户端原生界面；普通 headless 探测的准入失败不能据此判断真实 Key 无效。付费回退只有在捕获到上游 429、且错误明确包含免费模型限流信息时才能触发；认证、准入、过载、协议与超时错误均保持在指定免费模型上。


## Copilot 桌面端本地验证

使用已安装的官方桌面端 SDK 和 Copilot CLI，验证原生模型记录到共享网关的 Chat Completions、Responses、Anthropic 请求。配置、会话和数据库均使用临时目录；上游为本机模拟服务，不读取真实账号凭据，也不调用付费模型。

```sh
MODELDECK_NATIVE_COPILOT=/absolute/path/to/copilot \
MODELDECK_NATIVE_COPILOT_SDK='/Applications/GitHub Copilot.app/Contents/Resources/copilot-sdk/index.js' \
cargo test --manifest-path src-tauri/Cargo.toml --lib \
  deck::copilot_desktop::tests::installed_desktop_sdk_uses_the_database_model_records \
  -- --ignored --exact
```
