# Phase 1（Claude/Codex/Gemini）：MCP/Provider 正确性对齐 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不引入 DB、保持 `config.json` 为 SSOT 的前提下，优先修复/对齐 upstream 的关键“正确性差异”（Gemini MCP 导入、MCP upsert 清理、Gemini provider 写入合并），降低真实用户配置被覆盖或导入失败风险。

**Architecture:** 以集成测试驱动（`src-tauri/tests/*.rs`），先锁定“现状会失败”的行为，再做最小改动：1) Gemini MCP 读取做反向转换；2) `McpService::upsert_server` 对 apps 取消勾选执行 live 清理；3) Gemini provider 的 `config` 写入改为 merge 现有 `settings.json`，保留 `mcpServers` 等字段。

**Tech Stack:** Rust（`serde_json` / `toml_edit` / `tempfile`）、Cargo integration tests（隔离 HOME），现有服务层（`src-tauri/src/services/*`）。

---

## Progress（截至 2026-01-30）

- [x] Task 1：Gemini MCP 导入失败用例（反向转换）— `df101b0`
- [x] Task 2：实现 Gemini MCP 反向格式转换 + timeout 兼容 — `df101b0`
- [x] Task 3：MCP upsert 取消勾选清理 live 的失败用例 — `df101b0`
- [x] Task 4：实现 upsert 的 “disable app => remove from live” 语义 — `df101b0`
- [x] Task 5：Gemini provider 切换保留 `mcpServers` 的失败用例 — `df101b0`
- [x] Task 6：Gemini provider 写入策略改为 merge（保留现有 `settings.json` 其他字段）— `df101b0`
- [x] Task 7：Phase 1 全量回归（`cd src-tauri && cargo test`）— 已多次验证通过（最近一次：2026-01-30）

## Task 1: 为 Gemini MCP 导入添加失败用例（httpUrl/url/type 反向转换）

**Files:**
- Modify: `src-tauri/tests/mcp_commands.rs`

**Step 1: Write the failing test**

在 `src-tauri/tests/mcp_commands.rs` 新增测试：
- `import_mcp_from_gemini_imports_http_and_sse_servers()`
- 在隔离 HOME 下写入 `~/.gemini/settings.json`：
  - `mcpServers.remote_http.httpUrl = "http://localhost:1234"`
  - `mcpServers.remote_sse.url = "http://localhost:5678"`
  - `mcpServers.local_stdio.command = "echo"`
- 调用 `McpService::import_from_gemini(&state)`
- 断言导入后统一结构里：
  - `servers["remote_http"].server.type == "http"` 且 `url` 存在（不是 `httpUrl`）
  - `servers["remote_sse"].server.type == "sse"` 且 `url` 存在
  - `servers["local_stdio"].server.type == "stdio"` 且 `command` 存在

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test import_mcp_from_gemini_imports_http_and_sse_servers -q`

Expected: FAIL（现状会把缺省 type 当 stdio，导致 httpUrl/url 的远端项导入失败或导入为 0）。

**Step 3: Commit（可选）**

Run:
```bash
git add src-tauri/tests/mcp_commands.rs
git commit -m "test: cover gemini mcp import reverse conversion"
```

---

## Task 2: 实现 Gemini MCP 反向格式转换（对齐 upstream `gemini_mcp.rs`）

**Files:**
- Modify: `src-tauri/src/gemini_mcp.rs`
- Modify: `src-tauri/src/mcp.rs`

**Step 1: Implement minimal code**

1) 在 `src-tauri/src/gemini_mcp.rs::read_mcp_servers_map` 增加反向转换（upstream 同款）：
- `httpUrl -> url + type:"http"`
- 无 `type` 时：
  - 有 `command` => `type:"stdio"`
  - 有 `url` => `type:"sse"`

2) 在 `src-tauri/src/mcp.rs::import_from_gemini` 改为使用 `crate::gemini_mcp::read_mcp_servers_map()` 的结果导入（避免再次手工 parse raw JSON）。

3)（建议一起做）在 `src-tauri/src/gemini_mcp.rs::set_mcp_servers_map` 补齐 upstream 的 timeout 转换：
- 将 `startup_timeout_sec/ms`、`tool_timeout_sec/ms` 折算成 Gemini 的 `timeout`（ms）并写入。

**Step 2: Run test to verify it passes**

Run: `cd src-tauri && cargo test import_mcp_from_gemini_imports_http_and_sse_servers -q`

Expected: PASS

**Step 3: Commit（可选）**

```bash
git add src-tauri/src/gemini_mcp.rs src-tauri/src/mcp.rs
git commit -m "fix: import gemini mcp with reverse conversion"
```

---

## Task 3: 为 MCP upsert 添加“取消勾选 app => 从 live 移除”的失败用例

**Files:**
- Modify: `src-tauri/tests/mcp_commands.rs`

**Step 1: Write the failing test**

新增测试：
- `upsert_server_disables_app_removes_from_gemini_live()`
- 准备 `~/.gemini/settings.json` 含 `mcpServers.remove_me = { "httpUrl": "http://localhost:1234" }`
- 准备 `state.config` 中存在同 ID server，且 `apps.gemini=true`
- 调用 `McpService::upsert_server(&state, McpServer{ id:"remove_me", apps.gemini=false, ... })`
- 断言写回后的 `~/.gemini/settings.json` 里 `mcpServers` 不再包含 `remove_me`

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test upsert_server_disables_app_removes_from_gemini_live -q`

