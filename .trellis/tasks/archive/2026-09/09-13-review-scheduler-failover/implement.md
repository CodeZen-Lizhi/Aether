# 实施与验收计划

状态：本地实现、行为验收与最终 lint 完成，尚未提交/部署。[PRD](prd.md) 定义需求，[设计](design.md) 定义契约。

## 阶段与依赖

保留一个任务，因为配置、执行和状态构成同一行为链；阶段不等于可单独宣布完成的产品。后续若拆子任务，依赖和契约必须写进子任务产物。

| 阶段 | 交付 | 前置 |
|---|---|---|
| P0 | 实际配置/适配路径、迁移样本、最小夹具和字段契约 | 用户整体批准 |
| P1 | 统一分类、健康公式、幂等仓储、熔断与探测 | P0 |
| P2 | 尝试配置迁移、退避冷却、共享首输出预算 | P1 |
| P3 | 三种排序/亲和与 HTTP/SSE/WS 终态贯通 | P1、P2 |
| P4 | 管理读写展示、真实入口验收和检查 | P1-P3 |

实施/检查使用 Trellis 子 agent，优先原生注入本版 PRD/design/research，缺失再由子 agent 加载。共享契约先单 owner 完成；稳定后前端与独立测试夹具可并行，不让多个 agent 同改 effects/candidate_loop。主会话负责整合与范围判断。

## 实施清单

- [x] P0：批准后读取 trellis-before-dev 和实际包规范，核对工作树及已验证代码入口，保护用户改动。
- [x] P0：列出匿名配置样本，包括 failover_rules 显式值、endpoint/provider 的 0/1/2/>2/null、继承和新旧冲突；迁移不导出凭据。共享 resolver 和 admin 的针对性回归已覆盖，见 research/config-contract.md。
- [x] P0：核对现有结算记录是否可事务去重，固定 attempt identity、凭据/熔断 epoch、存储版本和 API 契约；若改变业务边界则返回规划。详见 research/storage-contract.md。
- [x] P1：实现一次规范化错误事实，定点健康与完整成功，覆盖直接零/中性/混合失败；保留用户终止规则。分类/评分模块回归已通过；各协议真实入口仍在 P4 验收。
- [x] P1：原子健康与幂等结算，memory/SQLite 一致；缓存失效、旧代次保护、CAS 重投；成功节流不丢回血。
- [x] P1：归零过滤排队尝试，移除新聊天策略旧 8 次/低成功率额外触发和快速满血；探测单租约、续期和取消回收。
- [x] P2：总尝试规范化及旧 2 幂等迁移，保留有效覆盖；展示实际生效来源，非聊天隔离。
- [x] P2：退避、完整 Retry-After、无提示冷却与累计 2 秒；等待释放准入资源，执行前再检查。
- [x] P2：逻辑请求共享 90 秒预算接入静态/动态/流式路径，不重置、不重复计分，不误改非流式/compact。
- [x] P3：固定人工顺序、缓存备用成功迁移、成本倍率优先与同价亲和；读取/写入同时贯通。
- [x] P3：同步/SSE/WS 分别接终态，区分有效输出和重放门；pinned continuation 不跨 K。
- [x] P4：修改必要表单和状态展示，生效次数、总预算、调度健康/成功率区分，i18n 和布局沿用既有规范。
- [x] P4：完成下表真实入口验收并由 check agent 复核，修复本次问题，记录未验证范围。

## 主要入口

| 范围 | 文件 |
|---|---|
| 健康/分类/效果/尝试配置 | apps/aether-gateway/src/orchestration/{health,classifier,effects,attempt}.rs |
| 排序/过滤 | crates/aether-scheduler-core/src/{health.rs,ranking/modes.rs,candidate/selectability.rs} |
| 候选计划 | apps/aether-gateway/src/ai_serving/planner/{candidate_materialization,candidate_ranking}.rs |
| 执行循环 | crates/aether-ai/serving/src/attempt_loop.rs；apps/aether-gateway/src/executor/candidate_loop.rs |
| 传输/流式 | apps/aether-gateway/src/execution_runtime/{transport.rs,stream/execution.rs,stream_pump.rs} |
| WS | apps/aether-gateway/src/handlers/proxy/websocket/responses/ |
| 仓储 | apps/aether-gateway/src/state/catalog.rs；crates/aether-data/contracts/src/repository/provider_catalog/types.rs；crates/aether-data/runtime/src/repository/provider_catalog/memory.rs；crates/aether-data/adapters/sqlite/src/provider_catalog.rs |
| 管理与页面 | crates/aether-admin/src/provider/；frontend/src/features/providers/；frontend/src/features/routing/ |

