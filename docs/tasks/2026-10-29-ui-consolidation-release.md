# 本轮 UI 收敛发布记录

日期：2026-10-29

## 范围

- 使用记录：移除请求类型筛选，状态、传输和调度筛选独立排列；表头恢复普通色，记录内容增加语义色并统一左对齐。
- 供应商管理：修正端点健康表头与内容对齐，移除无实际功能的详情眼睛按钮。
- 模型映射：删除重复的“精确”标识；管理映射弹窗恢复紧凑标签且不负责排序，列表展开区通过拖动手柄调整优先级。
- 模型测试：保持单一弹窗状态流，统一请求内容编辑表面，测试 Key 只展示最新名称。

各功能的实现与浏览器验收见同目录对应任务记录。

## 验证

- 目标 Vitest：11 个文件、178 项通过。
- 桌面脚本测试：19 项通过。
- TypeScript、目标 ESLint、前端生产构建通过。
- 真实浏览器覆盖 1200px、1024px、390px，均无横向溢出。
- `build:verified` 的 8 项门禁全部通过：使用记录 UI、模型映射 UI、模型测试 UI、供应商 UI、前端质量、桌面脚本、随包生命周期和产物校验。

## 桌面交付

- 版本：`0.1.77`
- 架构：Apple Silicon arm64
- APP：`target/release/bundle/macos/Aether.app`
- DMG：`target/release/bundle/dmg/Aether_0.1.77_aarch64.dmg`
- SHA-256：`d27932f3f710d6018fd38c40ba3b6a38b9823a15114354ad291c724279a313e5`
- 签名：ad-hoc；未进行 Apple 公证。
- 门禁证据：`.cache/delivery/2026-10-29-ui-consolidation-release/run-1/result.json`
