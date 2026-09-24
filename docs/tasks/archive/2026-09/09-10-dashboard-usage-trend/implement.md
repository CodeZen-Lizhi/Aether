# 实施与交付顺序

1. 核对已有 stats、daily-stats、time-series 的时间范围、Token、费用和历史去重口径。
2. 实现累计前端 API、累计 / 今日组件、默认小时趋势与筛选布局、模型和提供商排行及翻译。
3. 更新回归测试，验证累计完整性、今日平均响应、历史缺失、失败重试、切换竞态及排行交互；执行类型检查和修改文件 lint。
4. 使用隔离数据进行真实浏览器验收，独立审查后修复本次问题，整理规格与验证证据。
5. 提交到 codex/tauri-macos，cherry-pick 通用功能到 slim-personal，核对分支差异后 atomic push。
6. 从干净代码构建 macOS 安装包，检查源码资源一致性、签名、架构、DMG 内容及网关；复制校验过的新包到 Downloads。
7. 记录提交和安装包校验值，归档本任务并同步文档提交，移除本任务创建的临时同步工作树。

官方图表配置依据：Chart.js multi-axis、line interpolation 和 Filler 文档，前序已经 find-docs 查询。本次沿用已有 Chart.js、LineChart 与项目样式，不增加图表依赖。
