# DeepSeek Balance API 依据

English version: [`api.en.md`](api.en.md).

本文记录 `deepseek-balance/provider.js`（`builtin-js`）当前实现依据的请求、响应形状和字段映射。它是示例 Provider 的实现说明，不代表第三方 API 的稳定官方契约；接口变化时应同步更新脚本和本文档。

## 请求

| 项目 | 值 |
|------|----|
| Method | `GET` |
| URL | `https://api.deepseek.com/user/balance` |
| 鉴权 | `Authorization: Bearer <DEEPSEEK_API_KEY>` |
| 固定请求头 | `Accept: application/json` |
| 必需环境变量 | `DEEPSEEK_API_KEY` |

可选本地显示配置：

| 环境变量 | 说明 |
|----------|------|
| `DEEPSEEK_BALANCE_CURRENCY` | 只显示指定币种；如果响应不包含该币种，脚本报错。 |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL` | 用于计算 `used` 和进度条的参考总额。 |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL_<CURRENCY>` | 按币种覆盖参考总额，例如 `DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY`。 |
| `DEEPSEEK_BALANCE_WARNING` | 低余额警告阈值。 |
| `DEEPSEEK_BALANCE_WARNING_<CURRENCY>` | 按币种覆盖低余额阈值，例如 `DEEPSEEK_BALANCE_WARNING_CNY`。 |

## 响应形状

当前解析器依赖下面这些字段：

```json
{
  "is_available": true,
  "balance_infos": [
    {
      "currency": "CNY",
      "total_balance": "110.00",
      "granted_balance": "10.00",
      "topped_up_balance": "100.00"
    }
  ]
}
```

## 字段映射

| 原始字段或本地配置 | 输出位置 | 说明 |
|--------------------|----------|------|
| `balance_infos[]` | `windows[]` | 每个币种生成一个余额窗口。 |
| `currency` | `windows[].id` | 生成 `balance-cny` 这样的稳定 ID。 |
| `currency` | `windows[].label` | 生成 `CNY balance` 这样的标签。 |
| `total_balance` | `windows[].remaining` | 当前余额，转为数字。 |
| `currency` | `windows[].unit` | 余额单位。 |
| `DEEPSEEK_BALANCE_REFERENCE_TOTAL[_<CURRENCY>]` | `windows[].limit` | 参考总额，转为数字。 |
| `referenceTotal - total_balance` | `windows[].used` | 只有参考总额和余额都可用时计算。 |
| `DEEPSEEK_BALANCE_WARNING[_<CURRENCY>]` | `windows[].warningRemaining` | 低余额阈值，转为数字。 |
| `total_balance` | `windows[].resetText` | 显示为 `Balance <amount> <currency>`。 |
| `is_available` | `metadata.isAvailable` | 账户是否可用。 |
| `DEEPSEEK_BALANCE_CURRENCY` | `metadata.currency` | 当前请求的币种过滤条件。 |
| `balance_infos[]` | `metadata.balances` | 保留每个币种的 `totalBalance`、`grantedBalance`、`toppedUpBalance`。 |

DeepSeek 返回的是余额，不是周期性 quota。没有参考总额时，窗口仍然显示 `remaining`，但 `used` 和 `limit` 为 `null`。

## 输出窗口

| 窗口 ID | Label 示例 | 来源 |
|---------|------------|------|
| `balance-cny` | `CNY balance` | `balance_infos[]` 中 `currency = "CNY"`。 |

## 错误与状态

- 缺少 `DEEPSEEK_API_KEY` 时脚本退出并报错。
- HTTP 非 2xx 响应会抛出 `DeepSeek balance request failed with <status>`。
- 设置了 `DEEPSEEK_BALANCE_CURRENCY` 但响应没有对应币种时脚本报错。
- `is_available === false` 时输出 `status: "warning"`。
- 任意窗口余额低于或等于本地 warning 阈值时输出 `status: "warning"` 并提供 diagnostics。
- 其他可解析响应输出 `status: "ok"`。

## 本地参考

- 示例脚本：[`provider.js`](provider.js)
- Manifest：[`provider.json`](provider.json)
- 原始响应 fixture：[`../../../fixtures/provider_outputs/deepseek_balance.json`](../../../fixtures/provider_outputs/deepseek_balance.json)
