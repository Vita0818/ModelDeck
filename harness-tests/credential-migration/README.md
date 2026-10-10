# 正式旧凭据迁移验证

2026-10-08，macOS arm64。项目状态核对发现，Arena 测试驱动此前已解析 OpenCode 的凭据引用，但正式 Rust 迁移和网关凭据读取仍把引用当成普通字符串。现已补齐产品代码。

迁移 `{env:变量名}` 会保留环境变量来源；迁移文件引用会按原 OpenCode 配置目录固定相对路径，保留引用，避免将读取后的 token 复制进新资源文件。网关在每次调用时解析引用，因此文件中轮换后的 Key 会生效。缺失、空值、未解析引用、多行或含控制字符的凭据会在发送请求前失败。LM Studio 等无需认证的本地服务仍允许空 Key。

新增五项凭据读取回归和两项旧数据迁移回归。完整 Rust 运行 **2710 passed、0 failed、15 ignored**；第十五个默认忽略项为本次新增、需明确指定现有凭据目录的只读审核。

另单独执行了该只读审核：从当前 OpenCode 旧数据库导入 OpenRouter、Arena 与 LM Studio 的三条真实记录，分别解析环境变量、文件与空凭据，并在内存中与原始来源比对。三条均通过；新资源图没有包含解析后的真实 token。原数据库、OpenCode 原生配置、Arena token 文件及官方 Codex `auth.json` 在检查前后字节一致。该审核不调用推理 API，不保存个人资源图，也不应用任何客户端配置。

结构化结果见 [summary.json](summary.json)。源码中的 opt-in 入口为 `deck::migration::tests::existing_opencode_reference_credentials_resolve_without_copying_keys`；运行需设置专用 `MODELDECK_CREDENTIAL_AUDIT_HOME`。此结果补充原有 [真实客户端矩阵](../README.md) 和 [Arena 交互式验证](../arena-retest/README.md)，没有将只读解析审核计作新的推理任务通过。
