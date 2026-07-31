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
3. Source script 会缓存到本机，并使用 manifest 声明的 runtime 执行。`builtin-js`
   使用应用内置的 QuickJS，不需要用户安装 Node.js；`node`、`python`、`pwsh`、`bash`
   和绝对路径仍会启动对应的外部进程。
4. 每次刷新时，QuotaBarWin 运行缓存脚本，并归一化为标准额度窗口。

同一个 manifest 可以安装多次，用于查询同一 Provider 的多个账号。QuotaBarWin 会为
每个本地账号实例生成稳定的 Provider id，例如 `kimi-coding`、`kimi-coding-2`，
每个实例可以配置不同的名称、环境变量、超时和窗口显示偏好。运行期代理通过 Provider
环境变量配置；项目全局代理只在该变量未配置时作为兜底。

## Manifest 格式（`provider.json`）

```json
{
  "schemaVersion": 2,
  "id": "kimi-coding",
  "displayName": "Kimi Coding Usage",
  "version": "1.2.0",
  "minAppVersion": "1.1.0",
  "description": "Kimi coding quota usage via remote provider script",
  "runtime": "builtin-js",
  "entry": "provider.js",
  "requiredEnvVars": ["KIMI_API_KEY"],
  "output": "provider-snapshot-v1",
  "permissions": ["env:KIMI_API_KEY", "net:https://api.kimi.com"],
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
      "description": "Kimi coding quota API token.",
      "helpUrl": "https://provider.example.com/api-keys"
    }
  ],
  "checksums": {
    "source": "sha256:<hex>"
  }
}
```

| 字段 | 必填 | 说明 |
|---|---|---|
| `schemaVersion` | 是 | `1` 是旧格式；当前应用仍可加载已缓存的 schema 1 `builtin-js` 脚本以完成升级，但远程安装或更新 `builtin-js` 必须使用 `2`，以便旧本体在下载脚本前安全拒绝不兼容更新。 |
| `id` | 是 | 稳定 manifest id。重复安装同一 manifest 时，QuotaBarWin 会自动生成不冲突的本地 Provider id。 |
| `displayName` | 是 | UI 中显示的人类可读名称。 |
| `version` | 否 | 人类可读版本号，会显示在设置页，建议使用 SemVer。缺省时 UI 会回退显示短 checksum。 |
| `description` | 否 | 简短说明。 |
| `minAppVersion` | schema 2 是 | 运行此 Provider 所需的最低 QuotaBarWin 版本，使用不带 `v` 前缀的 SemVer，例如 `1.1.0`。当前本体低于此版本时会拒绝更新且不改写缓存。 |
| `runtime` | 是 | 执行 `entry` 的 runtime。`builtin-js` 使用内置 QuickJS；也可用 `node`、`python`、`pwsh`、`bash` 或绝对路径。 |
| `entry` | 是 | Source 文件名。相对路径按 manifest 所在位置解析；也支持 HTTPS / file / 本地路径。 |
| `requiredEnvVars` | 否 | 脚本需要的环境变量。刷新时会先查 provider `envVars`，再解析 `${secret:NAME}`。 |
| `output` | 是 | 当前仅支持 `provider-snapshot-v1`。 |
| `permissions` | 否 | 对外部 runtime 是说明字段；对 `builtin-js` 是强制能力边界，格式和用法见下一节。 |
| `defaultConfig` | 否 | 首次安装时写入本地 provider 配置的默认值，例如 `name`、`timeoutSeconds`、`visibleWindowIds`、`windowLabelOverrides`、`envVars`。后续 provider 更新不会覆盖用户本地修改。 |
| `parameters` | 否 | 设置页展示的结构化参数。每项可包含 `name`、`label`、`kind`、`required`、`defaultValue`、`placeholder`、`description`、`options`、`helpUrl`、`advanced`。不要放真实凭据。 |
| `checksums.source` | 否 | Source 文件 SHA-256。启用安全 auto-update 时需要，格式为 `sha256:<hex>`。`version` 只用于展示，不替代 checksum 校验。 |

### 向导式配置字段

当前版本会基于 `parameters` 生成安装后的配置表单：`secret` 为密码框，`string` 为文本框，`number` 为数字框，`select` 为下拉框。必填字段会先校验，已保存的 secret 永不回传或回显给前端。

