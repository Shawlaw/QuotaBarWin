# QuotaBarWin 远程 Provider 指南

默认语言：简体中文。English documentation:
[`remote-provider-guide.en.md`](remote-provider-guide.en.md).

远程 Provider 允许你通过一个托管的 `registry.json` / `provider.json` 安装额度
Provider，而不需要把 Provider 打包进 QuotaBarWin 主程序。这适合第三方 Provider、
快速迭代 Provider 脚本，或在多台机器之间共享 Provider 配置。

## 工作方式

1. 在设置页提供 registry URL 或本地路径，例如 `registry.json`。支持 `https://`、
   `file://` 和普通本地路径。
2. QuotaBarWin 读取 registry，拉取每个 Provider manifest，校验可选 checksum，
   并下载 source script。
3. Source script 会缓存到本机，并使用 manifest 声明的 runtime 执行，例如
   `node`、`python`、`pwsh`、`bash` 或绝对路径。
4. 每次刷新时，QuotaBarWin 运行缓存脚本，并把 stdout 解析为标准额度窗口。

## Manifest 格式（`provider.json`）

```json
{
  "schemaVersion": 1,
  "id": "kimi-coding",
  "displayName": "Kimi Coding Usage",
  "version": "1.0.0",
  "description": "Kimi coding quota usage via remote provider script",
  "runtime": "node",
  "entry": "provider.cjs",
  "requiredEnvVars": ["KIMI_API_KEY"],
  "output": "provider-snapshot-v1",
  "permissions": ["env:KIMI_API_KEY"],
  "checksums": {
    "source": "sha256:<hex>"
  }
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `schemaVersion` | 是 | 必须为 `1`。 |
| `id` | 是 | 唯一 Provider id，不能与当前配置中的 Provider 冲突。 |
| `displayName` | 是 | UI 中显示的人类可读名称。 |
| `version` | 否 | 人类可读版本号，会显示在设置页，建议使用 SemVer。缺省时 UI 会回退显示短 checksum。 |
| `description` | 否 | 简短说明。 |
| `runtime` | 是 | 执行 `entry` 的 runtime，例如 `node`、`python`、`pwsh`、`bash` 或绝对路径。 |
| `entry` | 是 | Source 文件名。相对路径按 manifest 所在位置解析；也支持 HTTPS / file / 本地路径。 |
| `requiredEnvVars` | 否 | 脚本需要的环境变量。刷新时会先查 provider `envVars`，再解析 `${secret:NAME}`。 |
| `output` | 是 | 当前仅支持 `provider-snapshot-v1`。 |
| `permissions` | 否 | 声明能力，目前主要用于说明。建议用 `env:<NAME>` 标注环境变量。 |
| `checksums.source` | 否 | Source 文件 SHA-256。启用安全 auto-update 时需要，格式为 `sha256:<hex>`。`version` 只用于展示，不替代 checksum 校验。 |

## Registry 格式（`registry.json`）

Registry 可以用一个 URL 安装多个 Provider：

```json
{
  "schemaVersion": 1,
  "providers": [
    {
      "id": "kimi-coding",
      "providerUrl": "kimi-coding/provider.json",
      "checksum": "sha256:<manifest-sha256>"
    }
  ]
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `schemaVersion` | 是 | 必须为 `1`。 |
| `providers` | 是 | Provider 条目数组。 |
| `providers[].id` | 是 | Provider id，必须与引用的 manifest 中的 id 一致。 |
| `providers[].providerUrl` | 是 | `provider.json` 的 URL 或本地路径。相对路径按 registry 所在位置解析。 |
| `providers[].checksum` | 否 | Manifest 文本 SHA-256；提供后安装前会校验。 |

## Source Script 输出协议

当 `output` 为 `provider-snapshot-v1` 时，脚本必须向 stdout 输出一个 JSON 对象。
`id`、`name` 和 `source` 可以省略，QuotaBarWin 会使用 manifest / config 中的值。

```json
{
  "status": "ok",
  "updatedAt": "2026-06-12T10:00:00.000Z",
  "windows": [
    {
      "id": "weekly",
      "label": "Weekly limit",
      "remaining": 88,
      "used": 12,
      "limit": 100,
      "unit": "requests",
      "usedPercent": 12,
      "remainingPercent": 88,
      "warningRemaining": 20,
      "resetAt": "2026-06-19T10:00:00.000Z",
      "resetText": null,
      "confidence": "exact"
    }
  ],
  "metadata": {}
}
```

### Window 字段

| 字段 | 必填 | 说明 |
|---|---|---|
| `id` | 是 | 稳定窗口 ID，会被用户配置引用。 |
| `label` | 是 | UI 显示名称，可以更友好或本地化，但不能作为唯一稳定身份。 |
| `used` | 否 | 已用数量。 |
| `remaining` | 否 | 剩余数量，余额型 Provider 很常用。 |
| `limit` | 否 | 总额度。 |
| `unit` | 否 | 单位，例如 `requests`、`tokens`、`percent`、`CNY`。 |
| `usedPercent` | 否 | 0-100。 |
| `remainingPercent` | 否 | 0-100。 |
| `warningRemaining` | 否 | 剩余数量低于该值时进入 warning 状态。 |
| `resetAt` | 否 | ISO 8601 时间。 |
| `resetText` | 否 | 人类可读重置说明。 |
| `confidence` | 否 | `exact`、`estimated` 或 `unknown`。 |

## 稳定窗口 ID 与用户自定义

请把每个 `windows[].id` 当成 Provider 的兼容性契约。用户可以通过本地配置中的
`visibleWindowIds` 和 `windowLabelOverrides` 控制显示顺序、可见性和名称；如果你随意
改 ID，会破坏这些偏好。

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "visibleWindowIds": ["300-minute", "usage", "total-quota"],
  "windowLabelOverrides": {
    "300-minute": "5h",
    "usage": "Weekly"
  },
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
  }
}
```

- `visibleWindowIds` 控制展示哪些窗口以及顺序。
- `windowLabelOverrides` 优先匹配 `window.id`，再兼容旧 label 匹配。
- `label` 只是展示文案，可以变得更清晰或本地化；`id` 应来自稳定 API 语义，例如
  `weekly`、`300-minute`、`tokens-limit-6-1`、`total-quota`。

## 解析 Provider 原始响应

远程 Provider 应把供应商 API 的解析逻辑留在脚本内部。QuotaBarWin 只需要脚本最终
输出标准 `provider-snapshot-v1`。

推荐流程：

1. 请求 Provider 原始 API，或在测试中读取 fixture。
2. 选择用户真正需要看到的 quota / balance 记录。
3. 将供应商字段转换成稳定的 `windows[]` 字段。
4. 将 plan、usageDetails、account metadata 等附加信息放进 `metadata`。
5. stdout 只输出一个 JSON 对象；诊断信息写 stderr。

示例映射：

- BigModel：将 `data.limits[]` 映射为 `windows[]`，用 `type/unit/number` 组合稳定
  ID，`currentValue -> used`，`usage -> limit`，`percentage -> usedPercent`，
  `nextResetTime -> resetAt`。
- Kimi：将 300-minute `limits[].detail` 映射为 `300-minute` / `5h`，将 `usage`
  映射为 weekly，并从 `totalQuota.limit - totalQuota.remaining` 推导总额度用量。
- Codex：将 `rate_limit.primary_window` 映射为 `5h`，将
  `rate_limit.secondary_window` 映射为 `weekly`；因为 API 主要返回百分比，
  `used` 和 `limit` 可以为 `null`。
- DeepSeek：将每个 `balance_infos[]` 货币映射为类似 `balance-cny` 的窗口，
  `remaining` 为 `total_balance`，`unit` 为币种。可用本地 env var 提供参考总额和
  低余额阈值。

## 本地配置与 Secret

远程脚本不应该包含凭据。脚本仍然读取 `process.env.NAME`，但 QuotaBarWin 只会把
配置中的环境变量注入到子进程。

`${secret:NAME}` 会优先读取 `<config-dir>/secrets/NAME.txt`，找不到时回退到环境变量
`NAME`。`${env:NAME}` 和 `${file:C:\path\secret.txt}` 也仍然支持。

示例：

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
  }
}
```

