---
title: "QuotaBarWin UI Refactor Spec"
version: "1.0"
target_app: "QuotaBarWin"
target_stack: "Tauri / Rust / egui or current desktop UI stack"
audience: "Codex / Claude Code / implementation agents"
language: "zh-CN"
agent_friendly: true
priority: "P0"
---

# QuotaBarWin UI 改版开发指导文档

> Agent 执行目标：在不重写业务逻辑的前提下，将当前 QuotaBarWin 从“工程后台式配置面板”改造成“状态优先、可扩展 quota window、可验证”的桌面 quota monitor。

---

## 0. 给 Agent 的执行摘要

你需要做的不是简单改颜色，而是完成一次 UI 信息架构重排。

核心结论：

1. **Overview 以 ProviderCard + QuotaWindowRow 为核心。**
2. **不要硬编码 `5h` / `Weekly limit` 两个额度窗口。**
3. **进度条必须保留明显可读性。**
4. **顶部摘要只做轻量状态提示，正常状态不要替用户判断“最关心哪个 quota”。**
5. **Settings 重排为 `General / Providers / Advanced`。**
6. **配置路径、Portable mode、Reset config、Custom Provider guide 进入 Advanced，默认折叠。**
7. **Provider 的 Up / Down / Remove 等低频操作降噪。**
8. **Settings 底部 Save Bar 固定，不随滚动消失。**
9. **必须补充单元测试和 E2E/视觉回归测试。**

---

## 1. 当前问题

当前 UI 可用，但更像 debug panel / 后台表单，主要问题如下：

- 页面大标题重复窗口标题，占据过多主视觉。
- Overview 卡片里按钮、标题、状态、进度条权重接近，扫读效率不高。
- `Refresh provider` 按钮过于显眼，干扰 quota 状态阅读。
- Settings 中常用设置、Provider 管理、配置文件路径、Portable mode、危险操作混在一起。
- 配置路径占据过多页面空间。
- UI 隐含假设每个 Provider 只有 `5h` 和 `Weekly limit` 两个 quota window，不利于自定义 Provider 扩展。
- 右侧滚动条与页面布局显得粗糙。
- Save 按钮在滚动内容底部，用户滚动时容易失去保存入口。

---

## 2. 设计原则

按以下优先级执行：

```text
状态优先 > 操作其次 > 配置最后
Provider 可扩展 > 固定两个 quota window
明显进度条 > 装饰性精致
异常提示高亮 > 正常状态安静
低频操作收纳 > 所有按钮平铺
```

---

## 3. 非目标

本次不要做：

- 不要重写 Provider 的底层获取额度逻辑。
- 不要改变现有配置文件格式到无法兼容旧配置。
- 不要为了美观弱化 quota 进度条。
- 不要把 Overview 固定成两个横向 metric card。
- 不要默认展示完整配置路径。
- 不要让 Remove / Reset config 这类危险操作直接高亮暴露在主路径上。
- 不要把 UI 做成仅适配当前两个 Provider 的特例。

---

## 4. 目标信息架构

### 4.1 主页面

保留两个主页面：

```text
Overview
Settings
```

### 4.2 Overview 结构

```text
Compact Header
Global Status Strip
Provider Card List
```

### 4.3 Settings 结构

```text
Settings
- General
- Providers
- Advanced
- Fixed Save Bar
```

---

## 5. 数据模型要求

### 5.1 Provider 必须支持 quotaWindows 数组

如果当前结构类似：

```ts
{
  name: string;
  fiveHourRemaining: number;
  weeklyRemaining: number;
}
```

需要在 UI 适配层标准化为：

```ts
type ProviderStatus = {
  id: string;
  name: string;
  enabled: boolean;
  kind?: string; // command / api / builtin / custom
  status: "ok" | "warning" | "error" | "unknown";
  lastRefreshAt?: string; // ISO string
  lastSuccessAt?: string; // ISO string
  errorMessage?: string;
  quotaWindows: QuotaWindow[];
};

type QuotaWindow = {
  id: string;
  label: string; // e.g. "5h", "Weekly limit", "Daily", "Monthly"
  remainingPercent?: number; // 0 - 100
  usedPercent?: number; // 0 - 100
  remainingText?: string; // e.g. "93% remaining", "12 requests left"
  usedText?: string; // e.g. "7% used"
  resetAt?: string; // ISO string
  resetText?: string; // fallback if resetAt cannot be parsed
  status?: "ok" | "warning" | "error" | "unknown";
  showOnOverview?: boolean;
  sortOrder?: number;
  description?: string;
};
```

