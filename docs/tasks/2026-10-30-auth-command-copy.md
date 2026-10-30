# Refresh Token 命令快速复制

## 目标与变更

- 用户希望无需选中文本，即可复制认证弹窗中的 Refresh Token 获取语句。
- `ProviderAuthDialog.vue` 将 Sub2API `refresh_token` 字段的帮助说明拆成提示与代码行，并添加复制图标按钮。
- 点击复用 `useClipboard`，只复制 `localStorage.getItem('refresh_token')`，不执行命令、不读取令牌、不改变认证提交数据；其他字段帮助文本保留。
- 新说明已补充英文映射，按钮复用现有“复制命令”翻译。

## 验证与自检

- Chromium 加载真实 ProviderAuthDialog，只有架构读取响应使用隔离数据；点击复制后用系统粘贴快捷键粘入文本框，内容与完整命令一致，成功反馈出现。
- 模拟剪贴板拒绝，真实 useClipboard 返回“复制失败，请手动选择文本进行复制”；英文按钮名称为 Copy command，新说明翻译正常。
- 900×640、1024×768、1440×900、1920×1080、390×844 下测量命令、按钮和正文，复制按钮 44×44px，均无横向溢出或相互覆盖。
- `npm run type-check` 通过；相关文件 ESLint 零错误，保留原有 formData 的 any 警告。
- 截图：`/tmp/aether-auth-command-copy-390.png`；临时夹具：忽略目录 `frontend/.cache/delivery/auth-command.html`。
- 自检确认命令只作为字符串复制，按钮为 type=button，普通字段帮助分支不变，未改变后端 Schema 或认证载荷。
- 未在 Tauri WKWebView 实测剪贴板；未重新打包本次改动。
