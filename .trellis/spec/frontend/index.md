# Frontend Spec (frontend/)

> **Scope**: `frontend/` — Vue 3 + Pinia + Tailwind 管理界面。后端 Rust 包的规范见各 `aether-*/` 目录。

外部模型目录的浏览器缓存、失败态、创建预设兜底与严格价格同步见
[External Model Catalog Reliability](../aether-gateway/backend/external-model-catalog.md)。

调度策略页的读写边界见 [Simplified Routing](../aether-routing-core/backend/simplified-routing.md)：保存须传入原分组配置，保留未退役规则的条件、阶段和执行语义；Key 优先级读取实体字段。

桌面启动页 `src/desktop` 的 IPC、校验、状态与原生验收见 [Desktop Host Contract](../aether-desktop/backend/desktop-contract.md)。管理后台继续使用本机 HTTP 同源 API，不获得 Tauri 桌面命令权限。

---

## Convention: 窗口自适应与滚动条

**What**: 管理页、表格、抽屉和弹窗不得依赖横向滚动。`style.css` 全局隐藏滚动条并取消 gutter 占位，保留纵向原生滚动。不能用 `overflow-x: hidden` 裁掉字段来通过验收。`AppShell` 负责页面间距；`PageContainer` 默认不重复加 padding。

**Lists**: 公共 `Table` 使用 `table-fixed`、`w-full` 和 `overflow-wrap:anywhere`，外层不创建横向滚动容器。复杂列表在外层设置 `responsive-list`，内部用 `responsive-list-table` / `responsive-list-cards`、`responsive-list-desktop` / `responsive-list-mobile` 切换。普通容器达到 52rem 才显示表格；`responsive-list--wide` 将门槛提高到 80rem。以内容容器宽度判断，不能只根据 viewport 断点忽略侧边栏。CSS 选择器必须能覆盖 Tailwind 的 `grid` / `inline-flex` 显示类。

**Usage records**: 列宽由可见列权重归一化到 100%，按实际 DOM 列顺序输出 colgroup；th / td 不再独立分配百分比。选择超过 9 列时使用 wide 模式，紧凑卡片仍显示选中的客户端、IP 和完整 User-Agent。金额不能为了塞入表格而被拆成难读的多行。

**Details**: 代码和 JSON 采用 `pre-wrap` / `overflow-wrap:anywhere`，JSON 内容 flex 子项使用 `min-width:0`，视觉缩进最多占内容列 25%。请求头对比的两侧共享同一 grid 行和纵向滚动容器，长短值换行后仍须逐行对齐。链路节点及重试按钮参与流式布局，序号表示换行后的顺序；不得把节点放到横向滚动轨道里。

**Verification**: 在 840×620、1024、1280、1920 px 宽度测量页面及可滚动容器的 `scrollWidth <= clientWidth`；检查所选字段、价格输入、长错误末尾、分页和保存按钮仍可达。使用记录单测验证列宽总和、列顺序和两种布局的完整元数据。真实浏览器核验长短请求头行高、深层 JSON、多个节点及多次重试，不能仅检查 CSS 字符串或把隐藏滚动条当成无溢出证据。

### Convention: 模态弹窗按 WebView 内容区比例缩放

**What**: 居中模态统一使用 `components/ui/dialog/sizing.css` 的 `.app-dialog[data-dialog-size]`。桌面宽度按内容视口比例计算，并受档位最大宽度限制；高度上限为 `min(80dvh, 50rem)`。`Dialog` 的标题和 footer 固定，正文独立纵向滚动；移动端继续使用底部抽屉。独立模态（如 `ReplayDialog.vue`）必须显式导入该样式并复用 `app-dialog`。

**Why**: 固定 `sm:max-w-*` 与接近完整视口的高度会使 APP 缩小时弹窗几乎铺满窗口；仅在调用方添加固定 `vh` 或 `overflow-x-hidden` 会继续造成字段裁切和重复滚动。

**Example**:

```vue
<div class="app-dialog" :data-dialog-size="size">
  <div class="dialog-body">...</div>
</div>
```

