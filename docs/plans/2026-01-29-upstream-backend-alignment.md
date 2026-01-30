# cc-switch-cli ↔ cc-switch（upstream）后端对齐文档（渐进式）

> **目标**：在保持 `cc-switch-cli`「纯 CLI + TUI」产品形态不变的前提下，使 Rust 后端能力/数据结构/关键行为尽可能与上游 `cc-switch` 对齐，从而降低后续同步/移植成本，并按阶段逐步补齐上游关键特性。
>
> **对齐基线（用于复现差异）**
> - `cc-switch-cli`：`a6df9cb1680ab59afda1576bfb2636b519359eb9`
> - `cc-switch`（upstream）：`08d9bb4cab08c41ac107a550f9ca12267c846d15`
> - 生成日期：2026-01-29

---

## 0. 当前进度（截至 2026-01-30）

- ✅ 已完成：Phase 1（正确性对齐）— `df101b0`
- ✅ 已完成：Phase 2（数据结构/核心语义对齐）— `ae8e93f`、`6a7e15d`、`ba36f2a`、`07167e7`
- ✅ 额外对齐：deeplink provider 导入能力（与 upstream 协议兼容的解析 + 导入）— `df101b0`、`dd39ca7`
- ✅ 已落实：移除 OpenCode 范围（计划层面）— `53aca67`
- ⏳ 未开始：Phase 3（Skills 系统重做）
- ⏳ 未开始：Phase 4（Proxy/Failover/Stream Check/Usage）

> 注：上述 commit 为本仓库 `cc-switch-cli` 的提交号，用于追溯实现与回归依据。

## 1. 背景与约束

### 1.1 背景
- upstream `cc-switch` 是「Tauri App + 前端 UI」形态，但其 Rust 后端已经沉淀了较多系统能力（Proxy、Failover、DB、Stream Check 等）。
- 本仓库 `cc-switch-cli` 已经是独立的 CLI 形态（clap/inquire/TUI），其后端以 `~/.cc-switch/config.json` 为 SSOT（单一事实来源），整体更轻量。

### 1.2 核心约束（必须遵守）
- **保持 CLI 特性**：不引入 tray/auto-launch/deeplink 等 GUI 绑定行为；不要求用户常驻后台进程（如需常驻由用户自行用 systemd/launchd/tmux 托管）。
- **尽量不破坏现有用户数据/行为**：涉及 live 配置写入、格式迁移、路径变更时，必须提供备份/回滚或兼容读取。
- **输出稳定 + i18n 友好**：用户可见字符串保持稳定、必要时走 `src-tauri/src/cli/i18n.rs`。
- **渐进式**：每阶段都应有可回归的测试与验收标准，避免“大爆炸式对齐”。

---

## 2. 差异总览（按域）

> 结论：两仓库目前最大的结构差异不是“代码写法”，而是 **状态/存储层（DB vs config.json）** 与 **系统能力（proxy/stream-check/usage 等）**。

### 2.1 状态与存储（SSOT）
- upstream：`AppState { db: Database, proxy_service }`，多数业务通过 SQLite DAO 读写。
- CLI：`AppState { config: RwLock<MultiAppConfig> }`，多数业务写 `~/.cc-switch/config.json`。

### 2.2 Provider（供应商）
对齐风险最高的差异点（需优先明确策略）：
- **Codex auth 语义冲突**：upstream 强制 `auth.json` / `settings_config.auth` 存在；CLI 当前允许“无 auth”（例如凭证存储/环境变量）。
- **Gemini live 写入策略**：upstream 倾向 merge 到现有 `~/.gemini/settings.json`（保留 `mcpServers` 等）；CLI 存在覆盖整文件风险。
- **数据结构不对齐**：upstream `ProviderManager.providers: IndexMap`（稳定顺序）、新增 `inFailoverQueue`、`ProviderMeta` 更多字段、`UsageScript.templateType` 等；CLI 缺。

### 2.3 MCP / Prompt / Skill
差异点与优先级：
- **MCP：Gemini 导入/验证差异**（高风险）：upstream 对 `httpUrl/type/timeout` 有更完善双向转换；CLI 可能把缺省 `type` 误判为 `stdio` 导致导入失败或导入为 0。
- **MCP：upsert 时“取消勾选 app => 从 live 移除”**：upstream 自洽；CLI 目前更多依赖 `toggle_app` 路径，`upsert` 不会清理取消项。
- **Skill：几乎两套系统**：upstream 有 SSOT `~/.cc-switch/skills/` + 多 app 同步（symlink/copy fallback）+ 扫描未管理技能；CLI 仍偏 Claude 单目录安装且 CLI 命令层基本占位。

### 2.4 Proxy / Failover / Stream Check / Usage / Database
- upstream：本地 HTTP proxy、熔断器、请求路由与 failover、stream-check、usage 日志与成本、SQLite 持久化（含 live 备份）。
- CLI：目前无 proxy/stream-check/usage 统计/DB。

