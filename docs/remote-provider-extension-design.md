# QuotaBarWin 远程 Provider 自刷新扩展方案（细化稿）

> 状态：历史设计稿；远程 provider registry 路线已部分实现  
> 适用范围：GitHub Raw 远程 Provider、自刷新/自更新、全局/按 provider 代理  
> 前置依赖：Phase 1 刷新机制优化（`stale` 状态、成功缓存、`ErrorKind`、重试调度）  
> 运行平台：Windows only

## 当前实现备注（2026-06）

当前代码已经实现的远程 provider 能力：

- 通过 registry 安装远程 provider：`install_remote_provider_registry(url, proxyUrl, autoUpdate)`。
- registry 和 manifest 均支持 `https://`、`file://` 和本地路径；相对路径会按 registry/manifest 位置解析。
- manifest schemaVersion 为 `1`，source checksum 可选；有 `checksums.source` 且安装时启用 auto update 时才会开启自动更新。
- source 脚本缓存在 app data 的 `providers/remote/<id>/` 下，包含 `provider.json`、source 文件、`.meta.json` 和 `.bak`。
- runtime 支持命令名或绝对路径，安装时会解析并用 `--version` / `--help` 校验。
- 全局代理和 provider 代理已实现，优先级为 provider proxy > global HTTP/SOCKS5/system > no proxy。
- Tauri command 当前包括 `install_remote_provider_registry`、`remove_remote_provider`、`refresh_remote_provider`、`check_remote_updates`、`apply_remote_update`、`get_network_proxy`、`set_network_proxy`。
- 前端设置页已有 Remote Providers 区域、registry URL、provider proxy、auto update、检查更新、应用更新、删除 provider 和打开指南入口。

与本设计稿不同或尚未实现的部分：

- 当前入口是 registry 安装，不是单个 `add_remote_provider(url, proxy, auto_update)` command。
- id 冲突当前在安装结果里作为失败/跳过处理，不提供覆盖/重命名弹窗流程。
- 首次安装安全确认弹窗尚未按本设计稿完整实现。
- 定时刷新前按 `updateIntervalSeconds` 自动检查更新的调度逻辑尚未完整接入。
- 以 `docs/remote-provider-guide.md` 作为 manifest 和输出协议的当前事实文档。

---

## 1. 设计目标

1. **远程 Provider 扩展**：用户可通过 HTTPS URL（推荐 GitHub Raw）安装 provider，应用自动下载 `provider.json` + 源码文件，用用户指定的运行时执行。
2. **自刷新/自更新**：远程 provider 仓库更新后，应用能检测到并自动/手动拉取新版本。
3. **代理支持**：访问远程 URL 时可配置全局代理或按 provider 代理，支持 HTTP / SOCKS5；Codex 等现有原生 provider 也复用同一套代理解析逻辑。
4. **安全/可控**：远程脚本必须显式确认、可校验、可禁用，不影响现有本地 provider。

---

## 2. 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│                        User Config                          │
│  { refreshInterval, networkProxy, providers: [ ... ] }      │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   Mock Provider      Command/Script Provider    Remote Provider
                             (本地)                 (新增)
                                                       │
                              ┌──────────────────────┘
                              ▼
                    Rust Backend Remote Fetch Service
                    (reqwest + proxy + cache + retry)
                              │
                ┌─────────────┴─────────────┐
                ▼                           ▼
        Fetch provider.json         Fetch source file
        from remote URL               from remote URL
                │                           │
                └─────────────┬─────────────┘
                              ▼
                  Cache to %APPDATA%/QuotaBarWin/providers/remote/<id>/
                              │
                              ▼
              Execute with user-specified runtime
              (node / python / pwsh / bash / absolute path)
                              │
                              ▼
                    ProviderSnapshot / AppSnapshot
