# Aether macOS 客户端界面改造规划

日期：2026-09-09。状态：规划草案，未实施。

用户要求先规划、不写代码，并已选择 **macOS 原生工具风格**。本次检查对象是 `/Users/zhenglizhi/otherProjects/Aether-tauri-macos` 的 `codex/tauri-macos` 工作区，基线为 `a31c31736`，包含尚未提交的客户端实现。本文件是独立的界面规划补充，不改变原客户端任务状态或其已有实现。

## 目标与边界

让现有 Aether 功能在 Mac 窗口中呈现为紧凑、清晰、稳定的桌面工具：导航和操作位置固定，内容使用窗口空间，缩放窗口时仍保持桌面操作方式。

保留 Vue、Rust 网关、现有管理业务、路由与数据契约。改造范围是桌面布局、视觉样式、现有控件的摆放、内容滚动和窗口适配。保留 Web/Docker 界面及其移动端适配，不复制一套桌面业务实现。

继续遵守已确认的客户端意图：免登录；不恢复账号/密码/退出登录入口及左下角身份区域；主题、语言、时区保留在已有设置体系；关闭窗口后后台运行；显式退出才停止网关。模型映射、备份导入导出等共享功能保持与个人分支一致。

本轮不实施产品代码、不构建新安装包，也不提交或推送。新业务功能、全局搜索/命令面板、多工作区、SwiftUI 重写、生命周期调整和新增 IPC 权限不属于这份界面改造。

## 已确认的问题

| 问题 | 代码或实机依据 | 对客户端的影响 |
| --- | --- | --- |
| 默认窗口会使用移动端列表 | [windows.rs](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/apps/aether-desktop/src-tauri/src/windows.rs:112) 默认管理窗口为 1200×820；[ProviderManagement.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/views/admin/ProviderManagement.vue:59) 和 [ApiKeys.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/views/admin/ApiKeys.vue:88) 使用 `hidden xl:block` 显示表格，低于该断点改为卡片。本地 Tailwind 默认 `xl=1280px`，项目没有覆盖响应式断点 | 默认大小就看不到桌面表格，信息密度降低，纵向滚动增加 |
| 缩小窗口会出现手机导航 | 管理窗口最小允许 840×620；[MainLayout.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/layouts/MainLayout.vue:182) 的导航在 `lg=1024px` 以下切换为手机顶部菜单 | 840–1023px 是合法桌面窗口宽度，却改变了导航位置和操作方式 |
| 滚动范围过大 | [AppShell.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/components/layout/AppShell.vue:30) 将 header 和 main 放在同一容器；[style.css](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/style.css:973) 让整个内容列滚动 | 右侧滚动条覆盖大段窗口高度，工具栏依赖 sticky，页面内部表格/抽屉又有自己的滚动区域 |
| 标题、间距和装饰仍按网页组织 | [MainLayout.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/layouts/MainLayout.vue:326) 有 64px 网页页头；[PageHeader.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/components/layout/PageHeader.vue:20) 使用 24–30px 页面大标题；外壳与 [PageContainer.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/components/layout/PageContainer.vue:46) 都有留白 | 部分页面重复标题和内边距，列表可见行数偏少；原生标题栏与网页工具栏缺乏整体感 |
| 概览在默认宽度也会堆高 | [Dashboard.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/views/shared/Dashboard.vue:18) 的四列统计使用 `xl:grid-cols-4`，同时有较大的图标、卡片内边距和装饰光斑 | 1200px 窗口中统计通常分成两列，核心信息占据更多纵向空间 |
| 设置页像长网页 | [SystemSettings.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/views/admin/SystemSettings.vue:1) 使用大标题、纵向分组和右侧 176px 悬浮目录 | 有限窗口里同时出现主侧栏、正文和网页目录，表单宽度被挤压 |
| 启动/客户端设置窗口也在整页滚动 | 实机观察到 960×760 的客户端设置窗口右侧滚动条及底部内容被截出视口；[desktop.css](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/desktop/desktop.css:1) 是最大 820px 的居中单列，状态、设置和日志纵向堆叠 | 低频信息长期占据主要空间，设置窗口像一个网页表单 |
| 只修改现有 desktop.css 不会覆盖管理页 | [desktop/main.ts](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/desktop/main.ts:1) 仅给启动窗口加载 desktop.css；管理页仍由 [main.ts](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/main.ts:1) 和共享 MainLayout 渲染 | 改启动页面的样式无法解决管理页的断点、表格和滚动问题 |

