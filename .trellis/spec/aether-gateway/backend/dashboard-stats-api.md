# Dashboard Stats API Contract (`/api/dashboard/stats`)

> Cross-layer contract between the gateway backend (card assembly) and the admin frontend (generic card rendering).

---

## Scenario: Dashboard stat cards are server-assembled

### 1. Scope / Trigger

- The 4 stat cards at the top of the admin Dashboard are **fully assembled in the backend** (`apps/aether-gateway/src/handlers/public/support/dashboard_filters.rs`, `handle_dashboard_stats_get`): names, values, sub-values, badges, and icons are all backend-authored Chinese copy. The frontend `Dashboard.vue` renders whatever the `stats` array contains and never interprets card semantics.
- Any change to card count, order, or content is a **cross-layer contract change** and must update: backend handler → backend integration tests → frontend `emptyStatPlaceholders` (empty-state copy) → `frontend/src/api/dashboard.ts` types.

### 2. Signatures

- `GET /api/dashboard/stats?timezone=...&tz_offset_minutes=...` → `DashboardStatsResponse`
- Backend assembly point: `handle_dashboard_stats_get` in `dashboard_filters.rs`; response is cached 30s (`stats:admin:<query>` key).

### 3. Contracts

`stats` is an ordered array of exactly 4 cards (order matters, UI renders left→right):

| # | name | value | subValue | change (badge) | icon |
|---|------|-------|----------|----------------|------|
| 1 | 今日请求 | 今日请求总数 (integer) | `成功 X / 失败 Y`（失败 = error_requests） | `成功率 Z%`（0 请求时 `0.0%`） | Activity |
| 2 | 今日 Token | compact token 值 | 输入/输出/缓存明细 | — | Zap |
| 3 | 今日费用 | USD 费用 | 仅当节省 > 0 时为 `节省 $X`，否则无该键 | — | DollarSign |
| 4 | 全站 RPM / TPM | 最近 60 秒 RPM / TPM | `最近 60 秒` | — | Activity |

Other top-level payload fields: `today`, `api_keys`, `tokens`, `system_health`, `cost_stats`, `cache_stats`, `token_breakdown`. The `users` field was **removed** in the single-user conversion (2026-09); `/api/admin/system/stats` is a different endpoint and keeps its own `users` payload — do not confuse the two.

### 4. Validation & Error Matrix

- Usage data backend unavailable → `dashboard_backend_unavailable_response` (503-class).
- Auth failure → `resolve_authenticated_local_user` error response (admin-only since single-user phase 4).
- No validation on card content: backend copy is trusted by the frontend renderer.

### 5. Good/Base/Bad Cases

- Good: card emitted with `change` badge → frontend renders it as a secondary `Badge`.
- Base: 0 requests today → card 1 shows `0`, subValue `成功 0 / 失败 0`, badge `成功率 0.0%`.
- Bad: emitting a card key the frontend type doesn't declare (e.g. re-adding `users` without updating `DashboardStatsResponse`) — type drift goes unnoticed because the renderer is generic.

### 6. Tests Required

- `apps/aether-gateway/src/tests/frontdoor/public_support/dashboard.rs` must assert: the exact 4-card name order array, per-card `value`/`subValue`/`change` for at least one fixture, and absence of removed payload fields.
- Frontend `frontend/src/views/shared/__tests__/Dashboard.spec.ts` covers refresh behavior only; keep `emptyStatPlaceholders` copy in sync manually when cards change.

### 7. Wrong vs Correct

#### Wrong

Adding a card only in the backend and assuming the frontend "will show it" — empty-state placeholders and `DashboardStatsResponse` docs go stale, and the empty dashboard shows the old card set.

#### Correct

Treat the 4-card array as a versioned contract: backend json! block + integration test order assertion + `emptyStatPlaceholders` + `dashboard.ts` types updated in the same change.