### 5.2 强制要求

- Overview 渲染时必须遍历 `quotaWindows`。
- 禁止在 UI 中硬编码只显示 `5h` 与 `Weekly limit`。
- Provider 没有 quota window 时必须有 empty state。
- Provider 有 1、2、3、4、7 个 quota window 时都必须稳定展示。
- 自定义 Provider 未来返回更多 quota window 时，不应推翻当前 UI。

### 5.3 适配层

如果短期不想改底层数据结构，至少增加 UI adapter：

```ts
function normalizeProviderStatus(raw: unknown): ProviderStatus
```

职责：

- 兼容旧结构。
- 兼容新结构。
- 处理缺失字段。
- 保证返回值里 `quotaWindows` 一定是数组。
- 不允许因为单个 Provider 数据异常导致整个 Overview 崩溃。

---

## 6. Overview 页面规格

### 6.1 Compact Header

当前内容区大标题 `QuotaBarWin V0.0.0` 需要弱化。

目标：

```text
QuotaBarWin                                      [Refresh]

[Overview] [Settings]
```

要求：

- App 名称保留，但字号降低。
- 版本号不要作为主视觉展示。
- 版本号可放在窗口标题栏、About、Footer 或 Tooltip。
- `Refresh` 是全局刷新按钮，右侧展示。
- 顶部区域不要占据过多纵向空间。

建议尺寸：

```text
page padding: 24px - 32px
app title font-size: 20px - 24px
nav button height: 40px - 44px
header bottom margin: 20px - 24px
```

---

### 6.2 Global Status Strip

新增轻量全局状态条。

正常状态：

```text
2 providers active · Last updated 09:00 · Auto refresh every 300s
```

Warning 状态：

```text
1 provider needs attention · Kimi Coding weekly below 20%
```

Error 状态：

```text
Zhipu Coding refresh failed · last successful update 38m ago
```

要求：

- 不要默认强行展示 lowest quota。
- 不要假设用户一定最关心最低 quota。
- 正常状态下使用低对比度文本。
- warning / error 时使用浅色提示底 + 明确文案。
- 一行优先，超出时省略或 tooltip。
- 无 Provider 时展示：

```text
No providers configured. Add a provider in Settings.
```

---

### 6.3 ProviderCard

每个 Provider 一个卡片。

结构：

```text
┌────────────────────────────────────────────────────────────┐
│ Zhipu Coding                         status ok       ⋯     │
│ Last updated 09:00                                          │
│                                                            │
│ 5h window                            100% remaining        │
│ resets 11:35                         ████████████████████  │
│                                                            │
│ Weekly limit                         93% remaining         │
│ resets Fri 10:01                     ██████████████████░░  │
└────────────────────────────────────────────────────────────┘
```

要求：

- Provider 名称是卡片主标题。
- status badge 放右侧。
- `Refresh provider` 不要作为大按钮常驻展示。
- 单 Provider 刷新、编辑、禁用等操作放入右侧 `⋯` 菜单。
- 如果当前框架暂时没有菜单组件，可临时保留小型 ghost button，但不能用高权重大按钮。
- Last updated 是次级信息，不应比 quota 更显眼。
- 错误状态下展示错误摘要，完整错误放 tooltip / 展开区域。

---

### 6.4 QuotaWindowRow

这是本次改版核心组件。

统一布局：

```text
label                              value text
reset text                         progress bar
```

示例：

```text
5h window                          100% remaining
resets 11:35                       ████████████████████
```

要求：

