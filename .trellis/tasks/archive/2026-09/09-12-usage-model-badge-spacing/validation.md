# 验证记录

## 原因与修复

旧实现将名称和徽标作为两个 flex 项；名称受列宽限制换行后，span 盒内仍有未绘制空白。之前的 w-fit 仅限制外层宽度，没有让徽标跟随文字末尾。属于隐含布局假设与浏览器验证缺口，单测中的 class 断言无法发现。

本次改为同一行内文字流，徽标左距 4px，空间不足自然续行；映射名在 DOM 中置后并独立下行，原有三徽标堆叠与所有业务文案保持。新增真实几何检查，使用文字 Range，不以容器盒宽或 CSS class 作为间距证据。

## 浏览器证据

- 真实组件 fixture 的旧代码：116px 模型列中 gpt-6-astra + high 距末行文字 43.13px；104px 为 31.13px。几何脚本明确失败。
- 修复后 94/104/116/128/160/240px × 表格/卡片/详情字体 × high/medium/长名称/映射/堆叠，共 90 个组合通过，无横向溢出。
- 主会话真实 `/admin/usage`，仅记录接口返回合成行：900px 窗口下旧代码 high 距末行文字 31.61px；修复后 900/1024/1200/1440/1920/900 连续缩放通过，同一行间距 4px，窄列自然续行。390px 真实卡片的三条记录均为 4px，无横向溢出。
- 组件回归：49 项相关测试通过；type-check、定向 ESLint、git diff --check 通过。
- 检查脚本 `frontend/scripts/check-usage-model-spacing.cjs`；fixture `frontend/scripts/fixtures/usage-model-spacing.html`。使用 5175 既有隔离 Vite 服务，未更改个人使用记录。
- 修复前后实际表格截图：`output/playwright/badge-spacing-app-before-900.png`、`badge-spacing-app-after-900.png`、`badge-spacing-app-after-1920.png`；卡片截图 `badge-spacing-app-card-390.png`。组件完整矩阵截图位于 `output/badge-spacing/`。

## 边界

复现与视觉检查使用真实前端、字体和浏览器，记录为合成数据。未替换或启动原生 APP，实际 WebKit 视觉验收不在这些证据范围内。

## 独立审查与打包

- 独立 trellis-check 未发现产品组件业务语义回归。修复几何脚本等待隐藏卡片元素导致超时的问题，使用 `:visible`；补充自然续行的最大垂直距离判断，避免任意下移也被判为相邻。真实页面 3 行和 fixture 90 组通过，故意下移负例正确失败；定向 ESLint 与 diff check 通过。
- 执行桌面默认构建脚本，自动分配版本 0.1.20，生成 `target/release/bundle/dmg/Aether_0.1.20_aarch64.dmg` 和随包 APP。包中短版本与构建版本均为 0.1.20，宿主/网关均为 arm64，最低 macOS 14.0。
- 构建 APP 与 DMG 内 APP 的深度严格签名检查通过；DMG 校验通过；只读挂载后确认 APP、Applications 链接，文件与构建 APP 一致。随包 Web 与 frontend/dist 逐文件一致。动态依赖仅系统框架和 /usr/lib。签名为本地 ad-hoc，未公证。
- SHA256：`676cf10a0313157069444c799d7a6708e45062a4da6c00a40dd94a096e3c9ed4`。
- 本次随包 release 网关执行桌面模式 qa_lifecycle 通过：JSON/SSE、会话续期、配置导入导出、8 个管理路由、用量持久化、stdin EOF、SIGTERM 与父进程退出清理。隔离诊断目录为临时目录下 `aether-cli-lifecycle-3uldysva`，子进程已清理，用户 APP 未操作。
