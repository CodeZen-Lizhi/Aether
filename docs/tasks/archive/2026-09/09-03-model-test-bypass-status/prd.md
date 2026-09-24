# 模型测试绕过供应商与密钥开关校验

## Goal

模型测试作为管理端诊断通道：不再校验供应商/端点/Key 的启用状态与健康状态，只要求 Key 存在
且与端点格式兼容，有 Key 必定发起真实上游请求；正式转发链路行为不变。

## 背景

管理端「模型测试」（`/api/admin/provider-query/test-model` 及 `test-model-failover`）是诊断通道：
用户配置好供应商后常需要先测试连通性，再决定是否启用供应商/Key。但当前实现把「运行态开关」
也当成了测试前置条件，导致：

- 供应商处于关闭状态 → 每个候选在执行前被 `provider_inactive` 跳过，测试必然失败；
- 所有 Key 处于关闭状态 → 候选构建阶段直接过滤掉全部 Key，返回 404
  “No active endpoint or API key found”，测试无法发起。

这在「先配置、先验证、后启用」的正常操作顺序下是反的。用户明确要求：
**模型测试只关心“有没有 Key”；有 Key 就一定发起真实请求，不管供应商/Key 的开关与健康状态。**

## 现状定位（调查结论，实现时直接对照）

阻塞点共三处，全部在 admin 测试路径及其复用的 transport policy 内：

1. **Key 候选过滤**（必须改）
   `apps/aether-gateway/src/handlers/admin/provider/query/models/model_test.rs:2055-2057`
   候选构建时 `.filter(|key| key.is_active)` 把禁用 Key 全部排除；全部 Key 关闭时
   `candidates.is_empty()` → 404 `ADMIN_PROVIDER_QUERY_NO_ACTIVE_TEST_CANDIDATE_DETAIL`。

2. **执行前 transport 支持检查中的状态检查**（必须绕过）
   `model_test.rs` 执行路径调用 `provider_query_transport_supports_model_test_execution`
   （`models/model_test/adapter.rs:201-249`），Standard/Grok 适配器最终落到
   `crates/aether-provider/transport/src/policy.rs:48-56`（openai:chat）与
   `policy.rs:121-128`（standard/gemini 同格式），provider/endpoint/key 任一
   `is_active=false` 即返回 `provider_inactive`/`endpoint_inactive`/`key_inactive` 跳过。
   `adapter.rs:85-93` 的 `provider_query_grok_test_unsupported_reason` 也直接检查了三个
   is_active（该函数为测试独有，可直接删除这三项检查）。
   **注意**：`policy.rs` 的这几个函数同时被正式转发链路
   （`apps/aether-gateway/src/ai_serving/planner/...`）复用，用于拒绝关闭的供应商/Key，
   **不得修改 policy.rs 的现有语义**；只能在测试路径绕过。

3. **端点选择只认活动端点**（需加回退）
   `model_test.rs:871-931` `provider_query_select_preferred_non_kiro_endpoint` 与
   `model_test.rs:1903-1923` `provider_query_select_test_endpoint` 均只选
   `endpoint.is_active` 的端点。供应商关闭场景下端点通常仍开着，但为满足“有 Key 就能测”，
   需要无活动端点时回退到任意端点。

已符合预期、无需改动的部分：

- 健康状态/熔断：`provider_query_test_key_sort_key`（`model_test.rs:978-1015`）仅用于
  候选排序，不拦截，保持现状（健康 Key 仍优先被测试）。

## Requirements

1. 不校验供应商是否启用（`provider.is_active` 不作为测试前置条件）。
2. 不校验 Key 是否启用（`key.is_active` 不作为候选过滤条件，也不作为执行跳过原因）。
3. 不校验 Key 健康状态与熔断状态（现状已满足，保持仅排序语义）。
4. 只校验“存在可用 Key”：
   - 供应商下没有任何 Key（或没有与所选端点格式兼容的 Key）→ 明确报错，不发起请求；
   - 存在至少一个格式兼容的 Key → 无论其状态，全部进入候选并真实发起上游请求。
5. 端点：优先选择活动端点；若供应商下没有任何活动端点，回退到任意端点，保证仍可测试。
6. 端点自身关闭（`endpoint.is_active=false`）同样不阻止测试（按第 5 条回退逻辑覆盖）。

### 保留的校验（非状态类，不得放开）

- Key 与端点的格式兼容判断 `provider_query_key_supports_endpoint`；
- Key 的 `allowed_models` 模型白名单（`provider_query_key_allows_effective_test_model`）；
- 指定 `selected_key_ids` 时仍要求这些 Key 存在；
- OAuth Key 等本地传输真实不支持的能力限制（如 `transport_oauth_resolution_unsupported`）、
  header/body 规则不兼容等——这些是“能不能发请求”的真实约束，与开关状态无关。

### 实现约束

- 只改 admin 测试路径（`model_test.rs`、`models/model_test/adapter.rs` 及其测试）。
  推荐做法：在测试路径内对 transport 快照克隆后强制 `provider/endpoint/key.is_active = true`
  再调用现有 policy 能力检查（最小侵入、不影响正式链路）；或等价的测试专用包装。
  禁止修改 `crates/aether-provider/transport/src/policy.rs` 的现有函数语义。
- 无候选时的错误信息从 “No active endpoint or API key found” 调整为“没有可用的 API Key”
  语义（常量 `ADMIN_PROVIDER_QUERY_NO_ACTIVE_TEST_CANDIDATE_DETAIL` 及相关文案，
  英文措辞可自定；前端 `errorParser.ts` 若有匹配需同步检查）。
- 候选排序逻辑保持不变（健康/熔断仍只影响顺序）。

## Acceptance Criteria

- [x] 供应商关闭 + 至少一个 Key（无论开关）→ 模型测试真实发起请求并返回上游结果。
- [x] 所有 Key 关闭但存在 → 模型测试真实发起请求。
- [x] 供应商与所有 Key 均开启 → 行为与现状一致（回归不破坏）。
- [x] 供应商下没有任何格式兼容的 Key → 返回明确错误，不发起请求。
- [x] 无任何活动端点但有 Key → 回退端点后仍可测试。
- [x] 正式转发链路不受影响：`crates/aether-provider/transport/src/policy.rs` 无语义变更，
      `ai_serving/planner` 相关测试全部通过。
- [x] `cargo test -p aether-gateway`（至少 model_test 相关模块：
      `handlers/admin/provider/query/models/model_test/tests.rs`、
      `control/tests/admin_provider_query.rs`、`tests/control/admin/provider_query.rs`）通过，
      受影响断言同步更新；`cargo test -p aether-provider-transport`（若涉及）通过。

## 范围外

- 不改前端测试入口的交互（按钮不因状态禁用，现状已如此）。
- 不改 failover 模拟之外的测试语义（failover 走同一候选构建函数，自动受益）。
- 不涉及正式代理链路对关闭供应商/Key 的拒绝行为。
