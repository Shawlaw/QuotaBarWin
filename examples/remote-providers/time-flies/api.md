# 光阴似箭 Provider 说明

English version: [`api.en.md`](api.en.md).

`time-flies/provider.cjs` 不访问网络、文件或账号凭据；它只根据运行机器的本机时区和当前时间，以分钟为单位输出「光阴似箭」的剩余时间窗口。

## 运行要求

| 项目 | 值 |
|------|----|
| Runtime | `node` |
| 必需环境变量 | 无 |
| 网络 / 凭据 / 文件权限 | 无 |
| 输出协议 | `provider-snapshot-v1` |

## 计算约定

- 所有日历边界使用运行机器的**本机时区**；输出的 `resetAt` 为对应本地边界转换后的 ISO 时间。
- 所有窗口都显示到下一个边界的**剩余分钟**，向上取整；例如周三下午会显示到周一零点的精确分钟数。
- 周一开始的窗口在下一个周一零点重置；周日开始的窗口在下一个周日零点重置。
- 月和年使用实际日历长度，因此会正确处理大小月和闰年。

## 输出窗口

| 窗口 ID | 默认标签 | 单位 | 上限 | 重置时间 |
|---------|----------|------|------|----------|
| `today` | 本日余时 | `minutes` | 1440 | 下一本地日零点 |
| `week-monday` | 本周余时（周一开始） | `minutes` | 10080 | 下一个周一零点 |
| `week-sunday` | 本周余时（周日开始） | `minutes` | 10080 | 下一个周日零点 |
| `month` | 本月余时 | `minutes` | 当月总分钟数 | 下月一日零点 |
| `year` | 本年余时 | `minutes` | 当年总分钟数 | 次年一月一日零点 |

每个窗口都提供 `remaining`、`used`、`limit`、`remainingPercent`、`usedPercent` 和 `resetAt`，因此可以沿用 QuotaBarWin 的剩余模式、已用模式、进度条和托盘展示。`metadata.timezone` 记录运行时检测到的时区，`metadata.precision` 固定为 `minutes`。

## 本地参考

- 示例脚本：[`provider.cjs`](provider.cjs)
- Manifest：[`provider.json`](provider.json)
- 输出协议说明：[`../../../docs/remote-provider-guide.md`](../../../docs/remote-provider-guide.md)
