# QuotaBarWin Provider Runtime 规划设计文档

> 这一版替换旧路线中的 `opencode-quota adapter` 主线。新的核心判断是：**Provider 是唯一对外数据提供抽象；command、curl、CLI、parser、adapter 都只是 Provider 内部实现细节。**

## 如何给 Codex 使用

建议只给 Codex 一个入口指令：

```text
目标版本：V1。按本文档自主管理上下文、实现、验收和自修复。
```

或者：

```text
继续实现最早未完成版本。按文档自主管理上下文、实现、验收和自修复。
```

Codex 应先读取：

```text
README.md
00_overview_and_coding_contract.md
10_codex_self_managed_workflow.md
```

然后只读取当前目标版本文档，不要一次性读完整个文档包。

## 文档列表

| 文件 | 用途 |
|---|---|
| `00_overview_and_coding_contract.md` | 总体架构、Provider 抽象、数据协议、全局编码契约 |
| `01_v0_technical_spike.md` | V0：Tauri + tray + mock provider 技术验证 |
| `02_v1_generic_command_provider.md` | V1：通用 command provider MVP |
| `03_v1_1_windows_polish.md` | V1.1：WindowsFirst 常驻体验打磨 |
| `04_v2_provider_parser_capabilities.md` | V2：Provider 内部 parser 能力增强 |
| `05_v3_provider_presets.md` | V3：Provider presets，包括 Kimi 与 BigModel/Z.ai 示例 |
| `06_v4_forecast_and_alerts.md` | V4：瓶颈窗口、预测、提醒 |
| `07_v5_productization_and_distribution.md` | V5：安装包、更新、诊断、发布 |
| `08_v6_commandhub_integration.md` | V6：CommandHub / 多 Agent 联动 |
| `09_provider_examples_and_fixtures.md` | Kimi JSON 与 BigModel 文本样例、期望归一化结果 |
| `10_codex_self_managed_workflow.md` | Codex 自主管理 token、上下文、验收和自修复规则 |

## 新版本路线

| 版本 | 名称 | 目标 |
|---|---|---|
| V0 | Technical Spike | 跑通 Tauri + tray + mock provider |
| V1 | Generic Command Provider MVP | 所有用量来源先统一成“命令执行结果”，Provider 负责输出标准 Snapshot |
| V1.1 | Windows Polish | 启动隐藏、托盘 tooltip、通知、缓存、配置恢复、窗口位置记忆 |
| V2 | Provider Parser Capabilities | Provider 内部支持 direct/json/text parser，覆盖 Kimi JSON 与 BigModel 文本 fixture |
| V3 | Provider Presets | 提供 Kimi、BigModel/Z.ai、opencode-quota、自定义 command 等 preset；Kimi/BigModel 均以 JSON fixture 验收 |
| V4 | Forecast & Alerts | 瓶颈窗口、reset 倒计时、低额度预测、使用建议 |
| V5 | Productization | installer、自动更新、配置迁移、诊断包、发布流水线 |
| V6 | CommandHub Integration | quota 接入 Agent 启动、群聊、任务调度 |

## 关键变化

旧设计里容易误把 `opencode-quota` 当成主干依赖。新设计中：

```text
opencode-quota 只是一个 provider preset
Kimi curl 是一个 provider preset
BigModel/Z.ai curl 是一个 provider preset；当前规范以 JSON 响应为准
Codex CLI 是一个 provider preset
自定义脚本也是一个 provider preset
```

真正的主干是：

```text
Provider Runtime
  -> Provider Implementation
    -> Fetch by command/native/mock
    -> Parse internally
    -> Normalize to ProviderSnapshot
  -> AppSnapshot
  -> UI Render
```

## 不要做的事

- 不要把 adapter 作为 UI 或产品层的一等概念。
- 不要把 opencode-quota 写进核心架构依赖。
- 不要在配置中默认保存明文 API Key。
- 不要让 Codex 调真实 Kimi / BigModel 接口做验收。
- 不要用 shell 字符串拼接 command。
- 不要一次实现所有 provider。
