# V0 Technical Spike Spec

> 目标：用最小代码跑通 WindowsFirst Tauri 托盘应用骨架，以及 mock provider 到 UI 的数据流。

---

## 1. 版本定位

V0 不是日常可用版本，只验证底座：

```text
Tauri App
  -> tray
  -> window
  -> Rust command
  -> mock provider
  -> AppSnapshot
  -> React render
```

---

## 2. 必须实现

### 2.1 项目初始化

技术栈：

- Tauri 2
- React
- TypeScript
- Vite
- Rust backend
- 普通 CSS

建议结构：

```text
src/
  App.tsx
  main.tsx
  styles.css
  types.ts
  lib/
    api.ts
    format.ts
  components/
    Header.tsx
    ProviderCard.tsx
    ProgressBar.tsx

src-tauri/
  src/
    main.rs
    quota.rs
    providers/
      mod.rs
      mock.rs
```

### 2.2 数据结构

必须在 TS 和 Rust 侧定义：

```text
QuotaWindow
ProviderSnapshot
AppSnapshot
```

字段以 `00_overview_and_coding_contract.md` 为准。

### 2.3 mock provider

mock provider 返回至少一个 provider，包含两个窗口：

```text
5h window
weekly window
```

示例数据：

```text
Codex Mock
- 5h remaining 72%
- weekly remaining 35%
```

### 2.4 Rust command

暴露：

```rust
refresh_snapshot() -> Result<AppSnapshot, String>
```

前端通过 Tauri invoke 获取数据。

### 2.5 UI

主窗口展示：

- title: QuotaBarWin
- refresh button
- provider card list
- progress bar

V0 不要求设置页，不要求配置文件。

### 2.6 托盘

实现：

- 系统托盘图标。
- 托盘菜单：Show / Refresh / Quit。
- Show 打开窗口。
- Quit 退出应用。

---

## 3. 不做什么

V0 禁止实现：

- command provider
- parser system
- settings view
- config.json
- real provider
- autostart
- updater
- notification
- opencode-quota / Kimi / BigModel preset

---

## 4. 自动验收标准

Codex 必须自己跑：

```bash
npm install
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

如果测试脚本不存在，应补齐最小 Vitest 配置。

### 4.1 Rust unit tests

至少包含：

```text
mock_provider_returns_app_snapshot
quota_percentages_are_in_range
```

### 4.2 Frontend tests

至少包含：

```text
ProviderCard renders provider name
ProgressBar clamps percent into 0..100
formatPercent handles null
```

### 4.3 Smoke 验收

由于 GUI 托盘在无头环境可能不可见，Codex 必须提供至少一个可观测 smoke 入口：

```bash
npm run smoke
```

或者 Rust 测试覆盖 tray menu 构建函数。

---

## 5. 完成定义

V0 完成必须满足：

- 项目可安装依赖。
- 前端 build 通过。
- Rust test 通过。
- mock provider 可以生成合法 `AppSnapshot`。
- React UI 可以渲染 mock snapshot。
- 托盘菜单相关代码存在并可编译。
- 未引入 V1 之后的复杂功能。

---

## 6. 给 Codex 的短 Prompt

```text
目标版本：V0。实现 Tauri 2 + React + TS + Rust 的 QuotaBarWin 技术验证版。只做托盘、主窗口、mock provider、refresh_snapshot、基础测试和 smoke 验收。不要做配置文件、settings、command provider、真实 provider。
```
