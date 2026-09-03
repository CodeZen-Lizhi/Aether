# Frontend Spec (frontend/)

> **Scope**: `frontend/` — Vue 3 + Pinia + Tailwind 管理界面。后端 Rust 包的规范见各 `aether-*/` 目录。

---

## Convention: i18n 文案机制（中文源文案 + 映射字典）

**What**: 组件模板/script 中**直接写中文**文案，不做 `t('key')` 调用。英文翻译由 `frontend/src/i18n/messages.ts` 的字典在渲染层完成：

- `legacyExactEnglishMessages`：精确匹配中文串 → 英文，如 `'代理节点': 'Proxy nodes'`。
- `legacyDynamicPatterns`：含变量的动态串用 pattern 匹配（注意标点与分隔符需逐字符一致，如全角冒号 `：`、间隔符 `· `）。

**Why**: 新增 UI 文案若不入典，英文语言下会原样显示中文；动态串 pattern 与组件内字符串不一致时静默失配，无报错。

**Example**:

```ts
// 组件中
const text = `测试通过：延迟 ${result.latency_ms}ms · 出口 IP ${result.exit_ip}`

// messages.ts — legacyDynamicPatterns 需覆盖同一字符串的全部组合（字段缺省时分支也要有）
[/^测试通过：延迟 (\d+)ms · 出口 IP (.+)$/, 'Test passed: latency $1ms · exit IP $2']
```

**Checklist（新增文案时）**:

- [ ] 精确串 → `legacyExactEnglishMessages` 加一条；`grep -rn "该文案" frontend/src` 确认无其他使用点遗漏。
- [ ] 动态串 → 每种字段组合一条 pattern，标点逐字符与组件内一致。
- [ ] 删除 UI 文案时，grep 确认字典条目无其他引用后再删。

---

## Convention: 设置卡片（CardSection）保存模式

**What**: 系统设置各分区统一使用 `@/components/layout` 的 `CardSection`：头部 `#actions` 放保存按钮，`:disabled="loading || !hasChanges"`，内容区表单向父级 emit `update:*`，保存由父级统一处理（见 `views/admin/SystemSettings.vue`）。

**Why**: 全部设置卡片共享同一 `hasChanges`/loading 契约；改动单一卡片的按钮行为前先确认不影响该全局模式。

**Example**: `views/admin/system-settings/SiteInfoSection.vue`（最简样例）、`ProxyConfigSection.vue`（含列表 + 弹窗子组件拆分）。

**Related**: 卡片内若存在「两种保存目标」（如卡片设置 vs 子资源 CRUD），头部按钮文案需消歧（如「保存默认代理」），子资源操作走独立弹窗组件。

---

## Convention: 汇总列表单元格不渲染报错文字（失败态静默为横杠）

**What**: 面向汇总的列表列（如供应商列表的余额监控列 `features/providers/components/ProviderBalanceCell.vue`）在查询失败、关联动作失败时只渲染横杠 `-`，不渲染报错 message、不挂 tooltip；具体报错只在交互入口展示（供应商「测试」、用户认证「验证」弹窗）。批量查询返回的失败状态数据仍要保留在缓存里，用于决定渲染 `-`，而不是回落到其他展示分支（如 monthly_quota）。

**Why**: 列表是汇总视图，把上游报错（如「连接失败: xxx」「签到失败」「签到 Cookie 已失效」）直接渲染进单元格会污染整个列表；成功态的短标识（如「已签到」）可以保留。原实现曾刻意把错误 message 打进余额列（注释称"保留全部状态，余额列才能显示认证和上游查询错误"），2026-09 按用户要求移除。

**Checklist（给汇总列表列新增状态展示时）**:

- [ ] 失败/异常态 → `-`；报错文字只出现在弹窗、详情抽屉等交互入口。
- [ ] 新增失败分支时不要把 message 或 title tooltip 挂到列表单元格上。

---

## Convention: 供应商列表排序口径与调度策略对齐

**What**: 供应商管理列表（`views/admin/ProviderManagement.vue`）的排序固定为三级：启用状态（启用在前）→ 调度优先级升序 → `created_at` 升序兜底。优先级只读系统默认分组：`listRoutingGroups()` → `findSystemDefaultRoutingGroup` → `parseSchedulingStrategy(group?.config_json).providerPriorities`（1 起越小越靠前），未配置的供应商用 `Number.MAX_SAFE_INTEGER` 缀尾。排序实现抽在 `features/providers/utils/providerPrioritySort.ts` 纯函数（含单测），桌面表格与移动端卡片共用同一 computed。

**Why**: 优先级不是供应商实体的属性，而是每个路由分组一份的 overlay 配置（`ui_provider_priority` 规则）；列表页没有分组上下文，取系统默认分组口径与调度策略页编辑的是同一份数据，两边顺序才会一致（2026-09 用户明确要求对齐）。

**Example**:

```ts
// features/providers/utils/providerPrioritySort.ts
const leftPriority = providerPriorities[a.id] ?? UNCONFIGURED_PROVIDER_PRIORITY
// UNCONFIGURED_PROVIDER_PRIORITY = Number.MAX_SAFE_INTEGER，与调度策略页 RoutingProfiles.vue 的 fallback 字面量同口径
```

**Related**: 列表页加载时，辅助配置请求（路由分组等）必须独立于列表请求触发（不要放进同一个 Promise.all 连坐 reject），失败静默置空、排序退化为「启用状态 → 创建时间」，不得阻塞供应商列表本身加载。

---

## Convention: 前后端口径对齐——代理 URL scheme 校验

**What**: 前端对 `proxy_url` 的前缀校验为 `/^(https?|socks5h?):\/\//i`；后端权威校验在 `apps/aether-gateway/src/state/proxy.rs`（`url::Url::parse` + scheme 以 `socks` 开头放行）。前端只做前缀级提示性校验，完整解析以后端为准。

**Why**: 避免前端实现完整 URL 语法校验后与后端口径漂移，产生「前端放过、后端拒绝」或反向的体验裂缝。