`helpUrl` 是获取 API Key 或参数说明的外部帮助链接。设置 `"advanced": true` 会将可选、代理或诊断类字段默认收进“高级设置”；不要将正常首次配置所需的字段标为高级。

新实例会以“待配置”、停用状态安装。用户点击“保存并测试”时，宿主会把 secret 写入该实例独立的本地文件，并把 `${secret:providers/<provider-instance-id>/<parameter-name>}` 写入主配置；解析仍由同一套 `${secret:...}` resolver 和正式 Provider runner 完成。Provider 作者不应要求用户手工创建文件或输入占位符。

第三方 manifest 未提供 `parameters` 时仍可安装，用户可使用保留的原始 `envVars` 编辑器；这保持对旧 manifest 的兼容。

## 内置 JavaScript runtime（`builtin-js`）

`builtin-js` 面向不想让普通用户额外安装 Node.js 的 Provider。它运行在应用内置的
QuickJS 沙箱中。新发布或远程更新的 manifest 必须使用 `schemaVersion: 2` 并声明 `minAppVersion`；入口必须是 `.js` 文件，`output` 必须为 `provider-snapshot-v1`，并导出
一个同步的全局函数 `main(qb)`：函数直接返回快照对象，**不使用** `console.log`、stdout
或 `process.exit`。官方 Provider 都使用此 runtime。

```js
function main(qb) {
  const token = qb.env.get("EXAMPLE_API_TOKEN");
  const response = qb.http.request("https://api.example.com/usage", {
    headers: { Authorization: `Bearer ${token}` }
  });
  if (!response.ok) throw new Error(`Usage API returned ${response.status}`);
  const raw = JSON.parse(response.body);
  qb.log({ level: "info", stage: "snapshot.ready", message: "Usage loaded" });
  return {
    status: "ok",
    updatedAt: qb.now(),
    windows: [{ id: "monthly", label: "Monthly", remainingPercent: raw.remaining, confidence: "exact" }]
  };
}
```

对应 manifest 至少要显式授予脚本使用的能力：

```json
{
  "schemaVersion": 2,
  "minAppVersion": "1.1.0",
  "runtime": "builtin-js",
  "entry": "provider.js",
  "output": "provider-snapshot-v1",
  "requiredEnvVars": ["EXAMPLE_API_TOKEN"],
  "permissions": ["env:EXAMPLE_API_TOKEN", "net:https://api.example.com"]
}
```

### `qb` API 与权限

| API | 所需 permission | 行为与边界 |
|---|---|---|
| `qb.env.get(name)` | `env:NAME` 或 `env-prefix:PREFIX_` | 读取已配置的值；未配置时抛错。`requiredEnvVars` 中的每项必须有匹配的 env permission。 |
| `qb.env.getOptional(name)` | 同上 | 未配置时返回 `null`；未声明仍会抛错。适用于可选阈值或筛选项。 |
| `qb.fs.readText(path)` | `fs:C:\exact\path`、`fs:~/.codex/auth.json` 或 `fs:env:NAME` | 只读 UTF-8 文本；宿主会规范化实际路径并拒绝未授权路径。`fs:env:NAME` 允许读取该环境变量指向的一个文件。 |
| `qb.http.request(url, options)` | `net:http`、`net:https`，或精确 origin 如 `net:https://api.example.com` | 同步 HTTP 请求，返回 `{ status, ok, body }`。Provider 专用代理或项目全局代理由宿主使用；脚本看不到代理凭据。请求体最多 1 MiB，响应文本最多 2 MiB。 |
| `qb.now()` / `qb.timezone()` | 无 | 分别返回当前 UTC ISO 时间和本机 UTC 偏移（如 `UTC+08:00`）。 |
| `qb.log(entry)` | 无 | 写入本地结构化应用日志。建议传 `{ level, stage, message }`，不要传 secret、token 或完整响应。 |
| `qb.meta` | 无 | 只读通用元信息：`providerId`、`manifestId`、`name`、`version`、`sourceChecksum`、`timeoutSeconds`。 |

