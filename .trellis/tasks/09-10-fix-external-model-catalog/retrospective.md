# Bug Analysis: 外部模型目录失败被误呈现为空目录

## 1. Root Cause Category

- **Category**: B / E — Cross-Layer Contract + Implicit Assumption
- **Specific Cause**: 网关把缓存未命中后的单次外部请求失败转成 503，创建弹窗又吞掉该错误并按空数组渲染；同时创建预设与价格同步共用了未声明新鲜度的目录读取语义。

## 2. Why Earlier Behavior Failed

1. 只依赖 15 分钟缓存：缓存命中时隐藏了外部依赖脆弱性，缓存未命中时没有恢复能力。
2. 组件记录日志后继续渲染：把“请求失败”错误降级成了“请求成功但没有数据”的空态。
3. 强制刷新先清浏览器缓存：外部请求失败后既不能同步，也丢失了创建预设可安全使用的最后成功结果。

## 3. Prevention Mechanisms

| Priority | Mechanism | Specific Action | Status |
| --- | --- | --- | --- |
| P0 | Architecture | 目录读取显式返回 `fresh/stale/error` 语义；严格刷新单独入口 | DONE |
| P0 | Runtime | 网关只对明确瞬时错误与 5xx 做三次有界尝试 | DONE |
| P0 | Tests | 覆盖重试成功/耗尽/4xx、旧缓存兜底、错误态和同步失败不更新 | DONE |
| P1 | Documentation | 固化跨层新鲜度与错误矩阵 | DONE |

## 4. Systematic Expansion

- **Similar Issues**: 其他同时服务“浏览查看”和“写入/同步”的远端目录也可能错误复用同一缓存策略。
- **Design Improvement**: API adapter 应拥有缓存与新鲜度语义，组件只消费带状态的结果。
- **Process Improvement**: 跨层验收必须分别模拟成功空数据、传输失败、过期缓存与恢复重试。

## 5. Knowledge Capture

- [x] 新增外部模型目录可靠性 code-spec。
- [x] 在 cross-layer guide 增加浏览与变更新鲜度分离检查。
- [x] 回归测试和浏览器三态验证已完成。
