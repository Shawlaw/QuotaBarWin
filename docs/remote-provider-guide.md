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

同一个 manifest 可以安装多次，用于查询同一 Provider 的多个账号。QuotaBarWin 会为
每个本地账号实例生成稳定的 Provider id，例如 `kimi-coding`、`kimi-coding-2`，
每个实例可以配置不同的名称、环境变量、代理、超时和窗口显示偏好。

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
  "defaultConfig": {
    "name": "Kimi",
    "visibleWindowIds": ["300-minute", "usage"],
    "windowLabelOverrides": {
      "300-minute": "5小时限",
      "usage": "周限"
    },
    "envVars": {
      "KIMI_API_KEY": "${secret:KIMI_API_KEY}"
    }
  },
  "parameters": [
    {
      "name": "KIMI_API_KEY",
      "label": "Kimi API Key",
      "kind": "secret",
      "required": true,
      "defaultValue": "${secret:KIMI_API_KEY}",
      "description": "Kimi coding quota API token."
    }
  ],
  "checksums": {
    "source": "sha256:<hex>"
  }
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `schemaVersion` | 是 | 必须为 `1`。 |
| `id` | 是 | 稳定 manifest id。重复安装同一 manifest 时，QuotaBarWin 会自动生成不冲突的本地 Provider id。 |
| `displayName` | 是 | UI 中显示的人类可读名称。 |
| `version` | 否 | 人类可读版本号，会显示在设置页，建议使用 SemVer。缺省时 UI 会回退显示短 checksum。 |
| `description` | 否 | 简短说明。 |
| `runtime` | 是 | 执行 `entry` 的 runtime，例如 `node`、`python`、`pwsh`、`bash` 或绝对路径。 |
| `entry` | 是 | Source 文件名。相对路径按 manifest 所在位置解析；也支持 HTTPS / file / 本地路径。 |
| `requiredEnvVars` | 否 | 脚本需要的环境变量。刷新时会先查 provider `envVars`，再解析 `${secret:NAME}`。 |
| `output` | 是 | 当前仅支持 `provider-snapshot-v1`。 |
| `permissions` | 否 | 声明能力，目前主要用于说明。建议用 `env:<NAME>` 标注环境变量。 |
| `defaultConfig` | 否 | 首次安装时写入本地 provider 配置的默认值，例如 `name`、`timeoutSeconds`、`visibleWindowIds`、`windowLabelOverrides`、`envVars`。后续 provider 更新不会覆盖用户本地修改。 |
| `parameters` | 否 | 设置页展示的参数提示。每项可包含 `name`、`label`、`kind`、`required`、`defaultValue`、`placeholder`、`description`、`options`。不要放真实凭据。 |
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

## 用 CLI 校验 Provider

编写或更新 Provider 后，建议在安装和发布前使用 `QuotaBarWin.Cli.exe validate`
校验 manifest、source、checksum 和 runtime。此命令的 stdout 始终是 JSON，适合在
CI 或脚本中根据退出码处理。

```powershell
# 校验本地 manifest；entry 是相对本地路径时会自动找到 source
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json

# entry 为 HTTPS / file URL 时，明确指定本地待校验 source
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json --source .\provider.cjs

# Provider 已安装后，校验实际配置、缓存 manifest、source checksum 和 runtime
.\QuotaBarWin.Cli.exe validate --provider my-provider

# 额外执行一次已安装脚本，校验 provider-snapshot-v1 输出；可能访问账户 API
.\QuotaBarWin.Cli.exe validate --provider my-provider --run
```

默认校验不会运行脚本或发起 Provider API 请求。`--run` 只适用于已安装的 Provider，
会沿用该实例配置的 runtime、secret 占位符和代理；不要在命令行中传入 token、Cookie、
API key 或代理凭据。

校验会检查 `schemaVersion`、非空 `displayName`、`output` 协议、runtime、source 是否
存在、`checksums.source` 是否匹配，以及已安装 Provider 所需环境变量是否已配置。
`checksums.source` 在当前公共协议中仍是可选项：缺失会给出 warning，不会单独导致失败。
`--run` 还会确认 stdout 能被当前的 `provider-snapshot-v1` 解析器接受；Provider stderr、
secret 和环境变量值不会写入报告。

校验不通过时退出码为 `30`；manifest 或配置无法读取等命令级错误退出码为 `20`。详情请见
[`cli.md`](cli.md)。

## Source Script 输出协议

当 `output` 为 `provider-snapshot-v1` 时，脚本必须向 stdout 输出一个 JSON 对象。
stdout 应只包含这个最终对象；流程、调试和错误日志请写入 stderr，否则宿主会把
stdout 当成 JSON 解析并失败。`id`、`name` 和 `source` 可以省略；即使脚本提供，
QuotaBarWin 也会优先使用本地安装配置中的 Provider id 和名称，以支持同一 manifest
的多账号实例。

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

### 流程日志与宿主元信息

QuotaBarWin 会逐行读取脚本 stderr，并把日志转发到本地应用日志
`quotabarwin.log`。脚本失败、返回非 0 退出码或被宿主超时终止时，最近约 16 KiB 的
stderr 摘要会进入 Provider diagnostics 的 `stderr` 字段，方便区分 provider 内部
失败和宿主进程超时。普通文本会原样转发；更推荐每行写一个结构化 JSON 对象。

