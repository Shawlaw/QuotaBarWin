# OpenCode Go Usage API 依据

English version: [`api.en.md`](api.en.md).

本文记录 `opencode-go/provider.js`（`builtin-js`）当前实现依据的请求、响应形状和字段映射。它是示例提供方的实现说明，不代表第三方 API 的稳定公开契约；接口变化时应同步更新脚本和本文档。

## 请求

| 项目 | 值 |
|------|----|
| Method | `GET` |
| URL | `https://opencode.ai/zen/go/v1/usage` |
| 鉴权 | `Authorization: Bearer <OPENCODE_GO_API_KEY>` |
| 固定请求头 | `Accept: application/json`、`User-Agent: QuotaBarWin/1.7` |
| 超时 | 提供方实例的 `timeoutSeconds`（默认 30 秒） |

密钥只来自提供方配置：安装后在「保存并测试」表单里填写 OpenCode Go API Key，应用会把它写入实例隔离的托管 secret，并注入为 `OPENCODE_GO_API_KEY`。本示例只支持 API Key，不读取本机 opencode 登录凭据或 SQLite 数据库；需要复用本机登录时，可参考 `codex-usage` 的本地 auth 文件模式扩展脚本。

代理由宿主处理：提供方 Proxy URL 优先，项目全局代理兜底；脚本看不到代理凭据。安装源代理只用于下载 registry、manifest 和脚本。支持 `socks5:`、`socks5h:`、`http:` 和 `https:` 代理。

## 响应形状

主形状是 `usage` 对象下的三个窗口：

```json
{
  "usage": {
    "mine": true,
    "useBalance": false,
    "rolling": { "status": "ok", "percent": 12, "resetsAt": "2026-11-04T13:20:00.000Z" },
    "weekly": { "status": "ok", "percent": 34, "resetsAt": "2026-11-09T00:00:00.000Z" },
    "monthly": { "status": "ok", "percent": 56, "resetsAt": "2026-12-03T08:30:00.000Z" }
  }
}
```

`percent` 是**已用**百分比，单位是 0–100 的百分数：`1` 表示已用 1%，`0.5` 表示已用 0.5%。它不是 0–1 的小数比例。

脚本同时兼容另外两种形状：

- 控制台页面 hydration 数据的字段名：`usage.rollingUsage` / `weeklyUsage` / `monthlyUsage`，数值字段为 `usagePercent`，重置字段为相对秒数 `resetInSec`。
- 旧版提案的 `usage.windows[]` 数组，每项用 `name`（或 `id`）匹配 `rolling`、`weekly`、`monthly`，并接受 `percent` 或 `usagePercent`。

## 字段映射

| 原始字段 | 输出位置 | 说明 |
|----------|----------|------|
| `usage.rolling` / `rollingUsage` | `windows[]` 中 id 为 `5h` 的窗口 | 5 小时滚动窗口。 |
| `usage.weekly` / `weeklyUsage` | `windows[]` 中 id 为 `weekly` 的窗口 | 本周窗口；缺失时不输出该窗口。 |
| `usage.monthly` / `monthlyUsage` | `windows[]` 中 id 为 `monthly` 的窗口 | 本月窗口；缺失时不输出该窗口。 |
| `percent` / `usagePercent` | `windows[].usedPercent` | 限定在 0–100。 |
| `100 - percent` | `windows[].remainingPercent` | 剩余百分比，下限为 0。 |
| `resetsAt` | `windows[].resetAt` | ISO 时间字符串。 |
| `resetInSec` / `resetAfterSeconds` | `windows[].resetAt` | 当前时间加相对秒数后转 ISO 时间。 |
| `useBalance`（顶层或 `usage` 内） | `metadata.useBalance` | 可选布尔值，原样保留。 |
| `plan` / `billing.subscriptionPlan` | `metadata.plan` | 可选套餐名。 |

接口只返回百分比，不返回剩余金额或绝对额度，所以 `used` 和 `limit` 保持为 `null`，`unit` 设为 `percent`。OpenCode Go 没有单独的余额金额字段；需要看钱请到 OpenCode Console 的 Billing 页面。

## 输出窗口

| 窗口 ID | Label | 来源 |
|---------|-------|------|
| `5h` | `5h` | `usage.rolling`（API）或 `rollingUsage`（控制台形状）。 |
| `weekly` | `Weekly limit` | `usage.weekly` 或 `weeklyUsage`。 |
| `monthly` | `Monthly limit` | `usage.monthly` 或 `monthlyUsage`。 |

三个窗口按 5h、weekly、monthly 顺序输出；某个窗口在响应里缺失时不会创建空窗口。`defaultConfig.visibleWindowIds` 同时展示三者，用户仍可在设置中调整可见窗口和标签。

## 错误与状态

- 没有 `OPENCODE_GO_API_KEY` 时脚本退出并提示在 OpenCode Console 创建 API Key。
- HTTP `401` 时提示 key 被拒绝，HTTP `403` 时提示 key 可能缺少 Usage 权限或账号没有 Go 订阅。
- 其它非 2xx 状态码抛出 `OpenCode Go usage API returned <status>`，正文只截取前 160 个字符。
- 响应无法解析为 JSON 时抛出解析错误。
- 响应里没有 `usage` 对象时抛出明确错误，提示 key 必须属于有 Go 订阅的账号。
- 至少生成一个窗口时输出 `status: "ok"`；`usage` 存在但没有任何可识别窗口时输出 `status: "warning"` 和空 `windows[]`。

## 本地参考

- 原始响应 fixture：[`../../../fixtures/provider_outputs/opencode_go_usage.json`](../../../fixtures/provider_outputs/opencode_go_usage.json)
- 示例脚本：[`provider.js`](provider.js)
- Manifest：[`provider.json`](provider.json)
- 输出协议说明：[`../../../docs/remote-provider-guide.md`](../../../docs/remote-provider-guide.md)
