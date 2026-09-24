# 修复与验证记录（2026-09-15）

## 实现

- Responses 首有效输出识别补全：非空 text/reasoning/refusal/tool done、content part 和含 message/reasoning/compaction 的 item done。空内容、启动事件不解除等待；单项 done 不作为整个响应成功。
- 首有效输出 INFO 日志记录事件类型与时间，不记录正文或 encrypted_content。
- 总预算取消保留最小身份/路由/正文引用，交接已有 usage 有序写入，终态 failed/504 与实际逻辑耗时。终态交接解除首输出预算，避免持久化再被同一计时器取消。
- watchdog 耗尽使用请求循环内保存的真实结果，候选 failed/504；避免读取异步候选投影来决定立即返回的错误。
- usage 终态补传已有 candidate_index 字段，无 Schema 变化。

## 已执行回归

- 实现代理：useful_output 5 项（原代码先失败，修复后通过），watchdog 10 项，scheduler_failover 17 项；另外原套件中耗时探针测试单独已经完整通过。
- 集成后：cargo test -p aether-gateway --lib first_output，6 项通过，包括总预算共享、终态持久化不取消、compact 身份/路由/敏感数据约束及候选耗尽。
- 仅原有 dispatch/refs.rs:71 unreachable-pattern 警告。一次临时编译错误（错误给同步 port 初始化流式字段）已移除；再次编译及 6 项测试通过。
- 第一轮真实 HTTP + 临时 SQLite：verification-1789404755507852000/summary.json。
  普通 text.done 与 compact item.done 首内容约 0.55 秒，完整完成约 95.6 秒，30 秒单次阈值 / 90 秒逻辑预算，均 completed/200。
  短 text.done/compaction.done/delta 长完成、总预算 504、输出后 EOF 失败也通过。
  此轮候选 watchdog 返回 503/no_local_stream_plans 失败，暴露了候选投影队列的读写时序问题；已改为循环内保存执行结果，最终二进制已重验通过。

## 审查依据与限制

主会话检查了两个串行候选循环、失败重试/终态交接调用点、usage seed 转换、候选状态入队与 SQLite 读回、敏感数据边界，并用公开接口复现纠正了内存测试未发现的问题。
所有上游为本机合成服务，数据库为临时 SQLite，仅从真实库只读取得表结构。没有重放生产请求，不能据此确认全部历史失败同源，也不能消除上游/Clash 的实际断线。
用户供应商 Input-国际线路只读核对仍为 300 秒 / 600000 ms。本次不改变默认值或用户配置，也不改变 legacy 非流式 compact 的总超时。
代码尚未提交、发布或替换 /Applications/Aether.app，运行中的安装版不自动获得本次修复。

## 最终验收

- 最终二进制 cargo build -p aether-gateway --bin aether-gateway 成功；cargo fmt -p aether-gateway -p aether-usage-runtime --check 和限定 git diff --check 通过。
- output/compact-timeout-diagnosis/final-verification.json 汇总 8 个通过场景（最终二进制 6 个短场景，加输出识别修复后已通过的 2 个 95 秒长场景；后续改动只涉及失败分类）。
- watchdog: HTTP 504、usage compact/failed/504，response_time_ms=702，候选 failed/504；总预算: HTTP 504、usage compact/failed/504，response_time_ms=1803，候选 cancelled/504。后续 SQLite 读回状态一致。
- 最终短场景第一轮 delta 测试未启动，是并行脚本 time_ns 临时目录冲突；目录加 UUID 后仅重跑该项，首内容 447ms、总时长 3473ms、completed/200。其余场景无需重复。
- 源码修复和限定验证已完成；没有授权提交/发布，因此保留可审阅工作区改动和活动任务，不执行 Trellis 自动提交/归档。
