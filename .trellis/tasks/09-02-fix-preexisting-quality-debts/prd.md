# 修复预存质量债：3个失败测试 + 前端 type-check 失效

## Goal

A) 修复/同步 3 个预存失败测试（codex 映射复用×2、claude 流式重写器×1），恢复 cargo test --workspace 全绿；B) 清理前端 ~358 个预存类型错误并将 type-check 脚本改为 vue-tsc -b，使类型检查真正生效

## Requirements

- TBD

## Acceptance Criteria

- [ ] TBD

## Notes

- Keep `prd.md` focused on requirements, constraints, and acceptance criteria.
- Lightweight tasks can remain PRD-only.
- For complex tasks, add `design.md` for technical design and `implement.md` for execution planning before `task.py start`.
