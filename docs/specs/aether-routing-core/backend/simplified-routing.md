# Simplified Routing and Configuration Round Trip

## 1. Scope / Trigger

Applies to the personal single-strategy UI and gateway resolver. The public
complete resolver remains a separate compatibility contract.

## 2. Signatures

- `resolve_routing_policy_simplified(&RoutingGroupConfig, RoutingPolicyInput) -> Result<ResolvedRoutingPolicy, RoutingPolicyError>`
- `resolve_routing_policy(&RoutingGroupConfig, RoutingPolicyInput)` keeps complete semantics.
- Frontend: `buildSchedulingStrategyConfig(mode, orderedProviderIds, baseConfig?)`;
  callers must pass the existing group's configuration when editing it.

Sources: `crates/aether-routing-core/src/policy.rs`, gateway
`src/routing/resolver.rs`, and frontend
`src/features/routing/utils/schedulingStrategy.ts`.

## 3. Contracts

- Simplified resolution ignores `allowed_models`, `model_policies`,
  `RestrictModels`, `GlobalKey`, and `SetKeyPriority` overlays. Provider ordering
  is authoritative; a key's entity `internal_priority` remains effective.
- Normalize legacy `load_balance` to `cache_affinity`, including gateway's
  static-default shortcut and rule actions.
- The exact `ui_model_scheduling:` ID namespace belongs to the retired per-model
  UI helper. Ignore its `SetScheduling` actions in simplified resolution and
  remove those actions on UI save. Keep its other actions and stop gate.
- Preserve surviving rules' conditions, phases, action order and
  `stop_processing`. Header/body mutations and non-retired external scheduling
  actions still execute under their original conditions.
- The UI owns the unconditional enabled `client_request` provider-priority rule
  with ID `ui_provider_priority` (or a generated numeric collision suffix).
  Conditional, disabled and other-phase rules are not global UI priorities.
- Saving edits the managed rule against a copy of the original configuration;
  it must not reconstruct `rules` from only the visible UI fields.
- A new global provider-priority rule executes before surviving client-request
  rules, which the backend sorts by `(priority, id)`. If `i32::MIN` is occupied,
  move only consecutive occupied client-request priority groups up one slot
  through the first gap. Preserve ties, relative execution order, conditions,
  stop gates, other phases, and the caller's original configuration.

## 4. Validation & Error Matrix

| Configuration | Simplified behavior |
|---|---|
| Retired model restrictions or key overlay | Readable, no effect on selection/ranking |
| `GlobalKey` / `load_balance` | Provider ordering / cache affinity |
| Similar external ID without exact retired namespace | Preserve action semantics |
| Surviving conditional header/body rule | Evaluate condition and phase normally |
| Empty rule with `stop_processing` | Keep the stop gate |
| Malformed routing config | Existing validation error; no fabricated success |

## 5. Good / Base / Bad Cases

- Good: change default mode while retaining a conditional header mutation.
- Base: absent config creates one managed global provider-priority rule.
- Bad: read all `SetProviderPriority` actions as global, drop unexposed rules on
  save, or disable the key entity's priority along with retired overlays.

## 6. Tests Required

Use existing `policy.rs` simplified/complete regressions, gateway static-resolver
tests, and `schedulingStrategy.spec.ts`. Assert legacy normalization, entity key
priority, mutations/conditions/phase/stop behavior, input immutability, and
save/parse round trips. Compare the shortcut directly with the core simplified
resolver, not with another call that can hit the same shortcut.

## 7. Wrong vs Correct

Wrong: `rules: [priorityRule]` when saving an existing group. Correct: preserve
the base rules, remove only retired actions, and update the managed provider
actions at their existing position.