- QuotaWindowRow 必须可重复渲染。
- 不允许写死两个 quota。
- 进度条必须明显、清晰。
- 进度条高度建议 `10px - 14px`。
- 进度条宽度占满可用空间。
- label、value、reset text、progress bar 的位置必须一致。
- `remainingPercent` 缺失时，不显示错误进度条，改为文本状态。
- `resetAt` 缺失时，不显示 reset 行，或展示 `No reset time`。
- warning / error quota row 要有明确视觉差异。
- `displayMode = Remaining` 时展示 remaining 语义。
- `displayMode = Used` 时展示 used 语义。

---

### 6.5 多 quota window 规则

默认规则：

- 如果 `quotaWindows.length <= 4`，全部显示。
- 如果 `quotaWindows.length > 4`：
  - 优先显示 `showOnOverview === true` 的 window。
  - 不足 4 个时，按 `sortOrder` 和状态优先级补足。
  - 默认最多显示 4 个。
  - 其余显示：

```text
+ 3 more quota windows
```

交互要求：

- 点击后在当前卡片展开全部。
- 展开后文案变为：

```text
Show less
```

- 展开/收起不触发数据刷新。
- 展开状态可不持久化。

---

### 6.6 Provider 状态计算

优先级：

```text
error > warning > ok > unknown
```

规则：

- Provider 刷新失败：`status = error`。
- 任一 quota window `status = error`：`status = error`。
- 任一 quota window `remainingPercent <= lowQuotaWarning`：`status = warning`。
- 任一 quota window `status = warning`：`status = warning`。
- 至少有一个 quota window 正常：`status = ok`。
- 没有数据且无错误：`status = unknown`。

注意：

- 不要只根据 weekly quota 判断。
- 不要只根据第一个 quota window 判断。

---

## 7. Settings 页面规格

Settings 页面重构为：

```text
General
Providers
Advanced
Fixed Save Bar
```

---

### 7.1 General Section

目标：

```text
General

Refresh interval       [ 300 ] seconds
Display mode           [ Remaining v ]
Low quota warning      [ 20 ] %
Log level              [ Info v ]
[✓] Launch at startup
```

要求：

- 保留当前已有字段。
- 双列表单布局可以保留，但 spacing 收紧。
- label 与 input 对齐。
- 单位明确，例如 `seconds`、`%`。
- 输入校验：
  - `Refresh interval > 0`
  - `Low quota warning` 在 `0 - 100`
- 错误输入时显示 inline validation，不要静默失败。

---

### 7.2 Providers Section

Provider 管理上移到 General 后、Advanced 前。

目标：

```text
Providers                                      [+ Add provider]

[ + Kimi Coding Usage ] [ + BigModel Coding Plan ] [ + OpenCode Quota Command ] [ + Custom Command Provider ]

┌────────────────────────────────────────────────────────────┐
│ ✓ Zhipu Coding              command provider       Edit ⋯  │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│ ✓ Kimi Coding               command provider       Edit ⋯  │
└────────────────────────────────────────────────────────────┘
```

要求：

- Provider 管理比配置路径更重要，应放在前面。
- Provider 列表显示：
  - enabled checkbox
  - provider name
  - provider type/kind
  - Edit
  - More menu
- `Up`、`Down`、`Remove` 不要全部平铺成高权重按钮。
- `Up`、`Down`、`Remove` 放入 `⋯` 菜单。
- `Remove` 使用危险样式，并需要二次确认。
- 如果暂时没有菜单组件，可以用低权重 inline buttons 过渡：
  - Edit 正常按钮
  - Up/Down ghost
  - Remove danger subtle
- Provider 为空时展示：

```text
No providers yet. Add one to start monitoring quota.
```

---

### 7.3 Add Provider

保留现有模板：

- Kimi Coding Usage
- BigModel Coding Plan
- OpenCode Quota Command
- Custom Command Provider

要求：

- 模板按钮横向换行。
- 不要比 Provider 列表本身更抢眼。
- 点击模板后进入对应 Provider 配置流程。
- 添加成功后 Provider 出现在列表中，并默认 enabled。

---

### 7.4 Advanced Section

以下内容进入 Advanced：

- Configuration storage
- Config file path
- Portable mode
- Custom Provider guide
- Reset config
- Debug / raw output，如有

