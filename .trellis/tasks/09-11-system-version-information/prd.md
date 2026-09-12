# 修正系统版本信息

## Goal

在桌面应用的系统信息中展示发行版本，而不是网关构建 SHA 或工作树状态。

## Confirmed Facts

- `desktop_status` 的 `version` 是 Tauri `CARGO_PKG_VERSION`，目前为 `0.1.7`。
- 系统信息组件当前经 `adminApi.getSystemVersion()` 显示网关版本，可能为 `387d7e56b-dirty`。

## Requirements

- 桌面会话优先使用可信的宿主 IPC 版本信息。
- 非桌面部署继续显示网关版本，不改变其 API 合约。
- IPC 读取失败不能阻断设置页；沿用已有服务端版本作为回退。

## Acceptance Criteria

- [ ] 在桌面应用中，系统信息显示格式为用户可读的发行版本（例如 `0.1.7`），不包含 SHA 或 `dirty`。
- [ ] 在非桌面会话中，当前 `/api/admin/system/version` 行为保持兼容。
- [ ] 版本读取错误不会导致设置页其他部分不可用。

## Out Of Scope

- 不调整版本分配、构建或发布脚本。
