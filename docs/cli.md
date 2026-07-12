# Agent / CLI 使用指南

English version: [`cli.en.md`](cli.en.md)。

`QuotaBarWin.Cli.exe` 是 QuotaBarWin 提供给 Agent、CI 和脚本的命令行入口。它复用桌面程序当前使用的配置、secret 占位符、已安装的远程 Provider 和 Provider 缓存；它不会打开窗口、托盘，也不会启动第二个 GUI 实例。

发布版 portable zip 中，`QuotaBarWin.Cli.exe` 与 `QuotaBarWin.exe` 位于同一目录。两者会识别同目录的 `quotabarwin.portable`，因而共享 portable 配置、Provider 缓存和快照。没有 portable marker 时，两者共享 `%APPDATA%\QuotaBarWin` 配置。

## 命令

```powershell
# 列出所有已启用 Provider 的最新数据；默认刷新后再输出
.\QuotaBarWin.Cli.exe get

# 只刷新并读取指定实例的一个窗口
.\QuotaBarWin.Cli.exe get --provider codex-usage --window 5h

# 不联网，只读上一次落盘快照
.\QuotaBarWin.Cli.exe get --provider codex-usage --cached

# 供 Agent 判断是否可以执行高消耗步骤
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
```

`get` 和 `check` 默认使用 `--refresh`。该操作会按已安装 Provider 的配置执行脚本和网络请求，也会更新桌面应用可见的快照。`--cached` 只读取 `last_snapshot.quotaBarWin.json`；快照不存在时命令失败。不要把没有刷新时间约束的缓存数据用于高风险或高消耗决策。

可选 `--config C:\path\config.quotaBarWin.json` 使用明确的本地配置文件，方便隔离测试环境。不要在命令行中传入 token、cookie、API key 或代理凭据；仍应使用已有的 `${secret:...}`、`${env:...}` 和 `${file:...}` 配置方式。

## 输出协议

除 `--help` 与 `--version` 外，命令总是在 stdout 输出一个 JSON 对象。协议版本为 `schemaVersion: 1`。输出只包含通用 Provider 和 quota window 字段；不会输出 `metadata`、Provider 诊断或 stderr，以免将 Provider 私有数据带入 Agent 日志。

`get` 输出形状如下：

```json
{
  "schemaVersion": 1,
  "dataSource": "refresh",
  "refreshedAt": "2026-07-10T00:00:00Z",
  "providers": [
    {
      "id": "codex-usage",
      "name": "Codex",
      "status": "ok",
      "updatedAt": "2026-07-10T00:00:00Z",
      "windows": [
        {
          "id": "5h",
          "remainingPercent": 42,
          "resetAt": "2026-07-10T05:00:00Z",
          "confidence": "high"
        }
      ]
    }
  ]
}
```

`dataSource` 为 `refresh` 或 `cache`。每个 Provider 保留当前状态；`error`、`stale` 状态不能安全地用于自动继续任务。窗口字段沿用公共 `QuotaWindow` 契约，具体是否有绝对 `remaining`、`limit`、百分比或 `resetAt` 由 Provider 决定。

## `check` 的退出码与决策

`check` 必须指定 `--provider`、`--window` 和 `--min-remaining-percent`（范围 0–100）。输出 JSON 中的 `decision` 为：

| 退出码 | decision | 含义 |
|---:|---|---|
| 0 | `continue` | `remainingPercent` 大于或等于阈值。 |
| 10 | `defer` | 剩余百分比低于阈值；建议延后高消耗工作或切换任务。 |
| 11 | `unknown` | Provider 为 `error` / `stale`，或没有有效的 `remainingPercent`；不要把它当成额度充足。 |
| 20 | — | 配置、刷新、缓存或查找 Provider / window 失败。 |
| 64 | — | 命令参数无效。 |

如果发生命令级错误，stdout 仍为 JSON：

```json
{
  "schemaVersion": 1,
  "error": {
    "code": "check_failed",
    "message": "Quota window 5h was not found"
  }
}
```

## Agent 编排示例

PowerShell 中可以只根据退出码做决策：

```powershell
.\QuotaBarWin.Cli.exe check --provider codex-usage --window 5h --min-remaining-percent 20
switch ($LASTEXITCODE) {
  0  { "继续执行高消耗步骤" }
  10 { "改做低消耗工作；在 resetAt 附近重新检查" }
  11 { "额度未知；短暂退避后重新刷新，不假设额度可用" }
  default { throw "QuotaBarWin CLI 查询失败：$LASTEXITCODE" }
}
```

对长任务而言，建议在阶段边界或昂贵操作前检查，而非对每个小操作都刷新。QuotaBarWin 会在 GUI 和 CLI 的刷新之间使用跨进程锁，避免它们同时运行 Provider；不过 Provider 本身仍可能有速率限制。

`resetAt` 是 Provider 返回的重置或建议再次检查时间。它可能代表滚动窗口、相对重试时间或服务端约束，不能理解为“到该时刻额度必定完全恢复”。在 `resetAt` 附近重新执行 `check --refresh` 后，再决定是否恢复高消耗工作。