Expected: FAIL（现状 `upsert_server` 只同步启用 apps，不会清理取消项）。

**Step 3: Commit（可选）**

```bash
git add src-tauri/tests/mcp_commands.rs
git commit -m "test: cover mcp upsert disables app removal"
```

---

## Task 4: 实现 `McpService::upsert_server` 的“取消勾选清理 live”语义

**Files:**
- Modify: `src-tauri/src/services/mcp.rs`

**Step 1: Implement minimal code**

在 `src-tauri/src/services/mcp.rs::McpService::upsert_server`：
- 在写锁内读取旧值（若存在）并缓存旧 `apps`
- 落盘后：
  - 对每个 app：若旧 apps 启用但新 apps 禁用，则调用 `remove_server_from_app(...)`
  - 最后再同步新启用 apps（保持现有逻辑）

**Step 2: Run test to verify it passes**

Run: `cd src-tauri && cargo test upsert_server_disables_app_removes_from_gemini_live -q`

Expected: PASS

**Step 3: Commit（可选）**

```bash
git add src-tauri/src/services/mcp.rs
git commit -m "fix: mcp upsert removes server from disabled apps"
```

---

## Task 5: 为 Gemini provider 切换添加“保留 mcpServers”的失败用例

**Files:**
- Modify: `src-tauri/tests/provider_service.rs`

**Step 1: Write the failing test**

新增测试：
- `switch_gemini_merges_existing_settings_preserving_mcp_servers()`
- 预先写入 `~/.gemini/settings.json`：
  - 顶层含 `mcpServers.keep = { "command":"echo" }`
- 构造两个 Gemini providers：
  - old：仅 env/token
  - new：env/token + `config` 仅包含 `security`（不包含 `mcpServers`）
- 令 current=old，执行 `ProviderService::switch(&state, AppType::Gemini, "new")`
- 断言切换后 `~/.gemini/settings.json` 仍包含 `mcpServers.keep`

**Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test switch_gemini_merges_existing_settings_preserving_mcp_servers -q`

Expected: FAIL（现状当 provider.config 为非空对象时，会整文件覆盖 settings.json，导致 mcpServers 丢失）。

**Step 3: Commit（可选）**

```bash
git add src-tauri/tests/provider_service.rs
git commit -m "test: gemini switch preserves existing mcpServers"
```

---

## Task 6: 对齐 Gemini provider 写入策略（config 对象 merge 到现有 settings.json）

**Files:**
- Modify: `src-tauri/src/services/provider.rs`

**Step 1: Implement minimal code**

在 `src-tauri/src/services/provider.rs::ProviderService::write_gemini_live`：
- 若 `settings_config.config` 是 object：
  - 读取现有 `~/.gemini/settings.json`（不存在则 `{}`）
  - 将 provider 的 `config` 顶层 key 合并覆盖到现有对象（保留 `mcpServers` 等未提及字段）
- 若 `config` 为 null 或缺失：
  - 保留现有文件（保持现状）
- 保持 `ensure_*_security_flag` 的行为不变（其自身会 preserve 其他字段）

**Step 2: Run test to verify it passes**

Run: `cd src-tauri && cargo test switch_gemini_merges_existing_settings_preserving_mcp_servers -q`

Expected: PASS

**Step 3: Commit（可选）**

```bash
git add src-tauri/src/services/provider.rs
git commit -m "fix: merge gemini provider config into existing settings.json"
```

---

## Task 7: Phase 1 回归

**Files:**
- N/A

**Step 1: Run full test suite**

Run: `cd src-tauri && cargo test`

Expected: PASS
