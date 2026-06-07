# Codex 自主管理 Token、上下文与验收工作流

> 目标：Codex 自己控制上下文读取范围、实现范围、命令验证和失败修复。人类只需要指定目标版本或说“继续下一个版本”。

---

## 1. 最高优先级规则

Codex 必须遵守：

```text
只读取当前任务必要上下文。
只实现当前目标版本。
不提前实现后续版本。
不把 GUI 行为交给人工验收。
失败后先自修复，再重新运行验收。
```

---

## 2. 自主上下文预算策略

### Step A：入口文档

先读：

```text
README.md
00_overview_and_coding_contract.md
10_codex_self_managed_workflow.md
```

### Step B：确定目标版本

如果用户明确指定版本，例如 `V2`，只打开对应版本文档。

如果用户没有指定版本，Codex 自己判断最早未完成版本，依据优先级：

```text
1. package.json / src-tauri / src 等关键目录是否存在
2. README / CHANGELOG 是否标记已完成版本
3. 测试和脚本是否覆盖目标版本验收项
4. 文件结构 / git diff / 现有实现能力
```

判断结果不超过 12 行。

### Step C：只读当前版本文档

只打开当前版本 spec，例如：

```text
04_v2_provider_parser_capabilities.md
```

不要提前打开后续版本文档。需要前置上下文时，只读前一版本文档的相关章节。

### Step D：代码搜索要精确

优先搜索：

```text
ProviderSnapshot|AppSnapshot|ProviderConfig|ParserSpec|CommandSpec
refresh_snapshot|get_config|save_config
Command::new|redact|diagnostics
```

不要无目的全仓长读。不要研究 CodexBar / opencode-quota 外部仓库全文。

---

## 3. 自主实现计划格式

开始改代码前，Codex 只能输出短计划：

```text
目标版本：Vx
判断依据：...
本轮只做：...
明确不做：...
预计修改文件：...
验收命令：...
```

限制：不超过 12 行。

---

## 4. 自主验收与自修复协议

实现后必须运行当前版本文档要求的验收命令。默认至少包括：

```bash
npm install
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

如果脚本不存在，Codex 应补齐合理脚本，而不是跳过。

如果命令失败：

```text
1. 读取失败日志
2. 定位原因
3. 做最小修复
4. 重新运行失败命令
5. 直到通过，或明确说明环境限制并提供替代验证
```

---

## 5. GUI / 托盘 / 通知的自验收要求

不得把这些项目简单写成“人工检查”：

```text
- 托盘菜单
- 关闭窗口隐藏
- 启动隐藏
- 通知
- tooltip
- 多屏 / DPI
```

Codex 应优先增加可观测验证方式：

```text
- npm run smoke
- Rust unit test
- TS component test
- fake provider test
- config roundtrip test
- structured log assertion
- diagnostic JSON output
```

允许保留可选视觉复核 checklist，但它不能作为版本完成前置条件。

---

## 6. Provider 实现边界防跑偏

Codex 必须应用：

```text
Provider 是唯一对外概念。
Parser/adapter 是 provider 内部实现，不进入主 UI 概念层。
opencode-quota 是 optional preset，不是核心依赖。
Kimi/BigModel 真实接口不得用于自动验收，只能用 fixture。
API Key 不得写入配置或日志。
```

---

## 7. 版本边界防跑偏句

### V0

```text
不要做设置页、配置文件、command provider、真实 provider。
```

### V1

```text
不要接真实 provider。只实现 mock provider、generic command provider、direct app/provider snapshot parser。
```

### V1.1

```text
不要改 provider 协议。只打磨 Windows 常驻体验。
```

### V2

```text
不要新增 preset UI，不调用真实 API。只增强 Provider 内部 parser 能力，覆盖 Kimi JSON 和 BigModel 文本 fixture。
```

### V3

```text
Preset 只是 ProviderConfig 模板。不要把 opencode-quota 变成核心依赖，不要调用真实网络接口做验收。
```

### V4

```text
不要新增 provider，只做预测、瓶颈窗口、提醒和建议。
```

### V5

```text
不要新增 provider，只做产品化、更新、迁移、诊断和发布。
```

### V6

```text
优先 CLI 集成 CommandHub，不引入不必要的常驻 HTTP server。
```

---

## 8. 输出格式：机器可读优先

每轮完成后，Codex 回复必须包含：

```text
版本：Vx
状态：pass | partial | blocked
变更摘要：
- ...
修改文件：
- ...
已运行命令：
- <command> => pass/fail
自动验收：
- ... => pass/fail
自修复记录：
- ...
未能自动验证：
- 项目：...
  原因：...
  替代验证：...
下一步：
- ...
```