```

核心思想：**远程 provider 在运行前被“物化”成本地缓存的 provider，复用现有 Script provider 的执行模式，但运行时由用户 manifest 指定。**

---

## 3. 关键设计决策（已确认）

| # | 决策点 | 确认结论 |
|---|---|---|
| 1 | **信任模型** | 首次安装弹窗确认；auto_update 时只要 manifest checksum 校验通过就静默更新 |
| 2 | **域限制** | **不限制**，任意 HTTPS URL 均可；首次安装弹窗明确展示来源 URL |
| 3 | **代理优先级** | `per-provider proxy > global config proxy > system proxy > no proxy` |
| 4 | **id 冲突处理** | 远程 provider 与本地/内置 provider id 冲突时**提示用户选择**（覆盖 / 重命名 / 取消）；冲突检测在 `add_remote_provider` command handler 中完成，不在 `remote_provider.rs` 模块内 |
| 5 | **checksum 策略** | manifest 必须包含 `checksums.source` 才能开启 auto_update；无 checksum 则强制手动更新 |
| 6 | **runtime 支持** | Phase 2 直接支持多 runtime：`runtime` 字段可以是命令名（`node`/`python`/`pwsh`/`bash` 等）或绝对路径；安装前校验 `--version` 可用性 |
| 7 | **Phase 2 范围** | 手动 URL 安装远程 provider + 代理 + 多 runtime；扩展市场 Registry Index 放到 Phase 3 |

---

## 4. 交互方案（UI/UX）

### 4.1 设置页新增“远程 Provider”区域

- **添加远程 Provider**：
  - 输入框：`Manifest URL`，例如：
    ```
    https://raw.githubusercontent.com/owner/repo/main/quota-providers/kimi-coding/provider.json
    ```
  - 可选项：`Proxy URL`（留空则使用全局代理）。
  - 可选项：`Auto update`（默认开启，但要求 manifest 带 checksum）。
  - 按钮：Add → 后端拉取 manifest，解析 `id/displayName/requiredEnvVars/runtime`，校验 schema。
- **列表展示**：
  - 名称、来源 URL、runtime、当前缓存版本（commit/ETag/tag）、更新状态。
  - 开关：启用/禁用。
  - 操作：Refresh / Check Update / Edit / Remove。

### 4.2 全局网络代理设置

在 Settings 顶部新增 `Network Proxy`：

- 类型：`HTTP` / `SOCKS5` / `System` / `None`
- 地址：`http://127.0.0.1:7890` 或 `socks5h://127.0.0.1:1080`
- 说明：对 Codex、远程 Provider 请求统一生效；单个 provider 可覆盖。

### 4.3 首次安装安全确认

弹窗确认：

```
Add remote provider "kimi-coding"?
Source: https://raw.githubusercontent.com/owner/repo/...
Runtime: node (resolved to C:\Program Files\nodejs\node.exe)
Required env vars: KIMI_API_KEY
Auto update: on (checksum verified)

⚠ This provider will execute a script on your machine using the runtime above.
Only install providers from sources you trust.
[Trust and Install] [Cancel]
```

如果 manifest 缺少 checksum：

```
Auto update: disabled (no checksum in manifest)
```

### 4.4 id 冲突处理

若 `id` 与现有 provider 冲突，弹窗：

```
A provider with id "kimi-coding" already exists.
[Overwrite existing] [Rename new provider] [Cancel]
```

### 4.5 更新提示

- `autoUpdate=true` + checksum 校验通过：后台静默更新，ProviderCard 显示 `updated just now`。
- `autoUpdate=false` 或缺少 checksum：ProviderCard 显示 `Update available · [Update now]`。
- checksum 校验失败：标记 `Update failed · checksum mismatch`，禁用该 provider 的更新直到人工确认。

---

## 5. 技术方案

### 5.1 Provider 配置模型扩展

全局配置新增 `network_proxy`：

```rust
pub struct AppConfig {
    // ... 现有字段
    pub network_proxy: Option<ProxyConfig>,
}

pub struct ProxyConfig {
    pub kind: ProxyKind, // Http / Socks5 / System / None
    pub url: String,
}
```

`ProviderConfig` 新增 `Remote` 变体：

```rust
pub enum ProviderConfig {
    // ... 现有 Mock / Codex / Command / Script
    Remote {
        id: String,
        name: String,
        enabled: bool,
        manifest_url: String,            // 远程 provider.json URL
        source_url: String,              // 安装时从 manifest.entry 解析并持久化
        runtime: String,                 // 命令名或绝对路径
        resolved_runtime: Option<String>,// 安装时校验并记录的实际可执行路径
        proxy_url: Option<String>,       // 覆盖全局代理
        auto_update: bool,               // 默认 true
        update_interval_seconds: u64,    // 默认 3600
        trusted_checksum: Option<String>,// 首次安装时记录的 source checksum
        window_label_overrides: HashMap<String, String>,
        visible_window_ids: Option<Vec<String>>,
    },
}
```

**说明**：

- `source_url` 在 `add_remote_provider` 时一次性解析并持久化；后续 manifest 变更 `entry` 时，由 `check_update` 重新解析并更新配置。
- `resolved_runtime` 记录安装时实际找到的可执行文件路径；执行时若该路径已不存在，自动重新 resolve。
- `trusted_checksum` 记录当前本地信任的 source 版本；更新时与 manifest 中的 `checksums.source` 对比。

### 5.2 远程 Provider URL 解析

支持直接指向 `provider.json` 的 HTTPS URL：