## 实现示例：Zhipu / BigModel

以下示例调用 Zhipu / BigModel quota endpoint，并输出 `provider-snapshot-v1`。
它们都从 `BIGMODEL_API_KEY` 读取 API key。

### Node.js

```js
const token = process.env.BIGMODEL_API_KEY;
if (!token) {
  console.error("BIGMODEL_API_KEY is required");
  process.exit(1);
}

fetch("https://open.bigmodel.cn/api/monitor/usage/quota/limit", {
  headers: { Authorization: `Bearer ${token}` },
})
  .then((res) => {
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  })
  .then((body) => {
    const limits = body.data?.limits ?? [];
    const windows = limits.map((l) => ({
      id: `${l.type}-${l.unit}-${l.number}`.toLowerCase(),
      label: `${l.type} / ${l.unit}`,
      used: l.currentValue ?? null,
      limit: l.usage ?? null,
      unit: l.type === "TOKENS_LIMIT" ? "tokens" : null,
      usedPercent: l.percentage ?? null,
      remainingPercent:
        l.percentage == null ? null : Math.max(0, 100 - l.percentage),
      resetAt: l.nextResetTime
        ? new Date(l.nextResetTime).toISOString()
        : null,
      resetText: null,
      confidence: "exact",
    }));
    console.log(JSON.stringify({
      status: "ok",
      updatedAt: new Date().toISOString(),
      windows,
      metadata: {},
    }));
  })
  .catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
```

### Python

