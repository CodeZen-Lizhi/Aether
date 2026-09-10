# Dashboard Stats API Contract (`/api/dashboard/stats`)

> Cross-layer contract between the gateway backend (card assembly) and the admin frontend (generic card rendering).

---

## Scenario: Dashboard stat cards are server-assembled

### 1. Scope / Trigger

- The 4 cards in the API `stats` array are **fully assembled in the backend** (`apps/aether-gateway/src/handlers/public/support/dashboard_filters.rs`, `handle_dashboard_stats_get`): names, values, sub-values, badges, and icons are backend-authored Chinese copy. The frontend renders these cards and appends a fifth, 今日平均响应, from the existing response-time summary.
- Changes to the API card array are a **cross-layer contract change** and must update: backend handler → backend integration tests → frontend placeholders → API types. The fifth card is frontend-only and does not change the four-card API contract.

### 2. Signatures

- `GET /api/dashboard/stats?preset=today&timezone=...&tz_offset_minutes=...` → `DashboardStatsResponse`. The Dashboard explicitly selects today; omitting the range defaults summaries to this month.
- Backend assembly point: `handle_dashboard_stats_get` in `dashboard_filters.rs`; response is cached 30s (`stats:admin:<query>` key).

### 3. Contracts

`stats` is an ordered array of exactly 4 cards (order matters, UI renders left→right):

| # | name | value | subValue | change (badge) | icon |
|---|------|-------|----------|----------------|------|
| 1 | 今日请求 | 今日请求总数 (integer) | `成功 X / 失败 Y`（失败 = error_requests） | `成功率 Z%`（0 请求时 `0.0%`） | Activity |
| 2 | 今日 Token | compact token 值 | 输入/输出/缓存明细 | — | Zap |
| 3 | 今日费用 | 原始 USD 费用 | 实际结算 USD 费用 | 节省 > 0 时在 extraBadge 显示 `节省 $X` | DollarSign |
| 4 | 全站 RPM / TPM | 最近 60 秒 RPM / TPM | `最近 60 秒` | — | Activity |

Other top-level payload fields: `today`, `api_keys`, `tokens`, `system_health`, `cost_stats`, `cache_stats`, `token_breakdown`. The `users` field was **removed** in the single-user conversion (2026-09); `/api/admin/system/stats` is a different endpoint and keeps its own `users` payload — do not confuse the two.

`system_health.avg_response_time` is seconds for the requested range, calculated as total recorded response time divided by response-time samples, including successes and failures. The frontend-only fifth card fixes the request to `preset=today`, displays two decimal seconds, and uses `system_health.total_requests` to distinguish no requests from a measured zero. Missing or invalid response values remain unavailable; trend filters must not change this card. The deleted monthly health section stays deleted.

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
- Frontend `frontend/src/views/shared/__tests__/Dashboard.spec.ts` covers trend interactions, retained historical totals and today's average response: explicit today/timezone parameters, fixed range, seconds, empty/missing/error states. Keep the five frontend placeholders aligned with rendered cards.

### 7. Wrong vs Correct

#### Wrong

Adding a card only in the backend and assuming the frontend "will show it" — empty-state placeholders and `DashboardStatsResponse` docs go stale, and the empty dashboard shows the old card set.

#### Correct

Keep the four-card API array stable; update backend json! block, integration assertions and frontend contracts together when changing it. A derived frontend card can reuse existing summary fields without adding a backend card.

## Daily model and provider breakdown (`/api/dashboard/daily-stats`)

- `StoredUsageDashboardDailyBreakdownRow.kind` distinguishes raw `Usage` (totals and dimensions), retained `Totals` (totals only), and `Breakdown` (dimensions only). Never encode a total as a synthetic model/provider named `aggregate`, or remove a real business name with a string blacklist.
- SQLite reads totals, `stats_daily_model_provider` / `stats_user_daily_model_provider`, and grouped raw usage in one read transaction. Every source applies the same user scope. Complete raw UTC days take precedence and can be regrouped in the requested timezone; retained rollups keep their stored UTC date labels when raw history is incomplete.
- Resolve coverage per day and model/provider group. Raw records fill gaps between retained dates and groups missing from old backups. A latest-aggregate cutoff loses historical holes; adding both a retained group and its raw records double-counts the dimension.
- Retained totals and grouped rows are independent: grouped tables do not retain actual cost, so daily actual cost and total requests must be read from totals exactly once. Partial raw history must not replace a larger retained daily total.
- Each daily response includes `unattributed_requests` and `unattributed_cost` for the retained remainder without model/provider details. It remains in daily totals, but is excluded from actual model/provider summaries and unique counts. The frontend labels the missing portion separately and explains the incomplete history. Never guess a provider or allocate its cost proportionally.
- Chart labels preserve full stored model names. Prefix stripping can make different real models look like another model.
- Regression evidence: SQLite usage tests cover saved dimensions, full raw days/timezone, partial history, retained-date gaps, user isolation and totals-only local-day queries. Gateway daily tests assert no double totals, real unique counts and the missing remainder; Dashboard tests verify real ranking names, preserved historical costs, rounded-cost differences and the distinction between totals and dimensions.

- Frontend lifetime totals read the existing daily endpoint from `1970-01-01` to the current local date and sum its daily totals once. Do not reuse the default usage-stats window or dashboard monthly aggregation as all-time history. Daily costs retain the endpoint's four-decimal precision.
- Period rankings use `model_summary` / `provider_summary`, whose costs are accumulated before rounding; summing already-rounded daily model costs loses small requests and can reorder the ranking. `unattributed_cost` is computed by the server before rounding: subtracting rounded row costs in the client can manufacture missing history.
