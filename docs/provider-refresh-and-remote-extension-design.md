# QuotaBarWin Provider 刷新机制优化方案（Phase 1）

> 状态：历史设计稿；部分能力已进入当前实现  
> 适用范围：方案 A（stale 降级）+ 方案 B（指数退避重试）  
> 远程 Provider / GitHub Raw 扩展相关内容已拆出到 `remote-provider-extension-design.md`

## 当前实现备注（2026-06）

当前代码已经实现了以下刷新保护：

- `ProviderSnapshot.status` 支持 `stale`。
- `build_app_snapshot_from_config_path` 和 `refresh_provider_from_config_path` 在 provider 失败时会从已有 `SNAPSHOT_CACHE` 中找同 id provider，保留旧 `windows`，将状态改为 `stale`，并附上新的 `error` / `diagnostics`。
- 对可重试错误做短间隔重试，间隔为 1s、2s。可重试识别覆盖 timeout、connection、connect，以及 Codex usage API 5xx。
- 首次失败且没有缓存时仍返回 `error` provider。

尚未实现或与本设计稿不同的部分：

- 没有独立的 `LAST_SUCCESSFUL_PROVIDER_SNAPSHOTS` per-provider 成功缓存。
- 没有持久或内存级 `ProviderRefreshState`、`consecutive_failures`、`next_retry_at` 长退避跳过逻辑。
- 当前重试是在单次刷新内完成，而不是调度层指数退避。

继续修改刷新机制时，以 `src-tauri/src/quota.rs` 的实现为准，并根据本备注判断设计稿中哪些内容仍是待办。

---

## 1. 背景与现状确认

当前 QuotaBarWin 的 provider 刷新策略比较“硬”：

- **失败一次即标记 `error`**：`command/script` provider 执行失败、超时、退出码非 0、JSON 解析失败，都会立即生成一个 `status = "error"` 的 `ProviderSnapshot`。
- **没有自动重试/退避**：没有 `consecutive_failures`、`exponential backoff`、`next_retry_at` 等概念。
- **降级能力有限**：全局有 `SNAPSHOT_CACHE`，整次刷新抛异常时前端会 fallback 到缓存；但单个 provider 失败后，该 provider 的数据直接变成 error，没有“保留上次成功数据并标 stale”的逻辑。

本方案聚焦把“一次性失败”改为“stale 降级 + 可控重试”。

---

## 2. 设计目标

1. **失败不丢数据**：单个 provider 失败后保留上一次成功数据，状态改为 `stale`，UI 继续展示旧配额。
2. **瞬时故障自愈**：对网络/超时等可恢复错误做短间隔重试 + 长间隔退避，避免“一次抖动就全红”。
3. **不阻塞其他 provider**：重试/退避机制不能卡住整个刷新循环。
4. **对现有协议影响最小**：不改动 `ProviderSnapshot` / `AppSnapshot` 核心字段，只新增 `stale` 状态。

---

## 3. 方案 A：后端缓存降级（stale 状态）

### 3.1 核心思路

单个 provider 刷新失败时，不清空它的旧数据，而是从独立的“上一次成功缓存”中取出旧 `ProviderSnapshot`，把 `status` 改成 `"stale"`，并写入简短错误说明。

### 3.2 缓存结构

**新增一个独立的 per-provider 成功缓存**，不和现有 `SNAPSHOT_CACHE` 混在一起：

```rust
static LAST_SUCCESSFUL_PROVIDER_SNAPSHOTS: OnceLock<Mutex<HashMap<String, ProviderSnapshot>>> = OnceLock::new();
```

- `SNAPSHOT_CACHE` 职责：缓存最后一次全局刷新结果，加速读。
- `LAST_SUCCESSFUL_PROVIDER_SNAPSHOTS` 职责：失败时兜底，保存每个 provider 最近一次成功数据。

更新规则：

- provider 刷新**成功**时：同时更新 `SNAPSHOT_CACHE` 和 `LAST_SUCCESSFUL_PROVIDER_SNAPSHOTS`。
- provider 刷新**失败**且存在成功缓存时：返回旧 snapshot + `status = "stale"` + `error` 字段。
- provider 刷新**失败且无成功缓存**时：返回现有 error snapshot（首次安装/从未成功过）。

### 3.3 `stale` 状态语义

- `status` 字段新增 `"stale"`，表示“数据是旧的，但可用”。
- `error` 字段可选，存放失败原因摘要（已脱敏）。
- `updated_at` 保留**上次成功刷新时间**。
- `windows` 保留旧数据，继续展示配额条。

### 3.4 UI 变化

- **ProviderCard**：
  - 继续展示 windows/配额条；
  - status badge 显示 `⚠ Stale · refresh failed`；
  - 显示上次成功时间：`last successful 2 min ago`；
  - 点击可展开 diagnostics。
- **全局状态条**：
  - 单个 provider stale 不触发顶部红条；
  - 只有当所有 provider 都失败且没有任何缓存时，才显示全局 error。

---

## 4. 方案 B：指数退避重试

### 4.1 核心思路

对“可能自恢复”的错误做重试：

- **单次执行内部**：做 1~2 次短间隔重试（1s、2s），覆盖瞬时网络抖动。
- **调度层**：通过 `next_retry_at` 跳过未冷却的 provider，实现长间隔退避，**不在刷新循环里 sleep**。

### 4.2 可重试 vs 不可重试错误

可重试：