实机观察的边界：本次可见窗口停在“正在启动网关”，从已有菜单打开管理后台后仍显示该状态，因此没有把管理页描述为已完成视觉实测。管理页结论依据当前源码。启动状态的原因未在本轮诊断；后续视觉验收需要使用可进入运行态的对应构建。

## 统一桌面布局

推荐采用以下区域分工：

| 区域 | 推荐形式 | 滚动规则 |
| --- | --- | --- |
| 窗口顶部 | 保留系统红黄绿按钮和窗口行为；页面标题、返回层级及常用操作收敛为紧凑工具栏 | 固定，不随正文滚动 |
| 左侧导航 | 复用现有七个入口及分组；浅灰侧栏、明确选中项；宽度建议 208–224px，紧凑模式沿用约 64px | 常规高度不滚动；极矮窗口仅导航区域可滚动 |
| 页面工具区 | 当前页已有的搜索、筛选、刷新、新建等操作放在一致的位置；复杂筛选可占第二行 | 保持可见，不与长列表一起滚走 |
| 内容区 | 概览、表格、设置面板按页面类型填满剩余空间 | 每个内容面板有明确的纵向滚动容器 |
| 详情与弹窗 | 复用已有抽屉、对话框；统一间距和标题/操作区 | 内部内容滚动，不带动后方页面；不遮挡操作按钮 |

不要给每个页面都套同一个固定高度：列表适合固定工具栏加滚动表格，概览适合单一内容区滚动，长表单适合分组导航加滚动表单。可以存在相邻独立滚动区域，但同一内容面板不应出现两条互相争抢的纵向滚动条。

### 滚动条处理

先调整滚动归属，再调整视觉。外层窗口壳不滚动，工具栏位于滚动容器之外；真正溢出的内容才需要滚动。日志和必要的宽表格可以内部横向滚动，但不能把整个窗口撑宽。

滚动条常驻还可能受 macOS“显示滚动条”偏好影响。本次没有读取或修改该系统偏好，不能认定常驻全部来自代码。系统支持“自动”“滚动时”“始终”三种设置：自动模式下尽量使用系统的滚动呈现；始终显示时留出正确空间，不挤压内容或引起跳动。不要用全局 `display:none` 隐藏所有滚动条，也不新增一套自制拖拽滚动控件。

### 窗口与原生外观

第一批保持现有窗口尺寸契约：管理窗口默认 1200×820、最小 840×620；客户端设置默认 960×760、最小 740×580。不要通过提高最小宽度或要求全屏来掩盖布局问题。

窄窗口仍保留桌面导航和列表。根据内容区宽度压缩留白、折行单元格、使用已有紧凑侧栏与详情抽屉；不能继续仅凭网页 `lg/xl` 断点切成手机页面。表格的重要状态、金额和操作仍需可获取。

原生标题栏与应用工具栏在视觉上对齐，移除重复的大页头。标题栏融合属于后续宿主适配验证点：保留系统按钮、拖拽空白区、双击标题栏、缩放和全屏行为，不用网页按钮模拟红黄绿。当前支持 macOS 14 起，不把依赖新系统的玻璃效果作为首版前提。

## 视觉基调

以下尺寸是本项目的建议初值，需要通过样板页截图校准，并非 Apple 强制规范：

| 项目 | 建议 |
| --- | --- |
| 字体 | 桌面范围优先系统 UI 字体，中文使用系统中文字体；模型标识、金额和路径使用合适的等宽/等宽数字样式，保留文本选择与复制 |
| 字号 | 正文约 13–14px，辅助文字至少约 12px，页面标题约 17–20px；不以全面缩小文字来压缩页面 |
| 颜色 | 保留 Aether 标识与现有蓝色强调；中性灰分层、浅色内容底，深色模式同步设计；状态不能只靠颜色区分 |
| 间距 | 页面内边距约 16–20px，控件间距约 8–12px；移除外壳与页面重复 padding |
| 表格 | 单行内容行高约 36–40px，长状态或名称允许增高；列对齐稳定，操作区位置一致 |
| 圆角与边框 | 常规面板约 6–10px 圆角，细分隔线；减少每层都套大圆角卡片、阴影、光斑和纸张纹理 |
| 动效 | 轻量状态反馈和必要的展开/收起，避免整页弹跳；尊重减少动态效果设置 |
| 可访问性 | 文本对比度、可见键盘焦点、清楚的图标标签；紧凑不等于难点、难读或只能悬停操作 |

