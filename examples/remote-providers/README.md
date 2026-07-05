# 远程 Provider 示例

English version: [`README.en.md`](README.en.md).

这里放的是 QuotaBarWin 可直接托管和安装的远程 Provider 示例。

## 目录结构

每个子目录包含：

- `provider.json`：远程 Provider manifest。
- `provider.cjs`：QuotaBarWin 执行的源脚本。

`manifest.example.json` 是独立的最小 manifest 模板，创建新 Provider 时可以复制后修改。

`registry.json` 是列出所有示例 Provider 的注册表。在 QuotaBarWin 中打开 **设置 → 提供方 → 远程安装源**，把它的 URL 或本地路径填入 **注册表 URL**，点击 **安装注册表** 即可一次安装这些示例。

本仓库托管的示例 registry URL：

```text
https://raw.githubusercontent.com/Shawlaw/QuotaBarWin/main/examples/remote-providers/registry.json
```

## Provider 列表

| Provider | 必需环境变量 | 说明 |
|----------|--------------|------|
| `kimi-coding` | `KIMI_API_KEY` | Kimi 编程额度用量。 |
| `bigmodel-coding-plan` | `BIGMODEL_API_KEY` | 智谱 / BigModel 编程套餐额度。 |
| `codex-usage` | 默认无 | ChatGPT / Codex 5h 与 weekly 用量。默认读取 `~/.codex/auth.json`；可选 `CODEX_ACCESS_TOKEN`、`CODEX_ACCOUNT_ID` 或 `CODEX_AUTH_FILE` 覆盖本地 Codex auth 文件；支持通过 `QBWIN_PROXY_URL` 注入运行时代理。 |
| `deepseek-balance` | `DEEPSEEK_API_KEY` | DeepSeek 按量付费余额。可选 `DEEPSEEK_BALANCE_REFERENCE_TOTAL`、`DEEPSEEK_BALANCE_WARNING` 和 `DEEPSEEK_BALANCE_CURRENCY`；也可以追加 `_CNY` 或其他币种代码做按币种覆盖。 |

## 使用方式

1. 将整个 `remote-providers/` 目录上传到静态托管服务，例如 GitHub Raw。
2. 在 QuotaBarWin 中打开 **设置 → 提供方 → 远程安装源**。
3. 将 `registry.json` 的 raw URL 填入 **注册表 URL**，点击 **安装注册表**。
4. 检查安装结果摘要。

Manifest 格式和输出协议见 [`docs/remote-provider-guide.md`](../../docs/remote-provider-guide.md)。

## 解析模式

每个 `provider.cjs` 都把 Provider 专属 API 解析逻辑留在脚本内部，并向 QuotaBarWin 输出标准化的 `provider-snapshot-v1` 对象。

改造这些示例时，建议沿用这个模式：

1. 请求原始 API 响应，或在测试时读取 fixture。
2. 用简短注释说明解析器期望的原始响应形状。
3. 将原始额度记录转换为 `windows[]`，使用稳定的 `id`、可读的 `label`、可用时提供数值型 `used` / `limit`、百分比和 ISO 重置时间。
4. 将套餐等级、模型用量、账户元数据、原始状态码等 Provider 专属细节放进 `metadata`。
5. 凭据只保留在本地。这些示例读取 `process.env.NAME`；QuotaBarWin 可从已安装 Provider 的 `envVars`、`<config-dir>/secrets/NAME.txt` 下的 `${secret:NAME}` 文件，或环境变量 fallback 注入，而不需要把密钥写进远程源码。

## 稳定窗口 ID

Provider 窗口 ID 是面向用户配置的键。QuotaBarWin 支持用 `visibleWindowIds` 选择显示哪些窗口并控制顺序，也支持用 `windowLabelOverrides` 重命名窗口。标签覆盖会优先匹配 `window.id`，所以 Provider 发布新版本时应保持 ID 稳定。

`label` 只用于友好的 UI 文本，可以更清晰、可本地化，也可以以后重命名；它不应成为唯一稳定身份。优先使用来自 Provider API 语义的 ID，例如 `5h`、`weekly`、`300-minute`、`tokens-limit-6-1` 或 `total-quota`，避免从翻译标签或营销文案生成 ID。

示例：

- BigModel 将 `data.limits[]` 映射到 `windows[]`，使用 `tokens-limit-6-1` 等稳定 ID，并将 `currentValue -> used`、`usage -> limit`、`percentage -> usedPercent`、`nextResetTime -> resetAt`。
- Kimi 将 300 分钟 `limits[].detail` 条目映射为 id `300-minute`、label `5h`，将 `usage` 映射为 id `usage`、label `Weekly limit`，并从 `totalQuota.limit - totalQuota.remaining` 推导总额度用量。
- Codex 将 `rate_limit.primary_window` 映射为 id/label `5h`，将 `rate_limit.secondary_window` 映射为 id `weekly`、label `Weekly limit`；因为 API 返回百分比，`used` 和 `limit` 保持为 `null`。
- DeepSeek 将每个 `balance_infos[]` 币种映射为 `balance-cny` 这样的稳定 ID。由于 API 返回的是当前余额而非额度上限，如需进度条和绝对低余额警告，可以设置 `DEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200`、`DEEPSEEK_BALANCE_WARNING_CNY=20` 等本地环境变量。

## 更新 checksum

如果修改源脚本，请重新计算 SHA-256 checksum 并更新 `provider.json`：

```bash
sha256sum provider.cjs
```

然后将 `checksums.source` 设置为 `sha256:<hex>`。

如果修改 `provider.json` 本身，包括 `version` 等展示元数据，也需要重新计算该 manifest 的 checksum 并更新 `registry.json`。