默认折叠：

```text
Advanced
▸ Configuration storage
▸ Custom Provider guide
```

展开 Configuration storage：

```text
Current mode: AppData

Config file
C:\Users\...\QuotaBarWin\config.quotaBarWin.json

AppData
C:\Users\...\QuotaBarWin\config.quotaBarWin.json

Portable
D:\...\target\release\config.quotaBarWin.json

[Open folder]   [Reset config]
```

要求：

- 默认不要展示完整大段路径。
- 路径默认中间省略。
- 点击路径可以复制完整路径。
- hover tooltip 可以显示完整路径。
- Reset config 是 danger 操作。
- Reset config 必须二次确认。
- Portable mode 说明保留，但默认放 Advanced。
- Custom Provider guide 默认折叠。

---

### 7.5 Fixed Save Bar

Settings 页面底部固定保存栏。

无改动：

```text
No changes                                      [Save disabled]
```

有改动：

```text
Unsaved changes                                  Reset changes   Save
```

要求：

- Save Bar 固定在 Settings 页面底部。
- 内容滚动时 Save Bar 仍然可见。
- 没有改动时 Save disabled。
- 有改动时 Save enabled。
- 保存成功后短暂显示：

```text
Saved
```

- 保存失败时显示错误信息。
- Reset changes 恢复到进入 Settings 时的配置。
- 滚动内容底部需要 padding，避免被 Save Bar 遮挡。

---

## 8. 视觉规范

### 8.1 颜色

保留当前青绿色主题，但降低滥用。

语义：

```text
Primary: 深青绿色，用于主按钮、正常进度条
Primary subtle: 浅青绿色背景，用于 selected tab 或轻提示
Success: status ok
Warning: 低额度、即将耗尽
Danger: 删除、reset config、刷新失败
Neutral: 边框、说明文字、禁用状态
```

要求：

- 主按钮只用于主要操作，例如全局 Refresh、Save。
- 普通操作使用 outline / ghost。
- Danger 操作优先红色文字或浅红描边，不要大面积红底。
- `status ok` 不需要过度显眼。
- warning / error 才需要抢注意力。

### 8.2 字体层级

建议：

```text
App title: 20 - 24px / semibold
Section title: 18 - 20px / semibold
Provider name: 18 - 20px / semibold
Quota label: 14 - 15px / semibold
Quota value: 14 - 16px / medium
Secondary text: 13 - 14px / regular
Button text: 14 - 15px / medium
```

要求：

- 不要所有标题都 heavy bold。
- 真正醒目的应该是 quota 状态，而不是按钮和页面标题。
- Last updated、reset time、path 使用次级文本颜色。

### 8.3 间距与卡片

建议：

```text
page padding: 24px - 32px
card padding: 20px - 24px
card radius: 12px - 14px
card gap: 16px - 20px
quota row gap: 14px - 18px
progress height: 10px - 14px
```

要求：

- 卡片之间间距不要过大。
- 卡片内部信息对齐。
- 进度条明显，但不要粗笨。
- 页面整体避免“表单堆叠感”。

### 8.4 滚动条

要求：

- 顶部 header 不应跟随 Settings 内容滚走。
- Settings 内容区可以滚动。
- 底部 Save Bar 固定。
- 滚动条尽量细、浅、桌面化。
- 避免粗大的默认滚动条破坏质感。

---

## 9. 交互规范

### 9.1 Global Refresh

要求：

- 点击后刷新所有 enabled providers。
- 刷新中按钮显示 loading。
- 刷新中避免重复点击。
- 完成后更新 Global Status Strip 和 ProviderCard。
- 部分 Provider 失败时，不要清空其他 Provider 的成功数据。

### 9.2 Single Provider Refresh

要求：

- 放在 ProviderCard 的 `⋯` 菜单中。
- 只刷新当前 Provider。
- 失败时 Provider status = error。
- 保留 last success data，并展示失败信息。

### 9.3 Display Mode

Remaining 模式：

- value text 显示 `xx% remaining`。
- progress fill 表示 remaining percent。

Used 模式：

- value text 显示 `xx% used`。
- progress fill 表示 used percent。

