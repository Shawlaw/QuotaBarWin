# Codex Usage API 依据

English version: [`api.en.md`](api.en.md).

本文记录 `codex-usage/provider.cjs` 当前实现依据的请求、响应形状和字段映射。它是示例 Provider 的实现说明，不代表第三方 API 的稳定官方契约；接口变化时应同步更新脚本和本文档。

## 请求

| 项目 | 值 |
|------|----|
| Method | `GET` |
| URL | `https://chatgpt.com/backend-api/wham/usage` |
| 鉴权 | `Authorization: Bearer <access token>` |
| 固定请求头 | `User-Agent: QuotaBarWin/0.0` |
| 可选请求头 | `ChatGPT-Account-Id: <account id>` |
| 超时 | `30000` ms |

Token 和 account id 来源按优先级读取：

1. `CODEX_ACCESS_TOKEN` / `CODEX_ACCOUNT_ID`
2. `CODEX_AUTH_FILE` 指向的 JSON 文件
3. `~/.codex/auth.json`

本地 Codex auth 文件当前读取 `tokens.access_token` 和 `tokens.account_id`。脚本还支持运行时代理，优先使用 `QBWIN_PROXY_URL`，然后依次尝试 `HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`。支持 `socks5:`、`socks5h:`、`http:` 和 `https:` 代理。

## 响应形状

当前解析器依赖下面这些字段：

```json
{
  "plan_type": "plus",
  "credits": {
    "granted": 100,
    "used": 12
  },
  "rate_limit": {
    "primary_window": {
      "used_percent": 32,
      "reset_after_seconds": 7200,
      "limit_window_seconds": 18000
    },
    "secondary_window": {
      "used_percent": 18,
      "reset_at": 1781913600,
      "limit_window_seconds": 604800
    }
  }
}
```

`reset_at` 以 Unix epoch 秒表示。没有 `reset_at` 但存在 `reset_after_seconds` 时，脚本用当前时间加相对秒数生成 ISO 时间。

## 字段映射

| 原始字段 | 输出位置 | 说明 |
|----------|----------|------|
| `rate_limit.*_window.limit_window_seconds` | `windows[].id` | 优先按 `18000` 秒识别为 `5h`，按 `604800` 秒识别为 `weekly`，不依赖 primary / secondary 位置。 |
| `rate_limit.primary_window` / `secondary_window` | `windows[]` | `limit_window_seconds` 缺失或未知时，兼容旧响应：primary 映射为 `5h`、secondary 映射为 `weekly`。 |
| `used_percent` | `windows[].usedPercent` | 转为数字。 |
| `100 - used_percent` | `windows[].remainingPercent` | 剩余百分比，下限为 0。 |
| `reset_at` | `windows[].resetAt` | Unix epoch 秒转 ISO 时间。 |
| `reset_after_seconds` | `windows[].resetAt` | 当前时间加相对秒数后转 ISO 时间。 |
| `plan_type` | `metadata.planType` | 账户套餐类型。 |
| `credits` | `metadata.credits` | 原样保留额度信息。 |

Codex usage API 当前主要返回百分比，不返回绝对计数，所以脚本将 `used` 和 `limit` 保持为 `null`，`unit` 设为 `percent`。

## 输出窗口

| 窗口 ID | Label | 来源 |
|---------|-------|------|
| `5h` | `5h` | `limit_window_seconds = 18000` 的窗口；缺失时回退为 `primary_window`。 |
| `weekly` | `Weekly limit` | `limit_window_seconds = 604800` 的窗口；缺失时回退为 `secondary_window`。 |

## 错误与状态

- 没有 access token 时脚本退出并提示设置 `CODEX_ACCESS_TOKEN` 或登录 Codex 生成本地 auth 文件。
- HTTP 状态码不是 `200` 时抛出 `Codex usage API returned <status>`。
- 响应无法解析为 JSON 时抛出解析错误。
- 至少生成一个窗口时输出 `status: "ok"`；没有可用窗口时输出 `status: "warning"`。

## 本地参考

- 示例脚本：[`provider.cjs`](provider.cjs)
- Manifest：[`provider.json`](provider.json)
- 输出协议说明：[`../../../docs/remote-provider-guide.md`](../../../docs/remote-provider-guide.md)
