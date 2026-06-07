# V6 CommandHub / Multi-Agent Integration Spec

> 目标：把 QuotaBarWin 的 provider runtime 变成 CommandHub / 多 Agent CLI 工作台的额度调度基础设施。

---

## 1. 版本定位

V6 不只是显示 quota，而是让 quota 参与 Agent 启动和任务分配。

---

## 2. 必须实现

### 2.1 Local API

提供本地只读接口，供 CommandHub 查询：

```text
GET /snapshot
GET /providers
GET /provider/:id
GET /recommendation?taskType=large_refactor
```

如果不想引入 HTTP server，可先提供：

```text
quotabarwin-cli snapshot --json
quotabarwin-cli providers --json
quotabarwin-cli recommend --task-type large_refactor --json
```

V6 首选 CLI 方式，避免常驻 HTTP server 带来的安全面。

### 2.2 Agent 绑定

支持配置：

```json
{
  "agentBindings": [
    { "agentId": "ccKimi", "providerId": "kimi-coding" },
    { "agentId": "ccZai", "providerId": "bigmodel-coding-plan" },
    { "agentId": "codex", "providerId": "codex-cli" }
  ]
}
```

### 2.3 Recommendation Engine

根据 quota 状态给出建议：

```ts
type Recommendation = {
  preferredAgentIds: string[];
  avoidAgentIds: string[];
  reason: string;
  riskLevel: "low" | "medium" | "high";
};
```

基础规则：

- urgent provider 绑定的 agent 不推荐。
- critical provider 只适合小任务。
- large_refactor 优先选择 remaining >= 30 的 provider。
- 如果所有 provider 都低，返回 high risk。

### 2.4 UI 增强

设置页新增：

- Agent Bindings。
- Copy snapshot JSON。
- Copy recommendation JSON。

### 2.5 多 Agent 群聊展示约定

提供 badge 数据：

```json
{
  "agentId": "ccKimi",
  "providerId": "kimi-coding",
  "badge": "Kimi 87%",
  "alertLevel": "none"
}
```

---

## 3. 自动验收标准

Codex 必须自己跑：

```bash
npm run build
npm run test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
```

### 3.1 Tests

至少包含：

```text
agent_binding_roundtrip_config
recommendation_avoids_urgent_provider
recommendation_prefers_high_remaining_provider
cli_snapshot_outputs_valid_json
cli_recommendation_outputs_valid_json
badge_generation_uses_bound_provider
```

---

## 4. 完成定义

V6 完成必须满足：

- CommandHub 可以通过 CLI 或只读 API 获取 AppSnapshot。
- Agent binding 可配置。
- Recommendation Engine 有测试。
- badge JSON 可生成。
- 不暴露 token 或原始 command。

---

## 5. 给 Codex 的短 Prompt

```text
目标版本：V6。实现 CommandHub 集成：snapshot CLI、provider/agent binding、recommendation engine、badge JSON 和设置页入口。优先 CLI，不引入常驻 HTTP server。不要暴露 token 或原始敏感 command。
```
