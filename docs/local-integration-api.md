# 本地集成 API

QuotaBarWin 1.3.0 起提供本地 HTTP API，供本机脚本、自动化、浏览器扩展和其他桌面应用读取归一化后的额度数据。它不是 Provider 接口，也不替代 [Agent / CLI](cli.md) 的阈值判断功能。

## 启用与监听范围

默认关闭。启用后只监听：

```text
http://127.0.0.1:41833
```

可在 **设置 → 通用 → 本地集成 API** 中关闭服务或修改端口。端口范围为 `1` 到 `65535`。

如需让局域网中的受信任设备访问，可选择一个或多个活动网卡；每张网卡的全部活动 IPv4 和 IPv6 地址都会监听。也可选择“所有活动网卡”，让服务监听当前所有活动 IP 地址。网络监听默认会同时保留 `127.0.0.1`，且该回环地址无需令牌；可在设置页关闭这项附加监听。勾选该回环地址时，未选择外部网卡也可以保存并只提供本机服务。网络变化后，请保存一次设置或重启应用以重新绑定地址。启用外部网卡地址前，必须先在设置页保存手动填写或随机生成的 Bearer token；未保存令牌时无法保存网络监听设置。已保存令牌以掩码状态显示，可直接复制到调用方配置中，也可手动替换或重新生成。该地址上的每个请求都必须携带它。IPv6 地址使用方括号形式；链路本地地址还需要 URL 编码的接口 scope，例如 `http://[fe80::1234%2512]:41833`。

该服务使用 HTTP，不包含 TLS。不要将它暴露到互联网、端口转发、公共 Wi-Fi 或不受信任的网络。即使使用了令牌，也建议只在你能控制的私有网络中使用。

令牌与主配置分开保存在当前配置目录的 `secrets` 下；它不会写入日志、诊断导出或 API 响应。切换 portable 模式时会随当前配置迁移。

## 鉴权

默认回环监听不需要鉴权。与网络监听同时启用的 `127.0.0.1` 同样无需鉴权；只有外部网卡地址上的请求必须提供：

```http
Authorization: Bearer <在设置页中取得的令牌>
```

令牌错误或缺失时返回：

```json
{
  "error": {
    "code": "unauthorized",
    "message": "A valid Authorization bearer token is required."
  }
}
```

## 端点

所有响应均为 `application/json; charset=utf-8`。服务不会发送 CORS 响应头；浏览器场景应通过同机扩展、原生桥接或受控后端调用，而不是开放给任意网页跨域访问。

### `GET /v1/health`

返回服务版本和是否已有可读取的快照。

```json
{
  "apiVersion": 1,
  "appVersion": "1.3.0",
  "snapshotAvailable": true
}
```

### `GET /v1/snapshot`

返回最近一次成功或可用的 `AppSnapshot`。字段与桌面应用的归一化快照保持一致，例如：

```json
{
  "schemaVersion": 1,
  "refreshedAt": "2026-08-19T12:00:00Z",
  "providers": [
    {
      "id": "codex-usage",
      "name": "Codex Usage",
      "status": "ok",
      "source": "remote",
      "windows": []
    }
  ]
}
```

若尚未产生快照，返回 `404` 和 `snapshot_unavailable`。快照暂时不可读取时返回 `503`。为避免泄露 Provider 内部信息，每个 Provider 的 `metadata` 与 `diagnostics` 均会清空为 `null`；错误文本也会先经过敏感信息脱敏。

### `POST /v1/refresh`

异步安排一次完整刷新。成功接收请求即返回 `202`，随后轮询 `GET /v1/snapshot` 获取结果：

```json
{
  "accepted": true,
  "status": "scheduled"
}
```

已有 API 刷新在进行时，同样返回 `202`，但内容为：

```json
{
  "accepted": false,
  "status": "refresh-in-progress"
}
```

刷新使用应用原有的 Provider 执行、缓存、重试和并发保护逻辑。该端点不会写入 Provider 配置或凭据。

## 示例

在默认本机监听下：

```powershell
Invoke-RestMethod http://127.0.0.1:41833/v1/health
Invoke-RestMethod http://127.0.0.1:41833/v1/snapshot
Invoke-RestMethod -Method Post http://127.0.0.1:41833/v1/refresh
```

在选中的局域网网卡上监听时，将令牌保存在调用方自己的安全配置中并传入请求头：

```powershell
$headers = @{ Authorization = "Bearer <本地集成 API 令牌>" }
Invoke-RestMethod -Headers $headers http://<选中的网卡地址>:41833/v1/snapshot
```

不要把真实令牌提交到代码、脚本仓库、日志或问题反馈中。
