# V4 Forecast & Alerts Spec

> 目标：从“显示额度”升级为“帮助用户判断该怎么用额度”。V4 不新增 provider，只消费已有 ProviderSnapshot。

---

## 1. 版本定位

V4 只做使用节奏、瓶颈窗口、提醒策略和建议。

```text
AppSnapshot
  -> Forecast Engine
  -> Bottleneck / Pace / Alert
  -> UI Render
```

V4 禁止新增真实 provider，也禁止修改 V1-V3 provider 协议语义。

---

## 2. 必须实现

### 2.1 瓶颈窗口识别

每个 provider 识别最低 remainingPercent 的 window：

```text
Codex
- 5h remaining 99%
- weekly remaining 1%
=> bottleneck = weekly
```

全局识别所有 provider 中最紧张的 window。

### 2.2 Alert level

```ts
export type AlertLevel = "none" | "low" | "critical" | "urgent";
```

规则：

```text
remaining >= 20 -> none
10 <= remaining < 20 -> low
5 <= remaining < 10 -> critical
remaining < 5 -> urgent
```

### 2.3 Reset 倒计时

- 如果有 `resetAt`，显示 `resets in 2h 14m`。
- 如果只有 `resetText`，直接显示原文。
- 如果都没有，显示 `reset unknown`。

### 2.4 使用建议

基于 remainingPercent 和 reset 信息生成简单建议：

```text
- ok：正常使用
- low：建议保留给高价值任务
- critical：避免长任务或大重构
- urgent：只用于必要任务，优先切换其他 provider
```

### 2.5 通知策略增强

- 每个 provider/window/level/reset cycle 只提醒一次。
- 如果 resetAt 变化，允许重新提醒。
- 如果从 urgent 回升到 none，清除 dedup 状态。
- 通知内容不包含原始 command 或 token。

### 2.6 UI 增强

每个 provider card 增加：

- bottleneck badge
- alert badge
- reset countdown
- suggestion text

顶部 summary 增加：

```text
Lowest quota: BigModel Coding Plan · 5-Hour Token Limit · 5% left
```

---

## 3. 自动验收标准

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 3.1 Forecast tests

至少包含：

```text
finds_lowest_window_per_provider
finds_global_lowest_window
maps_remaining_percent_to_alert_level
formats_reset_at_countdown
uses_reset_text_when_reset_at_missing
generates_suggestion_for_low_critical_urgent
notification_dedup_keys_include_provider_window_level_reset
```

### 3.2 Frontend tests

至少包含：

```text
provider_card_renders_bottleneck_badge
summary_renders_global_lowest_quota
provider_card_renders_suggestion_text
```

---

## 4. 完成定义

V4 完成必须满足：

- 不新增 provider。
- 不修改 provider 协议语义。
- 能识别瓶颈窗口。
- 能生成 alert level 和 suggestion。
- 通知去重策略有测试。
- UI 渲染 forecast 信息。

---

## 5. 给 Codex 的短 Prompt

```text
目标版本：V4。只基于已有 AppSnapshot 实现 Forecast & Alerts：瓶颈窗口、alert level、reset 倒计时、使用建议、通知去重增强和 UI badge。不要新增 provider，不要改 ProviderSnapshot 语义。
```
