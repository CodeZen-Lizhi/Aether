# 验证记录

## 复现与根因

`cargo test -p aether-data-sqlite sqlite_dashboard_daily_stats_preserves_saved_model_provider_breakdown -- --nocapture` 在修复前失败：数据库已保存两个模型 / 供应商分组，读取结果却只有 aggregate。修复后同一测试通过，并覆盖部分分组恢复时原始记录去重。

根因位于 SQLite 统计读取：每日总量被标记为虚构模型和供应商；最大聚合日期截断原始查询，导致该日期之前的真实分组及日期空洞丢失。网关将占位当真实名称，连带污染图表、唯一数量和分组金额。

## 自动检查

- SQLite usage 相关 30 项测试通过，包括原有统计、生命周期、结算快照与本次新增回归。
- Gateway daily 相关 5 项测试通过，包含管理员 / 用户 API 集成，以及总量和维度独立累计、缺失明细、部分历史恢复。
- Dashboard 前端 4 项测试通过，覆盖完整模型名、真实供应商及缺失金额单独展示。
- 前端类型检查、所有修改前端文件 ESLint、Rust 格式化和 git diff --check 通过。
- 自检覆盖 SQL 参数绑定、单一读快照、用户范围、原始与聚合的覆盖关系、日期空洞、实际成本及唯一数量，未发现需阻止交付的问题。

## 交付验收

### 隔离 API 与页面

- 使用独立 SQLite 与合成账号，未访问或修改真实客户端数据。
- 两个已保存历史分组分别返回 `claude-qa-model-two` / `QA Provider Two` 和 `gpt-qa-model-one` / `QA Provider One`，9 次请求、37 Tokens、费用 1.25、实际费用 1.0，唯一模型与供应商均为 2。
- 只有总量的历史日保留 3 次请求、15 Tokens、费用 0.75、实际费用 0.5，并返回 `unattributed_requests = 3`、`unattributed_cost = 0.75`。
- 页面加入当天 24 条合成请求后，总请求 36、总 Tokens 460，费用 2.0005、实际费用 1.5005；历史图例显示完整模型名和真实供应商，缺失部分为灰色“未保留模型明细 / 未保留提供商明细”。
- 浏览器实际查看 1600×1100 与 840×620 窗口。两种尺寸文档与主区域均无横向溢出；小窗口不存在横向滚动容器。
- 已关闭本次浏览器标签页、恢复窗口尺寸并停止隔离前端与网关。

### 安装包

- 使用干净的 `e868096ba8446242893007ae4f67fe52bc1933d0` 源码执行 `npm --prefix apps/aether-desktop run build`，构建成功。
- arm64 / macOS 14+，ad-hoc 签名，未公证。校验签名、系统动态库、包内前端与 `frontend/dist` 一致、修复字符串以及挂载 DMG 内 117 个文件一致。
- 包内网关通过 JSON / SSE 转发、会话续期、配置导出再导入、8 个管理页面，以及 EOF / SIGTERM / 父进程退出的生命周期检查。
- 交付路径：`/Users/zhenglizhi/Downloads/Aether_0.1.0_aarch64_20260910_e868096ba.dmg`。
- 大小：31,535,257 字节。
- SHA256：`da7da3f53be101d8f72ff6fd234eeeeca5552759709cd1609278811320ae96c5`；复制后的文件已重新计算并匹配。

### 两分支同步

- `codex/tauri-macos`：`e868096ba8446242893007ae4f67fe52bc1933d0`。
- `slim-personal`：`5490761aca9f6f1f53178674a4c23d3712baf1fd`。
- 两分支功能提交已 push。9 个相关文件字节一致，`frontend/src/i18n/messages.ts` 保留原有 80 条客户端专属翻译差异；对比本次修改前后的差异内容完全一致。
- 已有无关 Rust warning：`apps/aether-gateway/src/dispatch/refs.rs:71` 的 unreachable pattern；本次未改动该位置。

## 用户追加：移除本月系统健康

- 删除整个月度健康区，包括平均响应、错误率、转移次数与本月费用四张卡片，并清理专用状态、数据赋值和图标导入；今日统计后直接展示统计周期与图表。
- 本次仅修改 `Dashboard.vue`。前端类型检查、该文件 ESLint、现有 Dashboard 4 项测试与 diff 检查通过。
- 隔离浏览器核验 840×620、1024×768、1280×920、1920×1080，月度健康和本月费用均已移除，所有尺寸的文档与主区域无横向溢出、无横向滚动容器；模型与供应商图例、每日表格和总量保持正常。
- 功能提交：macOS `47c0e72b6cc6a3d5f32815fcdf4e8ce7778e85c0`，slim `9469c5c2a`，均已 push。
- 已按上述干净源码重新打包；包内资源不含“本月系统健康”，并包含历史分组修复。签名、arm64、macOS 14+、DMG 与 117 个文件校验通过。
- 网关二进制与先前通过生命周期检查的版本 SHA256 相同，复用既有验证结论。页面仅使用合成数据库，测试服务已关闭。
- 最新交付：`/Users/zhenglizhi/Downloads/Aether_0.1.0_aarch64_20260910_47c0e72b6.dmg`，31,537,045 字节，SHA256 `4b2f2863160dac41192766663274be01dc9002f769a199bb134f7f976f27f4ca`。