要求：

- 不要混淆 used 和 remaining 的进度条含义。
- 模式切换后 Overview 所有 quota row 同步更新。
- 如果 `usedPercent` 缺失但 `remainingPercent` 存在，可使用 `100 - remainingPercent` 推导。

### 9.4 Low Quota Warning

当 `remainingPercent <= lowQuotaWarning`：

- 对应 QuotaWindowRow 标记 warning。
- Provider status 至少为 warning。
- Global Status Strip 显示 needs attention。
- 进度条或状态文本使用 warning 样式。
- 不要只对 weekly quota 生效。

### 9.5 Error Handling

Provider 刷新失败：

- Provider status = error。
- 卡片展示简短错误：

```text
Refresh failed · last successful update 38m ago
```

- 完整错误放 tooltip 或展开区。
- 不要因为一个 Provider 失败导致整个 Overview 空白。
- 如果所有 Provider 失败，Global Status Strip 显示 error。
- 如果无历史成功数据，QuotaWindowRow 区域展示 empty/error state。

---

## 10. 可访问性要求

- 所有按钮可键盘访问。
- Tab 顺序符合视觉顺序。
- `⋯` 菜单可通过 Enter / Space 打开。
- status badge 不只依赖颜色，应有文本：
  - ok
  - warning
  - error
  - unknown
- progress bar 需要可访问 label，例如：

```text
Zhipu Coding Weekly limit 93% remaining
```

- input 必须有 label。
- warning / danger 颜色对比度要足够。
- disabled 按钮要有明显 disabled 状态。

---

## 11. 响应式与窗口尺寸

目标桌面窗口参考当前约 `1140 x 970`。

要求：

- 宽度 `>= 900px`：使用主布局。
- 宽度 `700px - 900px`：
  - quota row 保持纵向。
  - header 按钮允许换行。
- 宽度 `< 700px`：
  - 顶部导航换行。
  - ProviderCard 仍然可读。
  - quota label/value 可上下排列。
- 不允许横向滚动。
- Settings 中 input 在窄宽下从双列变单列。

---

## 12. 建议组件拆分

按职责拆分，即使不是 React/Vue/Svelte，也应保持类似边界。

```text
AppShell
TopHeader
NavTabs
GlobalStatusStrip

OverviewPage
ProviderCard
ProviderStatusBadge
QuotaWindowRow
ProgressBar
ProviderActionMenu
EmptyProviderState

SettingsPage
SettingsSection
GeneralSettings
ProviderSettingsList
ProviderSettingsItem
AddProviderTemplates
AdvancedSettings
ConfigStoragePanel
FixedSaveBar
ConfirmDialog
```

### 12.1 ProviderCard

输入：

```ts
{
  provider: ProviderStatus;
  displayMode: "remaining" | "used";
  lowQuotaWarning: number;
}
```

职责：

- 渲染 Provider header。
- 渲染 quota windows。
- 处理展开更多 quota windows。
- 提供单 Provider 操作入口。

不得：

- 不得内部硬编码 Provider 名称。
- 不得硬编码 quota window 数量。
- 不得直接执行刷新逻辑，只触发事件。

### 12.2 QuotaWindowRow

输入：

```ts
{
  providerName: string;
  quotaWindow: QuotaWindow;
  displayMode: "remaining" | "used";
  lowQuotaWarning: number;
}
```

职责：

- 展示 label。
- 展示 remaining / used value。
- 展示 reset 信息。
- 展示 progress bar。
- 根据 warning / error 状态调整视觉。

### 12.3 GlobalStatusStrip

输入：

```ts
{
  providers: ProviderStatus[];
  refreshIntervalSeconds: number;
  lastGlobalRefreshAt?: string;
}
```

职责：

- 正常状态展示轻量汇总。
- warning / error 状态展示明确提示。
- 不替用户过度判断“最关心的 quota”。

---

## 13. 时间格式规范

避免主界面展示工程化时间：

```text
2026/6/9 09:00:05
2026/06/13 GMT+8 10:01
```

建议：

```text
Last updated 09:00
resets 11:35
resets Fri 10:01
resets in 2h 35m
```