复杂表单用 `dialog-grid-2` / `dialog-grid-3` / `dialog-grid-wide`，通过容器查询在窄弹窗内重排；不要恢复固定双栏的 `sm:grid-cols-*`，也不要用 `overflow-x-hidden` 隐藏溢出。

**Verification**: 至少在 900×640、1024×768、1440×900、1920×1080 和 390×844 测量 bounding rect、`scrollWidth <= clientWidth`，并动态缩放已打开弹窗，确认输入和 footer 操作保留。

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

## Convention: 仪表盘布局与统计范围

**Layout**: `Dashboard.vue` 只编排加载和范围状态。`DashboardOverview.vue` 展示全部历史累计与五张今日卡片；`DashboardUsageTrend.vue` 承载周期筛选及主要图表；`DashboardUsageBreakdown.vue` 展示模型和提供商排行。不要恢复平台拆分、请求日志、重复成本图和默认展开的每日大表。保持项目 Card、颜色变量、字体与图标，今日卡片常见桌面宽度五列、窄窗口换行且填满行；概览卡片断点使用 `.dashboard-overview` 容器查询，以实际内容宽度适配侧栏展开后的主区域，不直接依赖 viewport 断点。

**Theme colors**: 当前 Tailwind 颜色配置直接引用完整 `var(--primary)` / `var(--muted)` 色值；不要用 `bg-primary/65` 之类 opacity 后缀，它在真实浏览器里没有有效背景。纯色用 `bg-primary`，条形透明度单独用 `opacity-*`；混色沿用项目的 `color-mix(...)`。用 computed style 和截图确认明暗主题，不只检查类名。

**Lifetime**: `dashboardApi.getLifetimeStats({ timezone, tz_offset_minutes })` 从现有 `/api/dashboard/daily-stats` 读取 `1970-01-01` 至当地今天，累加每日 `requests`、`tokens`、`cost`、`actual_cost`，以首个非零日期作为起始日。每日总量已包含保留的历史缺失部分，不能再次相加 `unattributed_*`。拒绝非法或负数的部分结果；失败独立重试，不能显示假零值。只缓存压缩后的累计对象 60 秒，缓存键包含本地结束日期和时区。接口会返回空日，不能把几十年的行常驻前端缓存。

**Sources**: `/api/admin/usage/stats` 默认窗口和原始 usage 范围不是全部历史；`/api/dashboard/stats` 的 `tokens.month` 在 SQLite 有聚合行时不能保证包含未聚合原始记录。累计应沿用 daily-stats 已修复的保留总量与原始记录去重。今日、累计与筛选周期互相独立，切换趋势不重新解释顶部指标。

**Rankings**: 使用同接口的 `model_summary` / `provider_summary` 周期汇总，不能再次累加每日已舍入的费用，否则小额请求的排行可能错误。默认按 Token 降序，可切费用 / 请求；各组默认显示前五项，分别展开全部。真实名称用跳过 legacy i18n 的 `samp` 文本保留原文与完整长度，费用 / 请求缺失单独标注，不能创造名为 `aggregate` 的业务分组。优先使用服务端 `unattributed_requests` / `unattributed_cost`，不能把舍入后的总额和分项相减冒充历史缺失。费用占比使用本组周期总额加保留缺口，Token / 请求占比仍使用包含历史的每日总量。

## Convention: 仪表盘使用趋势的数据口径

**What**: `DashboardUsageTrend.vue` 使用现有 `/api/admin/stats/time-series` 的费用、输入、输出、缓存创建与缓存命中数据；`usageTrend.ts` 结合 `/api/dashboard/daily-stats` 的历史总量。仅改前端时不能假设时间序列接口包含已清理的请求明细。

**History**: 按天、ISO 周、月对齐每日总量，以保留的每日费用汇总为准。总量请求数大于原始时间序列请求数时，Token 曲线使用 `null`，不能填 0 或连接缺口；小时视图不能把历史每日费用分摊到小时。原始输入 Token 在部分上游格式中已包含缓存，沿用原接口口径，并在明细中说明各项不能直接相加。缓存命中率必须使用时间序列接口按 API 格式归一化后返回的 `total_input_context` 作为分母，不能在前端用输入、缓存创建和缓存命中重新相加。

