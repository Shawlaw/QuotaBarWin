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

# 校验已安装 Provider 的配置、缓存 manifest、source checksum 与 runtime
.\QuotaBarWin.Cli.exe validate --provider codex-usage

# 在安装前校验本地 manifest 和 source 文件
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json --source .\provider.js
```

`get` 和 `check` 默认使用 `--refresh`。该操作会按已安装 Provider 的配置执行脚本和网络请求，也会更新桌面应用可见的快照。`--cached` 只读取 `last_snapshot.quotaBarWin.json`；快照不存在时命令失败。不要把没有刷新时间约束的缓存数据用于高风险或高消耗决策。

传入 `--provider ID --refresh` 时，CLI 只刷新该实例；不传 `--provider` 才刷新所有已启用 Provider。

可选 `--config C:\path\config.quotaBarWin.json` 使用明确的本地配置文件，方便隔离测试环境。不要在命令行中传入 token、cookie、API key 或代理凭据；仍应使用已有的 `${secret:...}`、`${env:...}` 和 `${file:...}` 配置方式。

## 异常额度确认

刷新时，QuotaBarWin 会把新结果与最后已确认的快照比较。若发现 Provider 时间戳或窗口
`resetAt` 无效/倒退，或同一窗口的已用百分比突然下降至少 20 个百分点（即剩余额度突然
增加），CLI 会保留旧快照并立即只重跑该 Provider 一次确认。

第二次结果回到旧区间时，会采用第二次结果；两次都落在相近的新额度区间时，会采用第二次
结果。若确认请求失败、仍有无效时间，或两次新值相差过大，则保留旧值并把 Provider 标记为
`stale`、窗口置信度标记为 `unknown`。因此 `check` 会返回 `unknown`（退出码 `11`），不会
把未经确认的“额度恢复”当成可以继续高消耗工作的依据。

`quotabarwin.log` 会记录不含凭据和额度原始值的流程事件，例如
`quota verification detected`、`quota verification refresh started` 与
`quota verification finished outcome=confirmed|reverted|pending`，用于排查上游响应波动。

## Provider 校验

`validate` 用于 Provider 作者、CI 和排障场景。默认不会执行 source script 或发起 Provider API 请求；只有显式传入 `--run` 时才会执行。

- `validate --provider ID`：读取本地配置和已缓存 Provider，检查 `providerDir`、`timeoutSeconds`、config runtime 与 manifest runtime 是否一致、required env var 是否已配置、manifest、source 文件、checksum 和 runtime 是否可用。`builtin-js` 显示为内置 runtime，不要求外部可执行文件。
- `validate --manifest PATH`：检查本地 manifest；若 `entry` 是相对本地路径，会自动定位 source。entry 是 URL 时必须传 `--source PATH`。

如需连同实际输出协议一起检查，使用 `validate --provider ID --run`。它会按照该 Provider 已配置的 runtime、secret 和代理执行一次缓存脚本，并确认结果能被当前 `provider-snapshot-v1` 解析器接受；外部 runtime 解析 stdout，`builtin-js` 解析 `main(qb)` 返回值。这可能访问网络和账户 API，因此默认不会执行。结果的 `scriptRun` 为 `notRun`、`passed` 或 `failed`。

它要求当前公开的 `provider-snapshot-v1` 输出协议、`builtin-js`、`node` / `python` / `pwsh` / `bash` 或绝对 runtime 路径，以及非空的 `displayName`。`builtin-js` manifest 必须使用 schema 2 并声明可满足的 `minAppVersion`；这是让旧本体在写入新 source 前拒绝不兼容更新的兼容性门槛。`checksums.source` 目前在公共契约中是可选的：缺失会产生 warning，但不会单独使校验失败。校验结果不会输出配置的环境变量值、secret 或 Provider stderr。

成功或失败时 stdout 都是 JSON。`valid: false` 时退出码为 `30`；无法读取配置或 manifest 等命令级错误为 `20`。例如：

```json
{
  "schemaVersion": 1,
  "target": { "kind": "installed", "providerId": "codex-usage" },
  "valid": true,
  "errors": [],
  "warnings": [],
  "runtime": { "declared": "node", "resolved": "C:\\Program Files\\nodejs\\node.exe" },
  "checksum": { "expected": "sha256:…", "actual": "sha256:…", "matches": true }
}
```

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
| 30 | — | `validate` 发现 Provider 配置、manifest、source、checksum 或 runtime 不符合规范。 |
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