推荐字段：

| 字段 | 说明 |
|---|---|
| `level` | `debug`、`info`、`warn` 或 `error`；写入日志时会受应用 `logLevel` 过滤。 |
| `stage` | 当前阶段，例如 `auth.loaded`、`usage.request.start`、`usage.response`、`snapshot.ready`。 |
| `message` | 简短的人类可读说明。 |
| `providerId` | 可选；宿主日志本身也会带 provider id。 |
| `version` | 可选；建议读取 `QBWIN_PROVIDER_VERSION`。 |
| `sourceChecksum` | 可选；建议读取 `QBWIN_PROVIDER_SOURCE_CHECKSUM`。宿主日志会显示短 checksum。 |
| 其他字段 | 仅建议记录布尔值、数字和短字符串。嵌套对象不会进入日志摘要。 |

宿主会在执行脚本前注入这些保留环境变量：

| 环境变量 | 说明 |
|---|---|
| `QBWIN_PROVIDER_ID` | 当前安装配置中的 provider id。 |
| `QBWIN_PROVIDER_MANIFEST_ID` | manifest 声明的 provider id。 |
| `QBWIN_PROVIDER_NAME` | UI 显示名。 |
| `QBWIN_PROVIDER_VERSION` | manifest `version`，缺省时不设置。 |
| `QBWIN_PROVIDER_SOURCE_CHECKSUM` | manifest `checksums.source`，缺省时不设置。 |
| `QBWIN_PROVIDER_TIMEOUT_SECONDS` | 当前宿主等待脚本退出的秒数。 |
| `QBWIN_PROXY_URL` | 可选；配置了 provider 代理时注入，脚本可用它发起网络请求。日志中只记录是否启用或协议，不要输出完整值。 |

Node.js 示例：

```js
function logStep(level, stage, message, fields = {}) {
  process.stderr.write(`${JSON.stringify({
    level,
    providerId: process.env.QBWIN_PROVIDER_ID || "my-provider",
    version: process.env.QBWIN_PROVIDER_VERSION || null,
    sourceChecksum: process.env.QBWIN_PROVIDER_SOURCE_CHECKSUM || null,
    stage,
    message,
    ...fields,
  })}\n`);
}

logStep("info", "usage.request.start", "Fetching usage", {
  timeoutSeconds: process.env.QBWIN_PROVIDER_TIMEOUT_SECONDS || null,
});

console.log(JSON.stringify({
  status: "ok",
  updatedAt: new Date().toISOString(),
  windows: [],
  metadata: {},
}));
```

如果 provider 自己设置 HTTP / CLI 超时，建议低于 `QBWIN_PROVIDER_TIMEOUT_SECONDS`，
这样脚本有机会先输出 `level=error` 的失败日志并正常退出；否则只能由宿主记录
`timeoutOrigin=host`。不要在日志里输出 token、API key、Cookie、授权头、账号 ID、
代理凭据或完整代理 URL。即使宿主会做基础脱敏，provider 仍应从源头避免泄露敏感值。

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
  "id": "kimi-coding-2",
  "name": "Kimi Coding - Work",
  "timeoutSeconds": 30,
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
- `timeoutSeconds` 控制 QuotaBarWin 宿主进程等待脚本退出的时间，默认 `30` 秒。
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

单账号时，如果 manifest 声明了 `requiredEnvVars: ["KIMI_API_KEY"]`，并且你没有在
设置页填写 `envVars`，QuotaBarWin 会默认尝试读取：

```text
<config-dir>/secrets/KIMI_API_KEY.txt
```

多账号时，请在每个本地 Provider 实例的 **设置 → 提供方 → 编辑 → 环境变量** 中显式
填写映射。等号左侧仍然是脚本需要的环境变量名；等号右侧才是这个账号使用的本地
secret 文件名。

例如同一个 Kimi Provider 的两个账号：

```json
{
  "kind": "remote",
  "id": "kimi-coding",
  "name": "Kimi Personal",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY_PERSONAL}"
  }
}
```

```json
{
  "kind": "remote",
  "id": "kimi-coding-2",
  "name": "Kimi Work",
  "envVars": {
    "KIMI_API_KEY": "${secret:KIMI_API_KEY_WORK}"
  }
}
```

对应的本地文件是：

```text
<config-dir>/secrets/KIMI_API_KEY_PERSONAL.txt
<config-dir>/secrets/KIMI_API_KEY_WORK.txt
```

这样两个实例都会向脚本注入同一个 `process.env.KIMI_API_KEY`，但值来自不同的
secret 文件；配置文件中只保存占位符，不保存明文 token。

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

- **只安装使用可信任的远程 Provider。Provider 脚本可以直接读取你配置给它的各类
  AI 鉴权信息，并发起网络通讯。**
- 安装前检查 source URL、runtime 和 required env vars。
- 优先使用带 `providers[].checksum` 的 registry。
- 优先使用带 `checksums.source` 的 manifest；否则无法安全自动更新。
- 缓存的 source 文件位于 app data 目录下的 `providers/remote/<id>/`，这里的
  `<id>` 是本地安装实例 id；同一个 manifest 的多个账号会使用不同目录。

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