**Time and interaction**: 默认今天、按小时。时间序列的小时标签虽然携带 `+00:00`，实际已转换为请求时区；展示直接使用标签的本地日期和时分，不能再次转时区。统计周期切换立即使旧请求失效，两个现有接口并行加载且失败不连带清空其他成功数据。`TimeRangePicker` 需显式传入 `:show-granularity="true"` 才显示小时 / 天 / 周 / 月控件。

**Today response**: 顶部 `/api/dashboard/stats` 固定传 `preset=today` 和本地时区偏移，从 `system_health.avg_response_time` 读取已按请求样本计算的平均秒数。四张服务端卡片后追加“今日平均响应”，统一显示两位小数和 `s`；不读取下方趋势范围的平均值，也不按模型平均再平均。今日没有请求显示 `--`，缺失或非法数值显示无响应数据。加载失败与空数据要区分，骨架 / 空态也保持五张卡片。不要恢复月度系统健康区。

**Chart**: 费用虚线使用独立美元轴，四种 Token 使用另一坐标轴，仅缓存命中填充面积。HTML 图例提供键盘按钮与 `aria-pressed`，明细通过折叠表格 / 窄屏卡片展示。复用 `LineChart`，面积填充需注册 Chart.js `Filler`；禁止为此额外引入图表库。

**Verification**: `usageTrend.spec.ts` 覆盖历史缺失、跨年 ISO 周 / 月、小时日期与总量；Dashboard 测试覆盖图例、失败重试和旧范围响应失效。浏览器检查明暗主题、中英文、小时范围，以及展开明细后的四种窗口尺寸。

---

## Convention: 系统设置分类与分组保存

**What**: `views/admin/SystemSettings.vue` 使用常用首页 `general` 和两个二级页 `proxies`、`advanced`。首页只有网关与数据备份；代理管理一跳可达，高级页按需展开六组。已访问视图保留挂载和草稿。可编辑配置由 `useSystemConfig` 按 proxy/basic/log/cleanup 四组维护快照，行内保存只提交该组变更字段，`Promise.allSettled` 逐字段更新成功基线，失败字段保留输入供重试。请求兼容与密钥规则视觉分组，但仍共享 basic 保存边界。

**Why**: 设置页需要按任务查找，而保存边界仍必须独立。整组提交或分类卸载会误覆盖其他值、丢失草稿，局部失败也不能显示为全部成功。

**Contract**: 清理批次键 `cleanup_batch_size`、`request_candidates_cleanup_batch_size`、`proxy_node_metrics_cleanup_batch_size` 可以继续从 API 读取，但不属于任何前端编辑组，普通保存不得写回。`compressed_log_retention_days` 是内容删除期限；`detail_log_retention_days` 只表示开始压缩的时间点。

**Navigation**: 旧 query `connection -> general`、`records -> advanced/记录与清理`、`backup -> general/数据备份`；旧 `#section-*` hash 映射到所属视图、打开必要折叠层并滚动。外部 query 使用 `Map` 或自有键校验，`constructor`、`toString`、`__proto__` 等未知值必须回到首页，不能落入空白视图。二级页提供返回入口，不增加页内侧栏或分类下拉框。主题/语言仍由全局顶部快捷切换。

**Layout**: 原生最小窗口为 900x640。桌面范围始终保持左标签、右控件；正文最大 960px 居中，行使用 `minmax(0, 1fr) minmax(280px, 320px)`，只让留白随内容宽度变化。标题 24px、控件文字 13px，不随视口缩小字体；不得恢复旧 760px 内容断点的结构切换。只有小于 600px 的 Web 视口允许设置行上下排列。Web 不显示桌面端口、自启动和 IPC 操作。

**Ownership**: `ProxyConfigSection` 的 selector/management 视图仅在管理页挂载节点编辑弹窗；`DataManagementSection` 的 backup/migration 视图各自拥有对应 file input，根页共享导入预览和恢复状态。桌面端口直接可编辑，保存并重启沿用原命令与 recovery 路径，自启动沿用即时保存。

