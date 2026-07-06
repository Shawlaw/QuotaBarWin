# Kimi Coding API 依据

English version: [`api.en.md`](api.en.md).

本文记录 `kimi-coding/provider.cjs` 当前实现依据的请求、响应形状和字段映射。它是示例 Provider 的实现说明，不代表第三方 API 的稳定官方契约；接口变化时应同步更新脚本、fixture 和本文档。

## 请求

| 项目 | 值 |
|------|----|
| Method | `GET` |
| URL | `https://api.kimi.com/coding/v1/usages` |
| 鉴权 | `Authorization: Bearer <KIMI_API_KEY>` |
| 必需环境变量 | `KIMI_API_KEY` |
| Fixture 环境变量 | `QUOTABARWIN_KIMI_FIXTURE` |

脚本在设置 `QUOTABARWIN_KIMI_FIXTURE` 时跳过网络请求并读取本地 JSON，便于离线验证解析逻辑。

## 响应形状

当前解析器依赖下面这些字段：

```json
{
  "limits": [
    {
      "window": {
        "duration": 300,
        "timeUnit": "TIME_UNIT_MINUTE"
      },
      "detail": {
        "used": "10",
        "remaining": "90",
        "limit": "100",
        "resetTime": "2026-06-07T16:35:14.207781Z"
      }
    }
  ],
  "usage": {
    "used": "15",
    "remaining": "85",
    "limit": "100",
    "resetTime": "2026-06-12T02:35:14.207781Z"
  },
  "totalQuota": {
    "limit": "100",
    "remaining": "99"
  },
  "user": {
    "region": "REGION_CN",
    "membership": {
      "level": "LEVEL_INTERMEDIATE"
    }
  },
  "parallel": {
    "limit": "20"
  },
  "authentication": {
    "method": "METHOD_API_KEY",
    "scope": "FEATURE_CODING"
  },
  "subType": "TYPE_PURCHASE"
}
```

## 字段映射

| 原始字段 | 输出位置 | 说明 |
|----------|----------|------|
| `limits[].window.duration == 300` 且 `timeUnit == "TIME_UNIT_MINUTE"` | `windows[].id = "300-minute"` | 300 分钟窗口显示为 `5h`。 |
| `limits[].detail.used` | `windows[].used` | 转为数字；空值或非数字转为 `null`。 |
| `limits[].detail.remaining` | `windows[].remaining` | 转为数字；用于计算剩余百分比。 |
| `limits[].detail.limit` | `windows[].limit` | 转为数字；与 `used` 或 `remaining` 计算百分比。 |
| `limits[].detail.resetTime` | `windows[].resetAt` | 原样使用 ISO 时间字符串。 |
| `usage` | `windows[].id = "usage"` | 映射为 `Weekly limit` 窗口。 |
| `totalQuota.limit - totalQuota.remaining` | `windows[].id = "total-quota"` 的 `used` | 总额度只输出绝对用量，不计算百分比。 |
| `user.region` | `metadata.region` | 账户区域。 |
| `user.membership.level` | `metadata.membershipLevel` | 会员等级。 |
| `authentication.method` | `metadata.authMethod` | 鉴权方式。 |
| `authentication.scope` | `metadata.authScope` | 鉴权范围。 |
| `subType` | `metadata.subType` | 套餐或额度来源类型。 |
| `parallel.limit` | `metadata.parallelLimit` | 并发限制，转为数字。 |

如果 300 分钟窗口的 `used`、`remaining`、`limit` 都缺失，脚本按“未知视为满额可用”处理：`usedPercent = 0`、`remainingPercent = 100`、`confidence = "estimated"`。

## 输出窗口

| 窗口 ID | Label | 来源 |
|---------|-------|------|
| `300-minute` | `5h` | `limits[].detail` 中的 300 分钟窗口。 |
| `usage` | `Weekly limit` | 顶层 `usage`。 |
| `total-quota` | `Total quota` | 顶层 `totalQuota`。 |

## 错误与状态

- 缺少 `KIMI_API_KEY` 时脚本退出并报错。
- HTTP 非 2xx 响应会抛出 `Kimi usage request failed with <status>`。
- 网络请求成功且 JSON 可解析时输出 `status: "ok"`；缺失的可选窗口只会减少 `windows[]` 数量。

## 本地参考

- 示例脚本：[`provider.cjs`](provider.cjs)
- Manifest：[`provider.json`](provider.json)
- 原始响应 fixture：[`../../../docs/specs/fixtures/provider_outputs/kimi_coding_usage.json`](../../../docs/specs/fixtures/provider_outputs/kimi_coding_usage.json)
- 命令 fixture 包装：[`../../../fixtures/commands/kimi_usage_fixture.js`](../../../fixtures/commands/kimi_usage_fixture.js)
