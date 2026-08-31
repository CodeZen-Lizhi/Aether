//! Responses WebSocket 两侧的 PII 脱敏：请求侧 mask + 响应侧 restore。
//!
//! HTTP 路径在前门建 `RedactionSessionSlot` 并塞进 `parts.extensions`，planner
//! 只有拿到这个 slot 才会脱敏。WS 的 planning Parts 是合成的：四个规划入口
//! （首轮、换模型 re-plan、独立轮、配额透明重试）靠 `build_planning_parts` 注入
//! slot 就能复用 planner 的脱敏；但复用已绑定 upstream 的 continuation 根本不进
//! planner，必须在这里先把客户端事件脱敏，再交给协议归一化、上游发送和审计。
//!
//! 因此约定：**进入任何下游用途之前，客户端 `response.create` 只在这里脱敏一次**，
//! 之后所有路径都只看脱敏后的事件。
//!
//! # 响应侧
//!
//! 只 mask 不 restore 是半个实现：HTTP 在把响应交给客户端之前会把占位符换回真实值
//! （`privacy::restore_sync_response_body` / `privacy::StreamingResponseRestorer`），
//! WS 少了这一步，客户端就会直接看到 `<AETHER:EMAIL:...>`。
//! [`ResponsesWebSocketRedactionRestorer`] 补上这一跳，语义与 HTTP 完全一致：
//! 复用 `privacy::restore_json_strings`，只还原本连接自己 mask 出来的映射，
//! 未映射的占位符原样透传。
//!
//! ## session 为什么活在连接上而不是活在这一轮里
//!
//! mask session 由 planner 写进 per-turn 的 slot，而 slot 随 planning Parts 在
//! 规划结束时就被丢弃，响应帧到达时已经无处可取。可选的存活范围有两个：
//!
//! * 挂在 `LogicalTurn` 上：这一轮结束即释放，是 HTTP「一个请求一个 session」的
//!   直译。但 WS 的会话历史留在上游：continuation 只发增量输入，第 1 轮的
//!   `input` 不会在第 3 轮重发。于是第 3 轮的响应里若回显了第 1 轮的占位符
//!   （"你刚才给我的邮箱是……"），本轮 session 里没有这条映射，占位符就漏给客户端。
//!   HTTP 不会漏，是因为它每次都重发整段历史，重新 mask 同一个值会派生出同一个
//!   sentinel（HMAC over 规则 + bucket + 值），所以映射天然齐备。
//! * 挂在当前 response chain 上（当前实现）：每轮仍然各自 mask、各自持有独立 session
//!   （per-turn 语义不变），连接只是把最近若干轮的 session 留下来一起参与还原，
//!   凑出的映射集合正好等于「等价 HTTP 请求会拥有的那一份」。
//!
//! 选后者。省略（或置空）`previous_response_id` 会开始一条独立 response chain，
//! 此时必须丢弃旧链的映射，否则新链里偶然出现的旧 sentinel 会被还原成旧链 PII。
//! 代价是每帧最多对 [`MAX_RETAINED_TURN_REDACTION_SESSIONS`] 个 session 各扫一遍，
//! 以及这些 session 的映射会驻留到当前链结束；用有界 FIFO 兜住上限。
//! 窗口不够用或每帧成本变高时，正确的下一步是在 `privacy` 侧提供跨 session 的
//! 合并匹配器，而不是把这个窗口调大。

use std::collections::VecDeque;

use serde_json::Value;

use crate::ai_serving::{
    resolve_local_decision_execution_runtime_auth_context, resolve_provider_chat_pii_redaction,
};
use crate::control::GatewayControlDecision;
use crate::privacy::{restore_json_strings, RedactionSession, RedactionSessionSlot};
use crate::{AppState, GatewayError};

/// Responses WebSocket 只承载 `openai:responses`，脱敏规则按这个客户端格式选取。
const RESPONSES_WEBSOCKET_CLIENT_API_FORMAT: &str = "openai:responses";

/// WS 在选出候选之前就要脱敏，所以脱敏 session 先记在这个固定 key 下。
///
/// slot 是 per-turn 的（见 `build_planning_parts`），这一轮之后即随 slot 一起丢弃；
/// planner 后续用真实 candidate_id 再取一次配置时，body 已是脱敏态、不会重复写入。
const WEBSOCKET_TURN_REDACTION_CANDIDATE_ID: &str = "responses_websocket_turn";

/// 一条连接最多留几轮的 mask session 用于响应侧还原。
///
/// 取值权衡见模块文档：调大会线性增加每帧还原成本和常驻映射量，调小则更容易漏还原
/// 上游历史里更早那几轮的占位符。8 覆盖的是「上游最可能回显的最近窗口」。
const MAX_RETAINED_TURN_REDACTION_SESSIONS: usize = 8;

/// 一轮客户端 `response.create` 的请求侧脱敏结果。
#[derive(Debug)]
pub(super) struct ResponsesWebSocketTurnRedaction {
    /// 脱敏后的客户端事件；这一轮之后所有下游路径都只看它。
    pub(super) client_event: Value,
    /// 这一轮 mask 出来的映射表，响应侧还原只能靠它。
    pub(super) session: RedactionSession,
}

