# External Model Catalog Reliability

## 1. Scope / Trigger

This contract applies to `GET /api/admin/models/external`, its explicit cache
clear route, the browser-side models.dev cache, create-model presets, and online
price synchronization. Read it before changing retry, cache freshness, external
catalog proxying, or UI empty/error states.

## 2. Signatures

Backend:

```text
GET    /api/admin/models/external
DELETE /api/admin/models/external/cache
```

Frontend (`frontend/src/api/models-dev.ts`):

```ts
getModelsDevListWithStatus(officialOnly?: boolean):
  Promise<{ models: ModelsDevModelItem[]; stale: boolean }>
refreshModelsDevList(officialOnly?: boolean): Promise<ModelsDevModelItem[]>
```

## 3. Contracts

- The gateway cache and browser cache use a 15-minute freshness window.
- A gateway cache miss may retry at most three attempts with bounded short
  delays. Retry only explicit transient transport categories and HTTP 5xx.
- HTTP 4xx, invalid JSON, invalid proxy/configuration, request construction,
  response-size, and unclassified failures are deterministic for this request
  and must not be retried.
- The gateway preserves the existing final `503` route contract after retry
  exhaustion and keeps cache entries isolated by the configured proxy node.
- Create-model preset loading may fall back to the newest structurally
  compatible expired browser cache. It must return `stale: true`, show a visible
  cache notice, and retain retry and manual-entry actions.
- Online price synchronization is strict: clear the gateway cache, force a new
  catalog fetch, and reject on failure. It must never apply or label expired
  browser data as the latest online price. A failed strict refresh keeps the
  last successful browser cache for later preset fallback without consuming it.
- Transport logs record request ID, attempt count, status and sanitized failure
  category. Do not log credentials, tokens, URL userinfo, query strings, or
  fragments.

## 4. Validation & Error Matrix

| Condition | Backend action | Create-model UI | Price sync |
| --- | --- | --- | --- |
| Fresh gateway/browser cache | Return cached payload | Show presets | Strict refresh still bypasses browser cache |
| First 5xx, later success | Retry, cache success | Show fresh presets | Apply fresh price |
| Transient transport failure, retries exhausted | Return existing unavailable contract | Use compatible expired cache with `stale: true`, else error state | Reject; do not update model |
| HTTP 4xx | No retry | Stale fallback or error state | Reject |
| Invalid JSON or incompatible browser cache | No retry / discard candidate | Error state, not empty state | Reject |
| Successful empty catalog | Return empty payload | Show true empty state | No matching price |

## 5. Good / Base / Bad Cases

- Good: a temporary 503 is retried and the second response populates both
  caches; the user sees normal presets.
- Base: a successful but empty catalog displays `暂无可用模型`.
- Bad: swallowing a 503 and rendering `暂无可用模型`, or clearing the only
  browser fallback before a strict refresh that later fails.

## 6. Tests Required

- Backend: assert `503 -> 200` returns data, persistent 5xx stops at the attempt
  limit, and 4xx is attempted once.
- Frontend API: assert an expired compatible cache returns `stale: true`; a
  failed strict refresh rejects without replacing the saved cache; a successful
  strict refresh replaces it.
- Component: assert no-cache failure shows error/retry/manual entry, stale
  fallback shows the cache notice and presets, retry can recover, and failed
  price sync does not call the model update API.
- Browser: inspect fresh, stale and error states at `/admin/models`; error and
  stale states must remain visually distinct from the successful empty state.

## 7. Wrong vs Correct

Wrong:

```ts
clearModelsDevCache()
return getModelsDevList(false).catch(() => [])
```

This destroys the fallback, conflates failure with a valid empty catalog, and
allows callers to lose freshness information.

Correct:

```ts
const result = await getModelsDevListWithStatus(false) // preset browsing
const fresh = await refreshModelsDevList(false)        // price synchronization
```

The two entry points state their freshness requirements explicitly.
