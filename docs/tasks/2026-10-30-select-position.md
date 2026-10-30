# 下拉菜单定位修复

## 目标与范围

修复认证模板及其他共用 Select 菜单偏到窗口左上方的问题。菜单应紧贴触发框，保留选项选择、搜索、滚动跟随和窗口边界避让；不调整业务配置或弹窗布局。

## 定位与实现

- 在真实 KeyFormDialog 夹具复现：900×640 下触发框 left=227，菜单 left=37；菜单仍挂载在弹窗内部。
- 仅临时关闭 `.app-dialog` 的容器查询即可恢复锚点，恢复容器查询则再次偏移，确认当前定位库与浏览器对 fixed 元素包含块的判断不一致。
- `SelectContent` 默认启用 Portal 并挂载到 body，保留显式 `disablePortal`；移除仅用于自动禁用 Portal 的 Dialog 注入标记。
- 补充 `check-select-position.cjs` 浏览器回归检查和前端规范。

## 验证

- 修复前运行浏览器检查，菜单坐标断言失败；修复后 Chromium 在 900×640、1440×900、390×844 下全部通过，间距均为 4px。
- 验证选择“跟随上游”、重新打开并选择“手动”，触发框更新且弹窗保持打开。
- `npm run type-check`、本次 Vue 文件 ESLint 通过。
- ProxyNodeSelect 与 KeyMultiplierControl 共 13 个现有测试通过；新增浏览器回归脚本 ESLint 通过。
- Ego Chromium 加载真实 ProviderAuthDialog 与 ProxyNodeSelect，只有读取数据由临时夹具提供：认证模板和认证方式菜单均左对齐、间距 4px；代理节点搜索、选中、正文滚动跟随、菜单打开时缩放到 900×640 均正常；弹窗外 Select 搜索和选择正常。
- 视觉证据：`/tmp/aether-auth-template-fixed.png`、`/tmp/aether-auth-proxy-fixed.png`。隔离夹具位于忽略的 `frontend/.cache/delivery/select-position.html`，不包含真实认证配置，不验证保存或上游认证。
- 自检覆盖所有 `disablePortal` 调用方及已移除标记的引用；现有显式开启 Portal 的调用方继续兼容。
- 未构建或替换安装中的桌面应用，未在 Tauri WKWebView 验证；当前结论为 Chromium 中实际组件的布局和交互验收。

## 追加尺寸与其他入口核验

- 共用 Select 回归脚本补测九组尺寸：840×620、900×640、1024×768、1280×800、1440×900、1920×1080、390×844、639×844、640×844，全部通过；打开菜单时从 1920px 每步缩小 60px 到 900px，再恢复 1920px，锚点与视口边界断言通过。
- 真实 ProviderAuthDialog 的认证模板、认证方式在上述九组尺寸共 18 个场景中通过：左右锚点一致、间距约 4px（小数像素舍入误差小于 0.5px）、菜单位于视口内、中心点可点击且无横向溢出。移动端背景夹具的固定宽度曾触发溢出断言，修正夹具后重跑通过，产品代码无新增改动。
- DropdownMenu、Popover、MultiSelect、PlatformSelect 的隔离代表样例，在 390、900、1920px 下共 12 个场景未出现同类坐标偏移；这部分仅检查锚点关系，不证明所有业务调用方的裁切、窗口边界或完整交互。PlatformSelect 当前无业务调用。
- 全部 Select 的显式 Portal 配置检查完毕，只有明确启用 Portal 的调用方，没有 `disablePortal=true` 的业务入口；本轮未逐页遍历全部业务入口。
- 扩展后的回归脚本 ESLint 与 `git diff --check` 通过。桌面 WKWebView 仍未实测，安装中的旧版本不会因源码修复自动更新。