规则：

- Overview 中优先短格式。
- tooltip 或详情中可以显示完整时间。
- 同一天 reset：`resets HH:mm`
- 非同一天但一周内：`resets Fri HH:mm`
- 更远：`resets YYYY/MM/DD HH:mm`
- `resetAt` 无法解析：回退到 `resetText`
- 主界面不要显示冗长 GMT 字符串，除非没有更好格式。

---

## 14. 配置兼容性

要求：

- 旧配置能正常读取。
- 旧的 5h / Weekly 字段在内存中转换为 `quotaWindows`。
- 保存时可以保存为新结构，或继续保持旧结构但 UI 层必须统一。
- 如果使用 UI adapter，确保所有 UI 组件只吃 `ProviderStatus`。
- 不允许因为旧配置导致 Overview 空白。

---

## 15. 测试要求

本次改版必须补充可验证测试。

---

### 15.1 单元测试

#### normalizeProviderStatus

用例：

- 旧结构 provider 只有 5h / weekly，能转换成 2 个 quotaWindows。
- 新结构 provider 有 3 个 quotaWindows，保持原样。
- provider 无 quotaWindows，返回空数组。
- quota window 缺少 percentage，不崩溃。
- provider refresh error 时 status = error。

#### status calculation

用例：

- 所有 quota > warning threshold，Provider status = ok。
- 任一 quota <= warning threshold，Provider status = warning。
- 任一 quota error，Provider status = error。
- refresh failed，Provider status = error。
- 无 quota 且无错误，Provider status = unknown。

#### display mode

用例：

- Remaining 模式显示 remaining percent。
- Used 模式显示 used percent。
- usedPercent 缺失时可由 remainingPercent 推导。
- remainingPercent 缺失时不显示错误进度条。

#### time formatting

用例：

- 同一天 reset 显示 `resets HH:mm`。
- 一周内 reset 显示 `resets Fri HH:mm`。
- 更远 reset 显示日期。
- invalid resetAt 回退 resetText。

---

### 15.2 E2E 测试

建议增加稳定 `data-testid`。

```text
top-header
global-status-strip
overview-page
provider-card-{providerId}
provider-status-{providerId}
quota-row-{providerId}-{quotaWindowId}
quota-progress-{providerId}-{quotaWindowId}
provider-action-menu-{providerId}

settings-page
general-settings-section
providers-settings-section
advanced-settings-section
fixed-save-bar
save-settings-button
reset-changes-button
```

#### Overview Case 1：正常展示两个 provider

Mock：

```text
Zhipu Coding: 5h 100%, weekly 93%
Kimi Coding: 5h 100%, weekly 80%
```

验证：

- 页面存在两个 ProviderCard。
- 每个 ProviderCard 内显示对应 quota row。
- 进度条存在。
- Global Status Strip 显示 active providers 和 last updated。
- 没有 warning/error 样式。

#### Overview Case 2：Provider 有 4 个 quota window

Mock：

```text
Custom Provider:
- 1h 90%
- 5h 80%
- daily 70%
- weekly 60%
```

验证：

- 4 个 quota row 都显示。
- 没有 `more quota windows`。
- 没有布局错乱。
- 不出现横向滚动。

#### Overview Case 3：Provider 有 7 个 quota window

Mock：

```text
Custom Provider:
- 1h
- 5h
- daily
- weekly
- monthly
- token
- command
```

验证：

- 默认最多显示 4 个 quota row。
- 显示 `+ 3 more quota windows`。
- 点击后显示全部 7 个。
- 点击 `Show less` 后恢复最多 4 个。

#### Overview Case 4：低额度 warning

Mock：

```text
Kimi Coding weekly remaining = 12
lowQuotaWarning = 20
```

验证：

- Kimi weekly quota row 有 warning 状态。
- Kimi Provider status = warning。
- Global Status Strip 显示 needs attention。
- 任意 quota window 低于阈值都触发 warning，不只 weekly。

#### Overview Case 5：刷新失败

Mock：

```text
Zhipu Coding refresh failed
lastSuccessAt exists
old quota data exists
```