### 2.5 命令面（CLI UX）
共同命令域：`provider`、`mcp`、`config`、`env`、`prompts`、`skills`。  
upstream 有而 CLI 缺：`proxy/global_proxy`、`failover`、`stream_check`、`usage`、`settings`、`import_export(sql)` 等。  
CLI 特有：`interactive(TUI)`、`completions`。

---

## 3. 关键决策点（需要先定，否则路线会分叉）

### D1：是否要把 CLI 收敛到 upstream 的 DB 架构？
选项：
- **A. 保持 `config.json` 作为 SSOT（推荐）**：只对齐数据结构与关键行为；需要时引入“可选的轻量状态文件/日志文件”，不把配置迁移到 SQLite。
- **B. 迁移到 SQLite DB 作为 SSOT（同构 upstream）**：对齐成本高、影响面大，但能最大程度复用 upstream 的 proxy/usage/stream-check/dao 体系。
- **C. 混合**：配置仍是 `config.json` SSOT，但引入 SQLite 仅做 logs/usage（可选 feature），不接管 providers/mcp/prompts 的 SSOT。

推荐：**A**（先把“行为正确性 + 数据结构”对齐，避免高风险大迁移）。✅ **已选：A**

### D2：Proxy 第一阶段形态（若做）
- **A（推荐）**：`cc-switch proxy serve` 仅提供代理服务；用户手动把各 CLI base_url 指向 `127.0.0.1:15721`；不做 takeover（自动改写 live）。
- **B**：直接做 takeover（自动改写 live + 备份/恢复）。风险与联动复杂度显著更高。

### D3：Codex auth 规则怎么对齐？
这里很容易“对齐上游”反而破坏 CLI 现有兼容性：
- **保持 CLI 行为（推荐）**：继续支持缺失 `auth.json`（credential store/env_key/requires_openai_auth 等路径），但在导入/upsert 时补充更明确的校验与提示。
- **强行同构 upstream**：把 auth 设为强制，会导致现有测试与用户配置失效，除非同时设计迁移策略。

---

## 4. 渐进式对齐路线图（Phase → 任务 → 验收）

> 原则：优先做“正确性/安全性”修复，其次做“结构收敛降低 diff 成本”，再做“新能力（Proxy 等系统能力）”。

### Phase 1（优先）：MCP/Provider 的正确性对齐（不引入 DB，不加大特性）

**目标**
- 修复会导致“导入为 0 / 覆盖用户配置 / 残留配置”的不一致行为，使其与 upstream 语义对齐或提供兼容开关。

**建议任务**
- [x] Gemini MCP 读写双向转换对齐：读取时补齐 `type`、反向映射 `httpUrl -> url`，并处理 `timeout` 兼容。（`df101b0`）
- [x] `McpService::upsert_server` 对齐语义：当 apps 从 true→false 时，执行 `remove_server_from_*` 清理对应 live。（`df101b0`）
- [x] Gemini provider live 写入改为 merge（保留现有 `settings.json` 的非 provider 字段，例如 `mcpServers`）。（`df101b0`）
- [ ] 引入 “should_sync” 行为开关（或默认策略）：当目标 app 未初始化（目录/文件不存在）时是否跳过写入/删除（upstream 更保守，CLI 当前更主动）。

**验收标准**
- 从 Gemini 导入远端 MCP：不再因 `type/httpUrl` 差异导入为 0。
- 更新 MCP 服务器并取消某 app 勾选：对应 app 的 live 配置中该 server 被移除。
- `cc-switch provider switch --app gemini ...` 不会覆盖掉现有 `~/.gemini/settings.json` 的 `mcpServers`（或至少提供可控策略）。

### Phase 2：Provider 数据结构与核心 API 面对齐（仍不引入 DB）

**目标**
- 对齐 upstream 的数据模型与关键“业务 API”语义，使后续移植 upstream 的高级能力成本更低。

**建议任务**
- [x] `src-tauri/src/provider.rs` 补齐 upstream 字段（`inFailoverQueue`、`UsageScript.templateType`、`ProviderMeta.*` 等）并保持 serde 向后兼容。（`ae8e93f`）
- [x] provider 列表顺序策略对齐：`ProviderManager.providers` 使用 `IndexMap`，并以 `sort_index -> created_at` 稳定排序。（`ae8e93f`）
- [x] `ProviderService::current()` 自愈：current 指向不存在时自动 fallback 到首个 provider。（`ae8e93f`）
- [x] usage script 对齐：当 `usage_script` 缺省 key/base_url 时回退从 provider settings 提取。（`ae8e93f`、`6a7e15d`）
- [x] Codex common config snippet：抽取/写回并在写入时 merge，避免污染 `mcp_servers.*`。（`ba36f2a`）
- [x] `services/provider` 结构收敛：拆分为 `provider/{mod,live,endpoints,usage,gemini_auth}.rs`。（`6a7e15d`、`07167e7`）