```
https://raw.githubusercontent.com/owner/repo/main/quota-providers/kimi-coding/provider.json
```

解析规则：

- manifest URL 的 base path 为目录；`source_url = base + manifest.entry`。
- `manifest.entry` 若已是绝对 URL，则直接使用。
- 未来可扩展支持目录 index URL。

### 5.3 Manifest 格式（v1）

```json
{
  "schemaVersion": 1,
  "id": "kimi-coding",
  "displayName": "Kimi Coding Usage",
  "description": "Reads Kimi coding quota and prints provider-snapshot-v1 JSON.",
  "runtime": "node",
  "entry": "provider.cjs",
  "requiredEnvVars": ["KIMI_API_KEY"],
  "output": "provider-snapshot-v1",
  "permissions": ["env:KIMI_API_KEY"],
  "checksums": {
    "source": "sha256:abc123..."
  }
}
```

约束：

- `schemaVersion` 必须为 `1`。
- `runtime` 可以是命令名（如 `node`/`python`/`pwsh`/`bash`）或绝对路径（如 `C:\tools\python3.exe`）。
- `id` 与现有 provider 冲突时提示用户选择。
- `checksums.source` 可选；缺失则 `auto_update` 强制为 `false`。

### 5.4 Runtime 校验（Windows only）

安装或更新 runtime 变更时执行：

1. 先尝试 `<runtime> --version`（超时 5s）。
2. 若失败，再尝试 `<runtime> --help`（超时 5s）。
3. 都失败则报错：`Runtime "bash" not found or not executable. Please install it or provide an absolute path.`

- 校验成功后记录 `resolved_runtime`（绝对路径）。
- 每次执行 provider 时，若 `resolved_runtime` 指向的文件已不存在，自动重新 resolve：
  ```rust
  if !Path::new(&resolved_runtime).exists() {
      resolved_runtime = resolve_runtime(&runtime)?;
  }
  ```

### 5.5 远程拉取服务（Rust）

新增 `src-tauri/src/remote_provider.rs`：

- `fetch_manifest(url, proxy) -> Result<ProviderManifest, RemoteProviderError>`
- `fetch_source(url, proxy) -> Result<String, RemoteProviderError>`
- `resolve_source_url(manifest_url, entry) -> String`
- `cache_remote_provider(id, manifest, source) -> Result<PathBuf, RemoteProviderError>`
- `check_update(id, manifest_url, proxy, trusted_checksum) -> Result<UpdateInfo, RemoteProviderError>`
- `verify_checksum(source, expected_checksum) -> Result<(), RemoteProviderError>`
- `resolve_runtime(runtime: &str) -> Result<PathBuf, RemoteProviderError>`

错误类型：

```rust
enum RemoteProviderError {
    Network(String),
    Http(u16),
    InvalidManifest(String),
    ChecksumMismatch { expected: String, actual: String },
    UnsupportedSchemaVersion(u8),
    RuntimeNotFound(String),
    Io(String),
}
```

### 5.6 代理解析（复用给 Codex）

新增 `src-tauri/src/proxy.rs`：

```rust
pub fn build_http_client(
    per_provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
    timeout: Duration,
) -> Result<reqwest::blocking::Client, String>;
```

优先级：

```
1. per-provider proxy URL（非空）
2. global config proxy（非 None）
3. system proxy（reqwest 默认读取环境变量）
4. no proxy
```

Codex provider 也改用此函数，保留其 `proxy_url` 作为 per-provider 参数。

### 5.7 本地缓存结构

```
%APPDATA%/QuotaBarWin/providers/remote/<id>/
├── provider.json              // manifest 缓存
├── provider.cjs               // source 缓存（文件名按 manifest.entry 取）
├── provider.cjs.bak           // 上一次版本备份
└── .meta.json                 // ETag, lastCheckAt, checksum, sourceUrl, resolvedRuntime
```

- 更新前写 `.bak`。
- 校验失败时自动回滚到 `.bak`。

### 5.8 自刷新/自更新检测

- **触发时机**：
  - provider 定时刷新前，若 `last_check_at + update_interval_seconds < now`，先 check update。
  - 手动点击 Refresh 时跳过 update check，直接执行本地缓存。
- **检测方式**：
  - 优先 HTTP `ETag` / `Last-Modified`。
  - 无 ETag 时 fallback 到 sha256 内容哈希。
- **更新流程**：
  1. 拉取 manifest。
  2. 校验 schemaVersion / id。
  3. 若 manifest 中的 `runtime` 与配置中不同，重新执行 `resolve_runtime` + `--version/--help` 校验，并更新 `resolved_runtime`。
  4. 若 manifest 中 `checksums.source` 与 `trusted_checksum` 不同：
     - `auto_update=true`：拉取 source → 校验 checksum → 更新缓存和配置。
     - `auto_update=false` 或无 checksum：标记 `update_available`，不替换。
  5. 更新 `.meta.json`。

