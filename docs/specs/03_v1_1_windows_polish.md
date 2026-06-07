# V1.1 Windows Polish Spec

> 目标：不改变 provider 协议，只打磨 WindowsFirst 常驻小工具体验。

---

## 1. 版本定位

V1.1 只做 Windows 使用体验，不新增真实 provider，不改 ProviderSnapshot 语义。

---

## 2. 必须实现

### 2.1 托盘体验

- 启动时默认隐藏主窗口，只显示托盘。
- 托盘 tooltip 显示最低剩余额度，例如：`QuotaBarWin · Lowest 12% · Last 10:31`。
- 托盘菜单显示最近刷新状态。
- 托盘菜单 Refresh 能触发刷新。
- Quit 才真正退出。

### 2.2 窗口行为

- 关闭窗口时隐藏到托盘。
- 再次 Show 时恢复上次窗口位置。
- 保存窗口位置、大小、最近打开的 view。
- 多屏场景下如果上次位置不可见，应回退到主屏中央。

### 2.3 通知

低额度通知规则：

- remaining < 20: warning
- remaining < 10: critical
- remaining < 5: urgent

要求：

- 同一 provider/window/threshold 不得反复刷屏。
- reset 后允许重新提醒。
- 通知文本不得包含 token、cookie、原始命令。

### 2.4 缓存

- 最近一次成功 `AppSnapshot` 持久化到 app data dir。
- 启动时优先显示缓存，再后台刷新。
- 刷新失败时 UI 显示 stale 状态，不清空旧数据。

### 2.5 诊断入口

设置页增加 Diagnostics 区域：

- 最近刷新时间。
- 最近错误。
- 每个 provider 最近一次 command exitCode、duration、timedOut。
- Copy diagnostics 按钮，复制脱敏后的 JSON。

---

## 3. 不做什么

V1.1 禁止：

- 新增 Kimi / BigModel 专用 parser。
- 新增 provider preset。
- 改动 AppSnapshot 字段语义。
- 引入数据库。
- 引入大型 UI 库。

---

## 4. 自动验收标准

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 4.1 Rust tests

至少包含：

```text
cache_roundtrip_works
stale_cache_used_when_refresh_fails
window_position_offscreen_falls_back_to_center
notification_deduplicates_same_threshold
tray_tooltip_uses_lowest_remaining_percent
copy_diagnostics_redacts_secrets
```

### 4.2 Frontend tests

至少包含：

```text
diagnostics_view_renders_provider_status
stale_snapshot_badge_is_visible
settings_can_toggle_start_hidden
```

### 4.3 Smoke 验收

需要提供可观测 smoke 命令：

```bash
npm run smoke
```

至少输出：

```json
{
  "trayMenuBuilt": true,
  "cacheRoundtrip": true,
  "diagnosticsRedacted": true,
  "notificationDedup": true
}
```

---

## 5. 完成定义

V1.1 完成必须满足：

- Windows 常驻体验相关代码可编译。
- 缓存、通知去重、tooltip、diagnostics 有自动测试。
- provider 协议不被破坏。
- V1 的 command provider 测试仍然通过。

---

## 6. 给 Codex 的短 Prompt

```text
目标版本：V1.1。只做 Windows 常驻体验打磨：启动隐藏、托盘 tooltip、通知去重、快照缓存、窗口位置记忆、diagnostics copy。不要改 provider 协议，不要新增真实 provider。
```
