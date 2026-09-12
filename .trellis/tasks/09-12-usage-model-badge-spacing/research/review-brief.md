Active task: .trellis/tasks/09-12-usage-model-badge-spacing

你是本任务的 trellis-check，不再派发其他代理。检查 check.jsonl、PRD，以及本次 5 个前端文件：共享 UsageModelDisplay.vue、UsageRecordsTable.spec.ts、RequestDetailDrawer.pricing.spec.ts、scripts/check-usage-model-spacing.cjs、scripts/fixtures/usage-model-spacing.html。

问题：gpt-6-astra 换行后 high 与可见文字间距过大；旧 w-fit flex 仍按收缩前文本盒宽度排徽标。实现改为普通行内文本流，徽标 ml-1；actual model 调整到 DOM 尾部独立 block，3 标签堆叠规则保持。模型名称、推理档位业务语义不变。

复用证据：实现代理真实组件 90 个组合（94/104/116/128/160/240px，表格/卡片/详情字体，high/medium/长名称/映射/堆叠）通过。旧代码 116px 文字距离 43.13px，脚本已先红后绿。49 项相关测试、type-check、定向 ESLint、diff check 通过。主会话真实 /admin/usage 页面拦截合成行，900px 重现 31.61px 间距；修复后 900/1024/1200/1440/1920 与390卡片通过，同一行 4px 或自然续行。

聚焦检查语义无回归与几何测试是否有漏洞。已知脚本声称可跑真实页面，但第7行 first() 会取隐藏的卡片模型，在实际表格页面等待超时，请改为可见选择器，并运行最小验证（可使用主会话 settings-stable，但勿 reload/goto 以免丢失合成API拦截；主会话接下来只读spec）。还有 badge-spacing 会话可跑 fixture。

允许小范围修复测试/代码；不碰 spec、任务文档、提交或打包，不重复全套检查。返回发现、修复和验证结论，原生 APP 未验证继续明确。