### 5.9 运行远程 Provider

远程 provider 物化后，按 Script provider 模式执行：

1. 读取本地缓存的 `provider.json` / source 文件。
2. 校验 `schemaVersion`、`id` 与配置一致。
3. 使用 `resolved_runtime`（或重新 resolve）执行 source 文件。
4. 解析 stdout 为 `provider-snapshot-v1`。

**与 Phase 1 的集成点**：

- manifest/source 拉取失败 → 复用 Phase 1 的 stale 降级（使用本地缓存继续运行）。
- source 执行失败 → 复用 Phase 1 的 `ErrorKind` 重试调度。

---

## 6. 安全设计

1. **无域白名单**：允许任意 HTTPS URL，但首次安装弹窗明确展示来源 URL 和 runtime。
2. **Manifest 校验**：`schemaVersion`、`id`、`runtime`、`entry` 必须合法。
3. **Checksum 校验**：auto_update 必须依赖 manifest 声明的 source checksum；校验失败禁止更新。
4. **用户确认**：首次安装必须弹窗确认；展示来源 URL、resolved runtime、所需 env var、auto_update 状态。
5. **Runtime 校验**：安装时执行 `--version` 确认 runtime 可用，拒绝无效命令。
6. **id 冲突提示**：不自动覆盖，由用户选择。
7. **备份回滚**：更新前写 `.bak`，校验失败自动回滚。
8. **脱敏**：错误、日志继续走现有 `redact.rs`。

---

## 7. 需要新增的 Tauri Command

| Command | 作用 |
|---|---|
| `add_remote_provider(url, proxy, auto_update) -> ProviderConfig` | 检测 id 冲突并提示用户选择；拉取 manifest，校验，缓存，返回配置 |
| `remove_remote_provider(id)` | 删除缓存和配置 |
| `refresh_remote_provider(id)` | 强制 check update 并执行 |
| `check_remote_updates() -> Vec<UpdateInfo>` | 检查所有远程 provider 更新 |
| `apply_remote_update(id)` | 手动应用更新 |
| `get_network_proxy() -> Option<ProxyConfig>` | 获取全局代理 |
| `set_network_proxy(config)` | 设置全局代理 |

---

## 8. 实施阶段（远程扩展）

### Phase 2：远程 Provider 拉取 + 代理 + 多 runtime

1. 新增 `ProxyConfig` 全局配置 + `proxy.rs` 统一代理构建。
2. Codex provider 迁移到统一代理函数（向后兼容 `proxy_url`）。
3. 新增 `Remote` provider 配置 + `remote_provider.rs`。
4. 实现 manifest/source 拉取、缓存、checksum 校验、更新检测。
5. 实现 runtime 解析与校验（命令名或绝对路径）。
6. 实现 `add/remove/refresh_remote_provider` command。
7. 前端设置页：全局代理、远程 provider 列表、添加/删除、id 冲突弹窗。

### Phase 3：自更新优化 + 扩展发现

1. 定时 update check 接入刷新循环。
2. 实现 Registry Index 读取和一键安装。
3. 完善首次安装弹窗、权限展示。

---

## 9. 风险与注意事项

1. **GitHub Raw 不稳定**：必须做代理，且默认提示用户配置。
2. **远程脚本执行安全**：依赖来源确认 + checksum + runtime 校验 + 备份回滚。
3. **Runtime 兼容性**：Phase 2 同时支持 node/python/pwsh/bash 等，但需在 Windows PATH 中可找到或提供绝对路径。
4. **版本兼容**：manifest `schemaVersion` 必须匹配，否则拒绝安装。
5. **Tauri allowlist**：新增 command 需在 `tauri.conf.json` 中注册。
6. **配置迁移**：新增 `network_proxy` 和 `Remote` 变体需兼容旧 config。
7. **Phase 1 依赖**：执行链路的 stale 降级和重试调度需要等 ccKimi 的 Phase 1 合并后才能完全接通。

---

## 10. Review 检查清单

- [ ] 不限制域白名单是否可接受？
- [ ] id 冲突提示用户选择是否可接受？
- [ ] 多 runtime 支持纳入 Phase 2 是否可接受？
- [ ] runtime 校验策略（`--version` + 支持绝对路径）是否可接受？
- [ ] 代理优先级是否可接受？
- [ ] checksum 缺失禁用 auto_update 是否可接受？
- [ ] Phase 2 范围是否可接受？