/// 对一条客户端 `response.create` 做请求侧脱敏。
///
/// 返回 `Some(..)` 仅当脱敏真正命中；`None` 表示未启用或没有命中，调用方
/// 继续用原事件即可（避免未开启脱敏时多一次整包 clone）。
///
/// 脱敏只改写 `instructions` / `input`（见 `privacy::mask_openai_responses_request_value`），
/// `type` / `model` / `previous_response_id` / `generate` 等协议字段原样保留，所以脱敏后的
/// 事件仍可直接用于协议归一化和上游发送。
///
/// 出错必须让这一轮失败：脱敏已启用却读不到配置或加密密钥时，把原文发上游就是
/// 静默旁路，正是本次要修的问题。
pub(super) async fn redact_responses_websocket_client_event(
    state: &AppState,
    parts: &http::request::Parts,
    control_decision: &GatewayControlDecision,
    client_event: &Value,
) -> Result<Option<ResponsesWebSocketTurnRedaction>, GatewayError> {
    redact_responses_websocket_client_event_with_reasoning_replay_policy(
        state,
        parts,
        control_decision,
        client_event,
        crate::ai_serving::OpenAiResponsesReasoningReplayPolicy::OpenAiItemIds,
    )
    .await
}

/// Variant used only after the gateway has selected and authenticated the
/// provider binding. The replay policy comes from that trusted binding, never
/// from client JSON, so a forged reasoning-item shape cannot opt itself into
/// byte-opaque PII handling.
pub(super) async fn redact_responses_websocket_client_event_with_reasoning_replay_policy(
    state: &AppState,
    parts: &http::request::Parts,
    control_decision: &GatewayControlDecision,
    client_event: &Value,
    reasoning_replay_policy: crate::ai_serving::OpenAiResponsesReasoningReplayPolicy,
) -> Result<Option<ResponsesWebSocketTurnRedaction>, GatewayError> {
    let Some(auth_context) =
        resolve_local_decision_execution_runtime_auth_context(control_decision)
    else {
        return Ok(None);
    };
    let redaction = resolve_provider_chat_pii_redaction(
        state,
        parts,
        client_event,
        &auth_context,
        RESPONSES_WEBSOCKET_CLIENT_API_FORMAT,
        reasoning_replay_policy,
        WEBSOCKET_TURN_REDACTION_CANDIDATE_ID,
    )
    .await?;
    if !redaction.redacted {
        return Ok(None);
    }
    // mask 命中时 `resolve_provider_chat_pii_redaction` 必定把 session 写进 slot。
    // 取不到就是内部契约被破坏了，此时继续下发意味着这一轮的响应无法还原、占位符
    // 会漏给客户端；按本模块既有的「脱敏链路出错就让这一轮失败」处理，不做降级。
    let Some(session) = parts
        .extensions
        .get::<RedactionSessionSlot>()
        .and_then(|slot| slot.take_for_candidate(Some(WEBSOCKET_TURN_REDACTION_CANDIDATE_ID)))
    else {
        return Err(GatewayError::Internal(
            "chat pii redaction masked a Responses WebSocket turn without retaining its session"
                .to_string(),
        ));
    };
    Ok(Some(ResponsesWebSocketTurnRedaction {
        client_event: redaction.body_json.into_owned(),
        session,
    }))
}

/// 当前 response chain 上「我们 mask 过哪些映射」的留存集合，供响应侧还原使用。
///
/// 每轮一个独立 session（per-turn mask 语义不变），当前链按 FIFO 留最近
/// [`MAX_RETAINED_TURN_REDACTION_SESSIONS`] 轮。物理上游重绑本身不决定生命周期；
/// `previous_response_id` 决定是否延续旧链。独立请求成功发出时由调用方通过
/// [`Self::start_new_chain`] 原子替换为新链的首轮 session。
#[derive(Default)]
pub(super) struct ResponsesWebSocketRedactionRestorer {
    sessions: VecDeque<RedactionSession>,
}

impl ResponsesWebSocketRedactionRestorer {
    pub(super) fn has_sessions(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// 登记这一轮的 mask session。
    pub(super) fn register(&mut self, session: RedactionSession) {
        if session.mapping_count() == 0 {
            return;
        }
        self.sessions.push_back(session);
        while self.sessions.len() > MAX_RETAINED_TURN_REDACTION_SESSIONS {
            self.sessions.pop_front();
        }
    }

    /// Commits a successfully started independent response chain.
    ///
    /// Keep this transition next to the successful upstream send/bind. A
    /// rejected independent request has not replaced the active chain and
    /// therefore must not discard the old chain's restore mappings.
    pub(super) fn start_new_chain(&mut self, session: Option<RedactionSession>) {
        self.sessions.clear();
        if let Some(session) = session {
            self.register(session);
        }
    }

    /// 把一帧 provider 事件里的占位符换回真实值，返回要发给客户端的帧文本。
    ///
    /// `None` 表示这一帧没有任何东西要还原，调用方必须原样转发上游字节：未启用
    /// 脱敏（没有任何 session）时连 clone 都不做。
    ///
    /// 入参只读：审计与终态观测继续消费脱敏态的事件，还原只作用于发往客户端的
    /// 那一份拷贝，和 HTTP 侧「审计存脱敏体、线上还原」保持一致。
    pub(super) fn restore_provider_frame_text(&self, event: &Value) -> Option<String> {
        if self.sessions.is_empty() {
            return None;
        }
        let mut restored_event = event.clone();
        let mut restored = false;
        for session in &self.sessions {
            // 逐 session 还原而不是合并映射：每个 session 只认自己 mask 过的
            // sentinel（`RedactionSession::restore_text`），跨 session 合并会绕开
            // 这条边界。同一个值在不同轮派生出的 sentinel 相同，所以顺序无关。
            restored |= restore_json_strings(&mut restored_event, session);
        }
        if !restored {
            return None;
        }
        // 刚从 JSON 解析出来的 Value 再序列化不会失败；真失败时宁可让客户端看到
        // 占位符，也不能丢掉这一帧——丢帧会让客户端的协议状态机卡死。
        serde_json::to_string(&restored_event).ok()
    }
}