```python
import json, os, sys, urllib.request, datetime

token = os.environ.get("BIGMODEL_API_KEY")
if not token:
    sys.exit("BIGMODEL_API_KEY is required")

req = urllib.request.Request(
    "https://open.bigmodel.cn/api/monitor/usage/quota/limit",
    headers={"Authorization": f"Bearer {token}"}
)

with urllib.request.urlopen(req) as res:
    body = json.load(res)

limits = body.get("data", {}).get("limits", [])
windows = []
for l in limits:
    pct = l.get("percentage")
    windows.append({
        "id": f"{l['type']}-{l['unit']}-{l['number']}".lower(),
        "label": f"{l['type']} / {l['unit']}",
        "used": l.get("currentValue"),
        "limit": l.get("usage"),
        "unit": "tokens" if l.get("type") == "TOKENS_LIMIT" else None,
        "usedPercent": pct,
        "remainingPercent": None if pct is None else max(0, 100 - pct),
        "resetAt": datetime.datetime.fromtimestamp(
            l["nextResetTime"] / 1000, tz=datetime.timezone.utc
        ).isoformat() if l.get("nextResetTime") else None,
        "resetText": None,
        "confidence": "exact"
    })

print(json.dumps({
    "status": "ok",
    "updatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "windows": windows,
    "metadata": {}
}, ensure_ascii=False))
```

### PowerShell

```powershell
$token = $env:BIGMODEL_API_KEY
if (-not $token) { throw "BIGMODEL_API_KEY is required" }

$res = Invoke-RestMethod -Uri "https://open.bigmodel.cn/api/monitor/usage/quota/limit" -Headers @{
  Authorization = "Bearer $token"
}

$windows = @()
foreach ($l in $res.data.limits) {
  $pct = $l.percentage
  $windows += @{
    id = "$($l.type)-$($l.unit)-$($l.number)".ToLower()
    label = "$($l.type) / $($l.unit)"
    used = $l.currentValue
    limit = $l.usage
    unit = if ($l.type -eq "TOKENS_LIMIT") { "tokens" } else { $null }
    usedPercent = $pct
    remainingPercent = if ($null -eq $pct) { $null } else { [math]::Max(0, 100 - $pct) }
    resetAt = if ($l.nextResetTime) {
      ([DateTimeOffset]::FromUnixTimeMilliseconds($l.nextResetTime).UtcDateTime).ToString("o")
    } else { $null }
    resetText = $null
    confidence = "exact"
  }
}

@{
  status = "ok"
  updatedAt = (Get-Date).ToString("o")
  windows = $windows
  metadata = @{}
} | ConvertTo-Json -Depth 10
```

### Bash（Git Bash）

```bash
#!/usr/bin/env bash
set -euo pipefail

TOKEN="${BIGMODEL_API_KEY:-}"
[ -z "$TOKEN" ] && { echo "BIGMODEL_API_KEY is required" >&2; exit 1; }

RESPONSE=$(curl -fsS -H "Authorization: Bearer $TOKEN" \
  "https://open.bigmodel.cn/api/monitor/usage/quota/limit")

UPDATED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

WINDOWS=$(echo "$RESPONSE" | jq '[.data.limits[] | {
  id: "\(.type)-\(.unit)-\(.number)" | ascii_downcase,
  label: "\(.type) / \(.unit)",
  used: .currentValue,
  limit: .usage,
  unit: (if .type == "TOKENS_LIMIT" then "tokens" else null end),
  usedPercent: .percentage,
  remainingPercent: (if .percentage == null then null else [0, 100 - .percentage] | max end),
  resetAt: (if .nextResetTime == null then null else (.nextResetTime / 1000 | strflocaltime("%Y-%m-%dT%H:%M:%SZ")) end),
  resetText: null,
  confidence: "exact"
}]')

jq -n --arg updatedAt "$UPDATED_AT" --argjson windows "$WINDOWS" \
  '{status: "ok", updatedAt: $updatedAt, windows: $windows, metadata: {}}'
```

Bash 示例需要 `jq`。Windows 上 Git Bash 通常会随附它。

## 安全检查清单

- 只安装你信任来源的远程 Provider。
- 安装前检查 source URL、runtime 和 required env vars。
- 优先使用带 `providers[].checksum` 的 registry。
- 优先使用带 `checksums.source` 的 manifest；否则无法安全自动更新。
- 缓存的 source 文件位于 app data 目录下的 `providers/remote/<id>/`。

## 示例

完整示例见 [`examples/remote-providers/`](../examples/remote-providers/)：

- `kimi-coding`：Kimi coding quota，需要 `KIMI_API_KEY`。
- `bigmodel-coding-plan`：Zhipu / BigModel coding plan quota，需要
  `BIGMODEL_API_KEY`。
- `codex-usage`：ChatGPT/Codex 5h 和 weekly usage，默认读取
  `~/.codex/auth.json`，也支持 `CODEX_ACCESS_TOKEN` 等覆盖。
- `deepseek-balance`：DeepSeek pay-as-you-go balance，需要
  `DEEPSEEK_API_KEY`，支持余额参考值和低余额阈值配置。

## 更新远程 Provider

如果 manifest 包含 `checksums.source`，QuotaBarWin 可以检测 source 变化。设置页会显示
已安装版本、安装时间、更新时间和上次检查时间：

- **自动更新**：安装时为 Provider 启用 auto-update 后，checksum 不同时会静默更新。
- **手动更新**：在 Settings 中使用 “检查更新” / “应用更新”。

如果缺少 `checksums.source`，需要移除并重新添加 Provider 才能更新。
