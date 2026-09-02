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

## Convention: 前后端口径对齐——代理 URL scheme 校验

**What**: 前端对 `proxy_url` 的前缀校验为 `/^(https?|socks5h?):\/\//i`；后端权威校验在 `apps/aether-gateway/src/state/proxy.rs`（`url::Url::parse` + scheme 以 `socks` 开头放行）。前端只做前缀级提示性校验，完整解析以后端为准。

**Why**: 避免前端实现完整 URL 语法校验后与后端口径漂移，产生「前端放过、后端拒绝」或反向的体验裂缝。