- `Network`：DNS、TCP、连接被重置等。
- `Timeout`：请求/执行超时。
- `Http(5xx)`：服务端暂时错误。

不可重试：

- `Auth`：401/403，需要用户换 token。
- `Config`：配置缺失或非法。
- `Script`：脚本执行失败（除非明确判定为超时导致）。
- `Parse`：JSON 解析失败。

### 4.3 退避参数

| `consecutive_failures` | 下次可刷新间隔 |
|------------------------|----------------|
| 1                      | 15s            |
| 2                      | 30s            |
| 3                      | 60s            |
| ≥4                     | 120s（上限）    |

手动刷新始终立即执行，不受 `next_retry_at` 限制。

### 4.4 状态结构

```rust
#[derive(Clone, Debug)]
enum ErrorKind {
    Network,
    Timeout,
    Http(u16),
    Auth,
    Config,
    Script,
    Parse,
}

struct ProviderRefreshState {
    consecutive_failures: u32,
    last_success_at: Option<DateTime<Utc>>,
    next_retry_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    last_error_kind: Option<ErrorKind>,
}
```

- 存储位置：内存 static，**不持久化**。
- 启动时重置，重新积累失败次数是合理且安全的。

### 4.5 刷新调度逻辑

| 场景 | 行为 |
|---|---|
| 首次/手动刷新 | 立即执行，不受状态限制 |
| 定时刷新 | 若 `next_retry_at > now`，跳过本次，返回 stale 数据 |
| 执行成功 | 重置 `consecutive_failures=0`，更新 `last_success_at`，清空 `next_retry_at` |
| 执行失败 | `consecutive_failures += 1`，按指数退避设置 `next_retry_at`，返回 stale 数据 |
| 无历史成功数据且失败 | 返回 error snapshot |
| 所有 provider 都失败且无缓存 | 显示全局 error |

### 4.6 为什么不阻塞整个刷新循环

当前刷新是串行的。如果某个 provider 在单次执行里 sleep 120s，其他 provider 会被卡住。因此：

- **单次执行内只睡 1~2s**，用于瞬时重试。
- **长间隔退避通过“跳过”实现**，而不是 sleep。

这样其他 provider 的定时刷新不受影响。

---

## 5. 需要改动的代码范围

### 5.1 Rust 后端

- `src-tauri/src/quota.rs`
  - 新增 `LAST_SUCCESSFUL_PROVIDER_SNAPSHOTS` static。
  - 修改 `run_provider_config` / `refresh_provider`，失败时 fallback 到成功缓存。
  - 新增 `ProviderRefreshState` 管理与重试调度。
- `src-tauri/src/types.rs`（或 `quota.rs` 内）
  - 确认 `ProviderSnapshot.status` 允许 `"stale"`。
- `src-tauri/src/command_provider.rs`
  - 错误分类：把原始错误映射为 `ErrorKind`。

### 5.2 前端

- `src/App.tsx`
  - 无需大改，stale snapshot 仍通过现有 IPC 返回。
- `src/components/ProviderCard.tsx`（或类似组件）
  - 识别 `status === "stale"`；
  - 展示 stale badge、上次成功时间、retry 倒计时；
  - 继续展示 windows。
- `src/components/GlobalStatusStrip.tsx`
  - 调整全局错误触发条件：忽略单个 stale。

---

## 6. 测试策略（TDD）

### 6.1 Rust 单元测试

1. **成功缓存测试**：provider 失败后，返回上一次成功 snapshot 且 status 为 stale。
2. **首次失败测试**：provider 第一次就失败且无缓存时，返回 error snapshot。
3. **错误分类测试**：超时映射为 `Timeout`，401 映射为 `Auth`，5xx 映射为 `Http(503)`。
4. **退避测试**：连续失败 1/2/3/4 次后，`next_retry_at` 分别为 15s/30s/60s/120s 后。
5. **跳过逻辑测试**：`next_retry_at > now` 时，定时刷新跳过该 provider。
6. **手动刷新不受限**：即使 `next_retry_at` 未到，手动点击 Refresh 仍立即执行。

### 6.2 前端测试

1. ProviderCard 在 `status="stale"` 时仍渲染 windows。
2. ProviderCard 显示 `last successful X min ago`。
3. 全局状态条在单个 stale 时不出现。

---

## 7. 实施步骤

1. **先写失败缓存测试**：验证失败后返回 stale 旧数据。
2. **实现独立成功缓存 + stale fallback**。
3. **前端识别 stale 状态**。
4. **写重试/退避测试**。
5. **实现 ErrorKind 分类 + ProviderRefreshState + 调度跳过逻辑**。
6. **集成测试 + 回归测试**。

---

## 8. Review 检查清单

- [ ] 独立成功缓存结构是否清晰？
- [ ] `"stale"` 状态是否被前端正确识别？
- [ ] `ErrorKind` 分类是否完整？
- [ ] 退避间隔参数是否合理？
- [ ] 长间隔退避是否通过“跳过”实现，而非 sleep 阻塞？
- [ ] `ProviderRefreshState` 是否不持久化、重启重置？
- [ ] 全局 error 条触发条件是否调整正确？

---

## 9. 后续说明

远程 Provider / GitHub Raw 自刷新扩展 / 代理支持方案已拆出到独立文档：

```
D:\LocalAgentWorkspace\QuotaBarWin\docs\remote-provider-extension-design.md
```

本 Phase 1 方案仅覆盖刷新机制优化（方案 A + B），不涉及远程扩展内容。
