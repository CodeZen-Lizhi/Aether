# 调整用量信息卡单行排版

## Goal

将截图中的模型请求用量信息从多行堆叠调整为紧凑的一行排版，并保持响应式可读性。

## Requirements

- 将请求用量卡中的模型、服务等级、传输状态、费用、时间/速度、Token 等摘要信息改为紧凑的横向信息行。
- 保留现有字段、数值和交互语义，只调整布局与必要的间距/换行规则。
- 在常见桌面宽度下摘要信息保持单行展示；窄屏空间不足时允许自然换行，不出现页面或容器横向滚动。
- 沿用现有前端组件、字体、颜色和响应式样式约定。

## Acceptance Criteria

- [ ] 摘要信息在桌面宽度下呈单行横向排版，字段顺序与现有展示一致。
- [ ] 840px 及更窄窗口下内容仍可读，长文本可换行且 `scrollWidth <= clientWidth`。
- [ ] 现有前端相关检查通过，未引入无关文件改动。

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