设计库未找到适合该 macOS 工具的完整方案：首次返回网页落地页模板，收窄检索仍主要是通用 Web 滚动建议，因此没有采用其页面结构、字体下载或配色模板。本规划的具体布局来自本项目源码和实机观察，系统行为参考文末 Apple 资料。

## 页面改造范围

| 页面 | 计划调整 | 保持的内容与行为 |
| --- | --- | --- |
| 渠道 | 默认窗口直接显示紧凑桌面列表；整合标题、搜索和筛选；固定表头；重排单元格内部信息，减少卡片式纵向堆叠 | 排序口径、余额与健康信息、启停、编辑及已有详情抽屉 |
| 系统设置 | 以分组设置面板取代网页式右侧目录；窄窗口用紧凑分组切换，不叠出过窄的第三列；统一标签和输入框对齐 | 现有分组、字段、独立保存与导入导出；主题/语言/时区仍从设置进入 |
| 概览 | 统计摘要收敛为紧凑区域，按内容区宽度排布；降低装饰比例；图表和健康摘要建立清晰层次 | 所有指标与统计口径，不以隐藏指标换取整洁 |
| 使用记录 | 筛选和刷新保持可见；列表使用剩余高度；分析区保留既有折叠能力；详情抽屉统一尺寸和滚动 | 过滤、分页、分析折叠偏好、自动刷新及记录详情，不新增查询维度 |
| 模型 | 应用统一列表、筛选和编辑面板规则，长模型名和多标签合理折行 | 模型映射、定价、允许范围和保存行为 |
| API Key | 默认使用桌面列表和紧凑工具栏，弱化外层装饰，复用已有编辑弹窗 | 密钥显示/复制规则、额度、限流和启停，不增加暴露范围 |
| 路由 | 明确列表、配置区和当前编辑上下文；减少重复标题，改善小窗口对齐 | 既有优先级、分组、候选策略及保存语义 |
| 客户端设置 / 启动 / 连接反馈 | 紧凑状态区＋分组设置；目录和诊断使用适当的次级展示；长日志只在自身区域滚动；状态切换的文案和视觉一致 | 已有真实状态、端口修改限制、自启、打开目录、启停和退出规则，不修改重试/超时逻辑 |

管理页“系统设置”与应用菜单“客户端设置”目前承载不同内容。首轮统一视觉、明确分组，保留其已有权限和入口语义，不直接把两个 WebView 合为一个，也不恢复已删除的左下角入口。

现有菜单快捷键和复制/粘贴行为继续有效。弹窗保留既有 Esc、焦点管理和提交规则；不在本轮新增全局快捷键系统。

## 实施分批建议

| 批次 | 交付范围 | 验收重点 |
| --- | --- | --- |
| 1：桌面基础布局 | 桌面专用布局容器、标题/工具栏、侧栏、滚动归属、桌面断点和基础样式 | 默认及最小窗口保持桌面导航；无整窗横向滚动；工具栏不随内容滚走 |
| 2：两个样板页 | 渠道＋系统设置；同时校准客户端设置窗口的密度 | 桌面列表、设置分组和控件密度形成稳定样式；功能和字段完整 |
| 3：其余页面 | 概览、使用记录、模型、API Key、路由、详情与弹窗 | 使用相同的布局规则；长内容、筛选、分页和弹窗不回退 |
| 4：原生窗口打磨 | 标题栏对齐、缩放/全屏、键盘、深浅主题及启动/错误反馈一致性 | 在真实 `.app` 中核验，不用普通浏览器结果代替 macOS 验收 |

建议首先认可第 1、2 批的视觉样板，再铺开其他页面，避免一次改遍所有页后重新调整风格。此分批是后续实施建议；当前只有规划文档，不开始任何一批的产品改动。

