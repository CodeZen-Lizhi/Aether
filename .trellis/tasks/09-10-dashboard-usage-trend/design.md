# 设计与修改边界

现有仪表盘只展示模型和供应商费用分布。`/api/admin/stats/time-series` 已按时区和粒度返回费用及四种 Token，但只读取保留的原始 usage；`/api/dashboard/daily-stats` 能保留历史总量。因此前端组合两者：以每日费用汇总为准，按请求数判断 Token 明细覆盖。小时明细缺失时不能分配每日总量到小时。

- `api/admin.ts` 为现有时间序列响应补准确类型，保持原接口和参数。
- `views/shared/usageTrend.ts` 负责按天 / ISO 周 / 月对齐总量与时间序列，显式返回缺失值。
- `views/shared/DashboardUsageTrend.vue` 负责五条曲线、双坐标轴、图例和数据明细，使用现有 Card / LineChart。
- `views/shared/Dashboard.vue` 并行加载现有两个统计接口；请求失败独立处理，切换范围使旧结果失效。
- `components/charts/LineChart.vue` 注册 Filler，保持原有默认行为。
- `i18n/messages.ts` 增加必要文案；相关测试验证金额、日期、缺失数据和交互。

图表置于统计周期下方、现有模型与提供商图表上方。沿用应用表面、边框与文字，使用红色费用虚线和四种 Token 曲线；明暗主题可读，图例自然换行。无后端变更、不新增依赖、不通过隐藏内容消除横向溢出。