仓储结构修改围绕原子结算和兼容契约，用既有迁移体系，不新建服务。风险集中在多层重试、格式跳过范围、旧默认遮蔽、异步重复结算、缓存失效和提交后重放。

## 真实入口验收

使用两个脚本可控本地假上游和隔离内存仓储，走真实鉴权网关路由；管理 API 保存/读取另测，SQLite 使用独立临时数据库验证事务、重开、迁移与导出。没有把这几层的测试包装成一次端到端 SQLite UI 流程。记录匿名目标、次数/顺序/时间，不调用付费模型。

| 场景 | 证据 |
|---|---|
| K1 快 500 两次、K2 成功 | K1/K1/K2，K1 健康 10→8→6，K2 不给 K1 回血，后续按模式正确选择 |
| 轻/重/混合失败夹成功 | A3 序列，成功清计数，中性取消/过滤不改变计数 |
| 本地满 vs 上游满 | 本地零上游请求/零扣分，上游真实失败计分/冷却 |
| Retry-After 短/长/日期/非法 | 累计 2 秒边界，长值转备用且原 K 全期限不被绕过 |
| 归零/探测 | 已排队不执行，到期单探测，长探测续租，取消释放，失败间隔翻倍/成功 1 分 |
| 重复/并发/迟到结算 | 同 attempt 只一次，CAS 冲突不丢分，旧凭据/熔断/亲和结果不污染新状态 |
| 总预算 | fake clock 验证截止；真实入口缩短配置验证跨 K 不重置；默认解析 90000 ms；有效输出后能越过初始截止继续完成 |
| 异常 200/心跳/断流/工具片段 | 不伪成功、不延长预算；预提交可转移，已提交不重发/拼接 |
| WS/状态引用 | 可安全重放的独立轮次可备用，每轮预算独立；有状态只原绑定或明确失败，不假设客户端重建 |
| 配置与界面 | 旧 2 真正两次，显式覆盖不变，重复迁移幂等，保存/刷新与实际值一致 |
| 兼容 | 非流式/compact 原时限、非聊天健康/重试、条件路由/请求修改/终止规则保持 |

按 PRD A1-A11 逐项关联证据；时序优先 fake clock，不让每个用例实际等待数分钟。短时限真实测试必须使用生产配置路径，不能测试特判绕过调度/持久化。

## 针对性检查

下列为采用的针对性检查范围；实际通过结果、修复后的重跑与未验证边界见 research/verification-log.md。新增回归放在对应模块，不以测试总数代替行为证据。

```sh
cargo test -p aether-scheduler-core --lib health::
cargo test -p aether-scheduler-core --lib ranking::
cargo test -p aether-ai-serving --lib attempt_loop::
cargo test -p aether-gateway --lib orchestration::
cargo test -p aether-gateway --lib executor::candidate_loop::
cargo test -p aether-gateway --lib handlers::proxy::websocket::responses::
```

SQLite/memory 事务、迁移回归核对 package name 后定向执行，不默认全仓测试。模块测试不能替代真实路由 A1/A2/A8/A9。前端在 frontend/ 定向运行 `npm run test:run -- <涉及测试文件>`、`npm run type-check`，eslint 仅改动文件，不用会全量修复的 npm run lint。

UI 改动后启动本地服务，完成“改次数/预算→保存→刷新→本地聊天请求→查看状态”流程，在桌面/窄 Web 检查控件可达、无裁切。端口占用则换空闲端口，交付 URL。模拟客户端与真实 Codex APP 验证分开记录，真实中转未测试时明确标明。

## 收尾与回退

- [x] check agent 审查跨层契约、真实次数/顺序、健康事务、状态刷新和重放边界，避免与 implement agent 同改文件。
- [x] 每项验收给实际结论或未验证原因，不用编译成功替代功能正确。
- [x] 只将已实现且验证的稳定契约写入 spec，不把规划当现状。
- [x] 交付迁移影响、测试和剩余风险；提交/push/发布仍需明确授权。

回退必须停止新策略写入并恢复匹配版本的配置/健康，不能旧新算法混写；保留结算证据。用户已整体批准并进入实施；验证仅调用本地模拟上游，不调用付费模型。