验证：

- Provider status = error。
- 卡片显示 refresh failed。
- 旧 quota 数据仍然可见。
- 其他 provider 正常显示。

#### Overview Case 6：Display mode 切换

步骤：

- Settings 中 display mode 从 Remaining 切换为 Used。
- 保存。
- 回到 Overview。

验证：

- quota value 从 `xx% remaining` 改为 `xx% used`。
- progress bar 语义随 display mode 改变。
- 页面没有同时混用 remaining 和 used。

---

### 15.3 Settings 测试

#### Settings Case 1：分组存在

验证：

- General section 存在。
- Providers section 存在。
- Advanced section 存在。
- Configuration storage 不应出现在 Providers 之前。

#### Settings Case 2：Save Bar 固定

步骤：

- 打开 Settings。
- 滚动到底部。
- 修改一个设置。

验证：

- Save Bar 始终可见。
- Save 按钮从 disabled 变 enabled。
- 点击 Save 后显示 Saved。
- 保存后 Save 再次 disabled。

#### Settings Case 3：Advanced 默认折叠

验证：

- Config file 完整路径默认不大段展示。
- 点击 Configuration storage 后展开。
- 展开后显示 AppData / Portable 路径。
- 路径可以复制或完整查看。
- Reset config 在 Advanced 中。

#### Settings Case 4：Remove Provider 二次确认

步骤：

- 打开 Provider item 的 More menu。
- 点击 Remove。

验证：

- 出现确认弹窗。
- 取消后 Provider 仍存在。
- 确认后 Provider 被移除。
- Save Bar 进入 unsaved changes 状态。

#### Settings Case 5：表单校验

步骤：

- Refresh interval 输入 0 或负数。
- Low quota warning 输入 200。

验证：

- 显示 inline error。
- Save disabled 或保存时阻止。
- 错误文案明确。
- 修正后可以保存。

---

### 15.4 视觉回归测试

至少覆盖以下截图：

- Overview 正常状态。
- Overview warning 状态。
- Overview error 状态。
- Provider 有 7 个 quota window 且未展开。
- Provider 有 7 个 quota window 且已展开。
- Settings 顶部。
- Settings 滚动到底部但 Save Bar 固定。
- Advanced 展开状态。
- 小窗口宽度状态。

验收标准：

- 无横向滚动。
- Provider card 内元素对齐。
- 进度条清晰可见。
- Save Bar 没有遮挡内容。
- 滚动条不粗糙突兀。
- Warning / error 状态可读。

---

## 16. 实施计划

### Phase 1：数据适配层

任务：

- 新增 `QuotaWindow` 和 `ProviderStatus` 类型。
- 新增 `normalizeProviderStatus(raw)`。
- 新增 `calculateProviderStatus(provider, lowQuotaWarning)`。
- 新增 `formatResetTime(resetAt, resetText)`。
- 添加单元测试。

完成标准：

- 旧数据和新数据都能输出统一 `ProviderStatus`。
- Overview 暂时不改 UI 也能拿到统一数据。

### Phase 2：Overview 组件重构

任务：

- 新增 `GlobalStatusStrip`。
- 新增或重构 `ProviderCard`。
- 新增 `QuotaWindowRow`。
- 将现有 5h / Weekly 渲染替换为遍历 `quotaWindows`。
- 将 `Refresh provider` 大按钮移到 More menu 或降级为小按钮。
- 支持超过 4 个 quota window 展开/收起。
- 添加 Overview E2E 测试。

完成标准：

- 不管 Provider 有多少 quota window，Overview 都能稳定展示。
- 进度条清晰。
- warning / error 状态正确。

### Phase 3：Settings 信息架构重排

任务：

- 创建 General / Providers / Advanced 三个 section。
- 将 Provider 管理移动到 Providers section。
- 将 config path / portable mode / guide / reset config 移入 Advanced。
- Advanced 默认折叠。
- Provider item 的 Up / Down / Remove 降噪。
- 添加 Settings E2E 测试。

完成标准：

- Settings 首屏主要展示 General 和 Providers。
- 配置路径不再默认占据大量空间。
- 危险操作不再显眼暴露。