**Verification**: 覆盖三种视图、旧 query/hash、未知 query、前进/后退、切换保留草稿、保存后刷新、局部成功和隐藏批次值保持；在 900x640、1024x768、1200x820、1440x900、1920x1080 检查溢出和连续缩放的列关系，另测 390px Web。明暗主题、中英文和长代理名称均应可读。原生最大化及实际 IPC 必须与浏览器/IPC fixture 的证据区分。

---

## Convention: 汇总列表单元格不渲染报错文字（失败态静默为横杠）

**What**: 面向汇总的列表列（如供应商列表的余额监控列 `features/providers/components/ProviderBalanceCell.vue`）在查询失败、关联动作失败时只渲染横杠 `-`，不渲染报错 message、不挂 tooltip；具体报错只在交互入口展示（供应商「测试」、用户认证「验证」弹窗）。批量查询返回的失败状态数据仍要保留在缓存里，用于决定渲染 `-`，而不是回落到其他展示分支（如 monthly_quota）。

**Why**: 列表是汇总视图，把上游报错（如「连接失败: xxx」「签到失败」「签到 Cookie 已失效」）直接渲染进单元格会污染整个列表；成功态的短标识（如「已签到」）可以保留。原实现曾刻意把错误 message 打进余额列（注释称"保留全部状态，余额列才能显示认证和上游查询错误"），2026-09 按用户要求移除。

**Checklist（给汇总列表列新增状态展示时）**:

- [ ] 失败/异常态 → `-`；报错文字只出现在弹窗、详情抽屉等交互入口。
- [ ] 新增失败分支时不要把 message 或 title tooltip 挂到列表单元格上。

---

## Convention: 模型查询和批量映射错误保留完整详情

**What**: `utils/errorParser.ts` 的 `parseUpstreamModelError(error: string): string` 可以为常见状态附加中文提示，但必须保留接口返回的完整原文；不按 80 / 100 字符截断，也不按分号只取第一个 Key 或格式的错误。`useUpstreamModelsCache` 合并 `data.error` 和 `data.warning`，避免其中一项覆盖另一项。

**Why**: 批量映射弹窗曾只显示 `error sending request for ur...`，后续 Key 的失败和 HTTP 响应详情被前端丢弃，无法排查真实原因。

**UI contract**: `BatchModelMappingDialog.vue` 使用可翻译的反馈标题和独立的原始详情。详情通过 `<pre class="whitespace-pre-wrap [overflow-wrap:anywhere]">` 的文本插值呈现，保留换行并使长 URL 换行；`pre` 会避开 legacy i18n 的标点与空白改写，不使用 `v-html`，不绕过已有脱敏。部分查询失败的详情留在弹窗中；批量保存按客户端模型列出所有失败原因，全部成功后自动关闭。

**Verification**: `errorParser.spec.ts` 验证长错误、多 Key、HTTP 状态 / 响应及请求失败原文完整保留；`useUpstreamModelsCache.spec.ts` 验证 error / warning 同时存在；`BatchModelMappingDialog.spec.ts` 验证详情末尾、所有失败模型以及重试成功关闭。修改时同时检查窄窗口无水平溢出。

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

## Convention: 代理节点连通性测试

**What**: 代理节点列表的快捷测试调用已保存节点测试接口，编辑弹窗的“测试连接”调用未保存表单测试接口；两者共享成功结果格式化逻辑。

**Contract**: 列表测试使用 `proxyNodesApi.testProxyNode(nodeId)`，请求 `POST /api/admin/proxy-nodes/:id/test`，不在前端读取或重新发送已保存密码。编辑测试使用 `proxyNodesApi.testProxyUrl({ proxy_url, username?, password? })`，仅发送当前表单值。

**UI state**: 列表测试状态按节点 ID 隔离；同一节点测试中禁止重复请求，其他节点仍可测试。成功提示按 `latency_ms`、`exit_ip` 的缺省组合生成，失败和请求异常通过现有 toast/表单错误入口反馈。

**Reuse**: 成功文案统一使用 `views/admin/system-settings/proxyTest.ts` 的 `formatProxyTestSuccessText`，避免列表与编辑弹窗在字段缺省时产生不同结果。

**Verification**: 测试列表成功、字段缺省、服务端失败、请求拒绝、同节点去重和跨节点并发；编辑弹窗继续验证保存前测试与字段变更后的结果清理。