**验收标准**
- 既有配置文件可无痛加载（旧字段不丢，新字段默认值合理）。
- provider 列表/导出结果顺序稳定。
- usage 查询在 `usage_script` 缺省 key/base_url 时仍可工作（按 upstream 语义）。

### Phase 3：Skills 系统重做（建议按 upstream 思路，但“文件型 SSOT”落地）

**目标**
- 把 CLI 的 skills 从“Claude 单目录 + 占位命令”提升为 upstream 同级别能力：SSOT + 多 app 同步 + 扫描未管理技能。

**建议任务**
1) 建立 skills SSOT：`~/.cc-switch/skills/` 作为统一安装目录；新增轻量 index（`skills.json` 或扩展 `config.json`）。
2) 实现 `sync_to_apps`：优先 symlink，失败 fallback copy；Windows 特殊处理。
3) 实现 `scan_unmanaged`：扫描各 app 的 skills 目录，将不在 index 的标记为 unmanaged。
4) CLI 命令补齐：`cc-switch skills discover/install/uninstall/enable/disable/sync/scan-unmanaged/import-from-apps` 等。
5) 迁移：从旧 `SkillStore` 迁到新结构（建议“新增新字段 + 保留旧字段读取 + 一次性迁移”）。

**验收标准**
- `skills` 子命令可用且不破坏既有 Claude skills。
- 多 app 同步在 Windows/macOS/Linux 均有可用路径（symlink/copy）。

### Phase 4：Proxy/Failover/Stream Check/Usage（可选，且建议最后做）

**目标**
- 在 CLI 中以最小形态引入 upstream 的“代理与系统能力”，避免 Tauri/UI/DB 强耦合。

**建议任务**
1) Proxy MVP：新增 `cc-switch proxy serve`（前台运行），迁入裁剪版 `proxy/*`（去 DB、去 tray/event、去 takeover）。
2) Failover：仅在 proxy 运行态生效；必要时引入轻量持久化（`~/.cc-switch/proxy.json` 或 JSONL）。
3) Stream check：以 CLI 命令暴露（对齐 upstream `services/stream_check.rs` 的核心逻辑）。
4) Usage/Logs：先 JSONL，再评估 SQLite（仅做日志，不接管配置 SSOT）。

**验收标准**
- 代理在本地可工作（基本转发 + 熔断 + 简单 failover）。
- 不因 proxy 常驻引入配置并发写冲突（proxy 默认只读 SSOT）。

---

## 5. 命令对齐矩阵（建议优先级）

> 目标：让 CLI 的命令面能承载上游核心能力，但不引入 GUI 语义。

优先级建议（由高到低）：
1) `skills`：补齐为可用（当前缺口最大，且可独立实现）。
2) `mcp`：补 `show/import-from-apps/toggle` 等可见性与一致性（尤其 Gemini 导入）。
3) `provider`：补 usage/script/endpoint/sort 等（为 proxy/failover 做准备）。
4) `prompts`：补 `import-from-file` 与 `file show` 等与上游一致的文件操作。
5) `proxy`/`failover`/`stream-check`/`usage`：作为后期系统能力引入。

---

## 6. 测试与回归策略

建议以 `src-tauri/tests/*.rs` 的集成测试为主，严格隔离 `HOME`，避免污染真实用户环境。

建议新增/强化的回归点（按 Phase）：
- Phase 1：Gemini MCP 导入/写回、MCP upsert 取消勾选清理、Gemini provider 写入不覆盖 mcpServers。
- Phase 2：current provider 自愈、provider 列表顺序稳定、usage script 回退、Codex common snippet 抽取。
- Phase 3：skills sync/symlink fallback/unmanaged scan。
- Phase 4：proxy serve 基本转发 + failover + 熔断。

---

## 7. 风险清单（需提前规避）

1) **Codex auth 规则对齐**：强行同构 upstream 会破坏 CLI 现有兼容与测试；建议保留 CLI 行为并增强校验/提示。
2) **Gemini settings.json 覆盖**：必须优先修复为 merge 策略或提供开关，否则有真实用户数据丢失风险。
3) **“should_sync” 行为变化**：上游更保守；CLI 更主动。建议引入配置项并清晰文档化默认值。
4) **Proxy 引入后的并发一致性**：proxy 常驻与 CLI 命令并行写 SSOT 会出问题；proxy 默认应只读 SSOT（或引入文件锁/事务）。
5) **skills symlink 的跨平台差异**：Windows 下需要 copy fallback，且要避免误删 unmanaged 内容。

---

## 8. 下一步（把文档变成可执行 task）

当你确认 **D1/D2/D3** 的决策后，我们会把 Phase 1 拆成一组可执行的小 task（每个 task 都有：涉及文件、验收点、测试命令），然后按 task 逐个编码与回归。