## 技术边界与需要同步检查的调用点

1. 桌面端可增加独立布局容器，复用现有导航构建器、路由、业务组件和 composables；不复制全部管理页面，也不让每一页自行实现平台检测和滚动外壳。
2. 管理 WebView 需要自己的桌面样式入口。现有 `frontend/src/desktop/desktop.css` 只覆盖启动窗口，不能误以为改它即可改全部客户端。共享组件可以增加展示变体，Web 默认展示应保持原样。
3. 桌面样式作用域要覆盖 Teleport 到 body 的弹窗/抽屉，同时不污染普通网页。平台检测复用现有桌面会话机制，不把视觉标志变成鉴权依据。
4. 保留 `#header-actions-right`、`#breadcrumb-actions` 等页面操作挂载契约，或成套迁移所有调用方。用量页通过 Teleport 放置分析折叠按钮，不能随重做工具栏丢失。
5. 改变滚动容器时，同步处理系统设置的目录、hash 跳转、IntersectionObserver 与滚动恢复。当前 [SystemSettings.vue](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/frontend/src/views/admin/SystemSettings.vue:254) 明确查找 `.app-shell__content` 并使用 80px 偏移，不能只改 CSS。
6. 核对通用 Table 的内部 `overflow-auto` 与外层表格容器，避免 nested overflow 破坏 sticky 表头、弹出菜单和横向滚动。
7. 保留桌面信任边界：[desktop-local.json](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/apps/aether-desktop/src-tauri/capabilities/desktop-local.json:1) 只给本地 `main` 窗口能力，管理窗口不具备通用 IPC 权限。窗口标题栏的调整由宿主承担，不为外观开放 shell、Keychain 或管理命令能力。
8. 原生窗口修改仅限展示相关配置，保留窗口与网关 generation 的绑定、关闭隐藏、退出收尾和恢复机制。它们不应被混入布局重构。

主要影响范围：`frontend/src/layouts`、布局/表格/弹窗等展示组件、桌面样式入口、上述管理页的展示部分，以及必要的 `apps/aether-desktop/src-tauri/src/windows.rs` 窗口外观配置。API、业务存储、网关执行和 SQLite 不属于主要修改范围。

## 后续验收清单

- 管理窗口在 1200×820、1024×768、840×620 及放大/全屏时保持桌面导航；默认窗口里的渠道和 API Key 使用桌面列表。
- 客户端设置在 960×760 与 740×580 下可操作；长日志、长路径和错误信息只让对应内容区域滚动，不遮挡关键控件。
- 无整窗水平溢出；同一面板没有双重纵向滚动；在系统自动与始终显示滚动条两种设置下均布局正确。
- 工具栏、筛选和表头在对应内容滚动时位置稳定；设置分组、深链接、详情关闭后的上下文和现有折叠偏好不丢失。
- 覆盖空数据、长列表、长中文/英文名称、窄窗口、加载、失败、弹窗和抽屉；图表或少量内容不被硬性要求塞进一屏而丢失信息。
- 深浅主题、键盘焦点、现有快捷键和文本复制保持可用；系统窗口按钮、拖拽、缩放与全屏正常。
- 模型映射、备份导入导出、渠道操作、用量过滤及设置保存保持原语义；普通 Web 端的桌面和手机页面没有样式回退。
- 受影响前端类型检查、lint、现有行为测试和构建有结论；滚动和断点用有意义的布局检查/真实窗口截图验证，不只增加 class 字符串断言。
- 最终在对应构建的 macOS `.app` 中验收，并区分已实测与尚未实测的系统/窗口场景。

## 参考

- [Apple HIG：Layout](https://developer.apple.com/design/human-interface-guidelines/layout)：内容与操作的层次、窗口适配和 macOS 布局注意事项。本方案不直接套用较新系统的材质效果。
- [Apple：Change Appearance settings on Mac](https://support.apple.com/en-au/guide/mac-help/mchlp1225/mac)：系统滚动条显示选项。
- [现有客户端需求](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/.trellis/tasks/09-09-tauri-macos/prd.md)。
- [桌面宿主与网关契约](/Users/zhenglizhi/otherProjects/Aether-tauri-macos/.trellis/spec/aether-desktop/backend/desktop-contract.md)。