### Phase 4：Fixed Save Bar

任务：

- Settings 页面增加固定底部 Save Bar。
- 实现 dirty state。
- 实现 Save disabled / enabled。
- 实现 Reset changes。
- 确保滚动内容不会被 Save Bar 遮挡。
- 添加对应 E2E 测试。

完成标准：

- 修改设置后用户始终能看到保存入口。
- 页面滚动到底部时 Save Bar 仍固定。
- 保存状态反馈明确。

### Phase 5：视觉 polish

任务：

- 调整 header 高度。
- 调整卡片 padding、radius、gap。
- 调整按钮权重。
- 调整 status badge。
- 调整滚动条样式。
- 调整路径截断。
- 做视觉回归截图。

完成标准：

- 整体像桌面状态工具，而不是后台表单。
- 主题色保留，但不滥用。
- 进度条仍然醒目。

---

## 17. Definition of Done

### 数据模型

- [ ] Provider 支持 `quotaWindows` 数组。
- [ ] UI 不硬编码 `5h` / `Weekly`。
- [ ] 旧配置兼容。
- [ ] 自定义 Provider 可以展示多个 quota window。

### Overview

- [ ] 顶部标题更紧凑。
- [ ] 有 Global Status Strip。
- [ ] 正常状态提示安静。
- [ ] 异常状态提示明确。
- [ ] ProviderCard 可展示任意数量 quota window。
- [ ] 进度条明显可读。
- [ ] Provider 操作降噪。
- [ ] 多 quota window 可展开/收起。
- [ ] warning / error 状态正确。

### Settings

- [ ] 分为 General / Providers / Advanced。
- [ ] Provider 管理在配置路径之前。
- [ ] Advanced 默认折叠。
- [ ] 完整路径默认不大面积展示。
- [ ] Reset config 是 danger 操作并二次确认。
- [ ] Remove provider 二次确认。
- [ ] Save Bar 固定。
- [ ] Save dirty state 正确。
- [ ] 输入校验正确。

### 测试

- [ ] 单元测试覆盖 normalize/status/time/display mode。
- [ ] E2E 测试覆盖 Overview 正常、warning、error、多 quota。
- [ ] E2E 测试覆盖 Settings 分组、Save Bar、Advanced、Remove、校验。
- [ ] 至少有基础视觉回归截图。
- [ ] 小窗口下无横向滚动。

---

## 18. 推荐给 Codex / Claude Code 的执行提示词

可以把下面这段直接作为 Agent 的任务开头：

```text
请根据 `docs/quota-bar-ui-refactor-spec.md` 执行 QuotaBarWin UI 改版。

优先级：
1. 先实现数据适配层，确保 ProviderStatus 支持 quotaWindows 数组，且兼容旧结构。
2. 再重构 Overview，禁止硬编码 5h / Weekly，必须通过 QuotaWindowRow 遍历渲染。
3. 再重排 Settings 为 General / Providers / Advanced，并实现固定 Save Bar。
4. 最后做视觉 polish。
5. 每个 Phase 都要补充或更新测试。不要只改 UI 不补测试。

硬性要求：
- 不要削弱进度条可读性。
- 不要默认展示完整配置路径。
- 不要把危险操作放在主路径上。
- 不要因为某个 Provider 刷新失败导致整个 Overview 空白。
- 不要引入与当前项目技术栈冲突的大型依赖。
- 每完成一个 Phase，请运行现有测试，并补充对应测试用例。
```

---

## 19. 最终效果预期

改版后，用户打开 QuotaBarWin 时第一眼应该知道：

```text
当前整体是否正常
每个 Provider 是否正常
每个 quota window 还剩多少
什么时候恢复
```

用户进入 Settings 时第一眼应该知道：

```text
常用设置在哪里
Provider 在哪里管理
高级配置在哪里展开
改完在哪里保存
```

最终界面应保留当前“进度条直观”的优势，同时减少按钮、路径、大标题、低频操作带来的视觉噪音，让产品从“开发者后台表单”升级为“精致、可扩展、可维护的桌面 quota monitor”。
