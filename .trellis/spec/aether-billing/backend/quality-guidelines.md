# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

<!--
Document your project's quality standards here.

Questions to answer:
- What patterns are forbidden?
- What linting rules do you enforce?
- What are your testing requirements?
- What code review standards apply?
-->

(To be filled by the team)

---

## Design Decision: Provider Key Cost Multiplier Has a Single Source of Truth

**Context** (2026-09, task `09-03-key-multiplier-unify`): a provider key used to carry two multiplier fields — `default_rate_multiplier` (key-level) and `rate_multipliers` (per-API-format overrides) — with billing resolving format-override first. This split the UI: the key list inline edit wrote `rate_multipliers[format]` while the edit dialog showed `default_rate_multiplier`, so the list could show `0.15` while the dialog showed `1`.

**Decision**: the key-level `default_rate_multiplier` is the only live multiplier. Billing (`aether-billing/src/pricing.rs::rate_multiplier_for_api_format`) and cost-based ranking (gateway `candidate_ranking.rs`) read ONLY this field; invalid values (negative / NaN / missing) fall back to `1.0`. Per-format overrides are NOT read anywhere.

**Legacy field contract** (`rate_multipliers`):
- DB column and API payload field are kept (no migration) but are inert: no consumer in billing, ranking, or UI.
- The admin key-update API still accepts the field so clients can clear it: explicit `null` → stored NULL; omitted (`undefined`) → value preserved (same semantics as `allowed_models`).
- The key form dialog submits `rate_multipliers: null` on every update to purge legacy stored overrides; create omits the field.

**Wrong vs Correct**:
```rust
// Wrong: resurrect the per-format map in any cost path
let m = ctx.provider_api_key_rate_multipliers.and_then(|map| map.get(format));
// Correct: key default only, sanitized
let m = ctx.provider_api_key_default_rate_multiplier.filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(1.0);
```

**Tests required**: billing must keep (a) a test proving the map is ignored even when present (`rate_multiplier_format_mapping_is_ignored_in_favor_of_key_default`), (b) a key-default-wins end-to-end cost test (default `0.15` beats conflicting map), (c) invalid-default fallback to 1.0. The shared service-test pricing fixture deliberately carries a decoy `{"openai:chat": 0.5}` map so any reintroduction of map consumption fails the suite.

**Known asymmetry (accepted)**: dialog validation allows `0–100` while the list inline edit allows `0.01–100`; a default of `0` bills at zero cost but the cost ranker treats it as neutral `1.0`.

---

## Forbidden Patterns

<!-- Patterns that should never be used and why -->

(To be filled by the team)

---

## Required Patterns

<!-- Patterns that must always be used -->

(To be filled by the team)

---

## Testing Requirements

<!-- What level of testing is expected -->

(To be filled by the team)

---

## Code Review Checklist

<!-- What reviewers should check -->

(To be filled by the team)