`net:https://api.example.com` 只允许该 scheme、主机和端口，路径由脚本决定；`net:https`
允许任意 HTTPS origin，应只在确有需要时使用。`env-prefix:` 适合
`DEEPSEEK_BALANCE_WARNING_` 这种按币种动态命名的可选参数。`QBWIN_` 是宿主保留前缀，
不能通过 `env:` 或 `fs:env:` 授权；通用元信息用 `qb.meta` 读取，代理由 `qb.http` 自动使用。

### 支持范围与非目标

可使用标准同步 JavaScript 及 `Date`、`JSON`、`RegExp`、`Map`、`Set`。`main(qb)` 必须
同步返回可 JSON 序列化的对象；不支持 Promise、顶层 await、ES module/import、`require`、
Node/Bun/Deno API、`process`、`console`、子进程、任意 socket、任意文件访问或动态加载。
`eval` 也不是受支持能力。运行时限制为 16 MiB 内存、512 KiB JS 栈，以及实例配置中的整体
`timeoutSeconds`；每个 HTTP 请求还会受剩余总时间限制。

这套边界是平台能力，不包含任何 Provider 专属 API、鉴权格式或响应解析。URL、Header、
本地 auth 文件格式和 `windows[]` 映射都仍由 Provider 脚本维护。外部 runtime 继续可用，
但不获得这套强制沙箱。

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
.\QuotaBarWin.Cli.exe validate --manifest .\provider.json --source .\provider.js

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

当 `output` 为 `provider-snapshot-v1` 时，外部 runtime 脚本必须向 stdout 输出一个 JSON
对象，且 stdout 只能包含这个最终对象；流程、调试和错误日志请写 stderr。`builtin-js`
不使用 stdout/stderr：它的 `main(qb)` 直接返回同形状对象，并使用 `qb.log()` 写日志。
`id`、`name` 和 `source` 可以省略；即使脚本提供，QuotaBarWin 也会优先使用本地安装
配置中的 Provider id 和名称，以支持同一 manifest 的多账号实例。

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
| `QBWIN_PROXY_URL` | 可选；优先使用 Provider 环境变量中的该值；未配置时宿主注入项目全局代理。安装源代理只用于下载 registry、manifest 和脚本。日志中只记录是否启用或协议，不要输出完整值。 |

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
- Codex：优先按 `limit_window_seconds` 识别 5 小时和周窗口，避免 API 调换
  `primary_window` / `secondary_window` 的位置；该字段缺失时兼容旧的位置映射。因为 API
  主要返回百分比，`used` 和 `limit` 可以为 `null`。
- DeepSeek：将每个 `balance_infos[]` 货币映射为类似 `balance-cny` 的窗口，
  `remaining` 为 `total_balance`，`unit` 为币种。可用本地 env var 提供参考总额和
  低余额阈值。

## 本地配置与 Secret

远程脚本不应该包含凭据。外部 runtime 脚本读取 `process.env.NAME`；`builtin-js` 脚本
通过受 permission 约束的 `qb.env.get("NAME")` 或 `qb.env.getOptional("NAME")` 读取。
QuotaBarWin 只会解析和提供实例配置中的环境变量。

对于带 `parameters` 的安装后表单，普通用户输入的 secret 会自动保存到：

```text
<config-dir>/secrets/providers/<provider-instance-id>/<parameter-name>.txt
```

对应 `envVars` 只保存 `${secret:providers/<provider-instance-id>/<parameter-name>}`。这是本地明文文件而非系统凭据存储；它会随便携目录复制，应用不会把其内容写入 config、日志、诊断或表单回显。这个应用托管形式也通过下述既有 `${secret:...}` resolver 读取。

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

### 应用版本兼容性

发布或远程更新 `builtin-js` Provider 时必须使用 manifest schema 2，并设置其实际所需的
`minAppVersion`。更新前，宿主会先校验 schema 与最低版本；若本体版本不足，会保留当前
缓存、配置和旧脚本不变，并提示用户先升级 QuotaBarWin。旧版应用只支持 schema 1，因此也会
在下载新 source **之前**拒绝 schema 2 manifest；这避免了自动更新将还能工作的 Node Provider
覆盖为旧本体无法执行的 `builtin-js` 脚本。为平滑升级，当前版本仍可运行已缓存的 schema 1
`builtin-js` Provider，但不会从远程安装或更新这类旧 manifest。
