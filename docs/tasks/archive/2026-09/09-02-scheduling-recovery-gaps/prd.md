# 调度残留缺口补丁：传输层失败接入健康/熔断 + 手动恢复全量清理

## Goal

关闭 Bug1 残留（sync/stream 传输层 Err 不触发健康投影与熔断）与 Bug2 残留（管理面恢复健康不清熔断 JSON/Codex 配额熔断 KV/会话亲和/限流冷却）

## Requirements

- TBD

## Acceptance Criteria

- [ ] TBD

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
