import type { RoutingGroupConfig, RoutingRule } from './routingPolicy'

/**
 * R11-8: 调度策略单页形态的纯逻辑。
 *
 * 单份策略 = 一个调度模式（default_policy.scheduling_mode）+ 一条
 * `ui_provider_priority` 规则（set_provider_priority / set_key_priority
 * 动作承载供应商与 Key 的优先级）。解析与构建互为逆操作；旧配置里的
 * load_balance 读取时映射为 cache_affinity（R10 软删），区分模型维度与
 * 模型白名单（R11-1/R11-2）在构建时不再产出、解析时直接忽略。
 */

export type SchedulingStrategyMode = 'cache_affinity' | 'fixed_order' | 'economy'

export const PROVIDER_PRIORITY_RULE_ID = 'ui_provider_priority'

export interface SchedulingStrategyState {
  mode: SchedulingStrategyMode
  /** provider_id -> priority（1 起，越小越靠前；未配置的供应商不在表内） */
  providerPriorities: Record<string, number>
  /** key_id -> priority（同上，来自 set_key_priority overlay） */
  keyPriorities: Record<string, number>
}

export function normalizeSchedulingMode(mode: string | undefined | null): SchedulingStrategyMode {
  if (mode === 'fixed_order' || mode === 'economy') {
    return mode
  }
  // R10 soft delete: legacy load_balance reads as cache_affinity.
  return 'cache_affinity'
}

function collectPriorityActions(
  rules: RoutingRule[],
  actionType: 'set_provider_priority' | 'set_key_priority',
): Record<string, number> {
  const result: Record<string, number> = {}
  for (const rule of rules) {
    if (!rule.enabled) continue
    for (const action of rule.actions) {
      if (
        action
        && typeof action === 'object'
        && (action as { type?: unknown }).type === actionType
      ) {
        const typed = action as { provider_id?: unknown; key_id?: unknown; priority?: unknown }
        const id = typeof typed.provider_id === 'string'
          ? typed.provider_id
          : typeof typed.key_id === 'string'
            ? typed.key_id
            : null
        const priority = typeof typed.priority === 'number' ? typed.priority : null
        if (id && priority !== null && Number.isFinite(priority)) {
          result[id] = priority
        }
      }
    }
  }
  return result
}

export function parseSchedulingStrategy(
  config: RoutingGroupConfig | null | undefined,
): SchedulingStrategyState {
  if (!config) {
    return { mode: 'cache_affinity', providerPriorities: {}, keyPriorities: {} }
  }
  return {
    mode: normalizeSchedulingMode(config.default_policy?.scheduling_mode),
    providerPriorities: collectPriorityActions(config.rules ?? [], 'set_provider_priority'),
    keyPriorities: collectPriorityActions(config.rules ?? [], 'set_key_priority'),
  }
}

export function buildSchedulingStrategyConfig(
  mode: SchedulingStrategyMode,
  orderedProviderIds: string[],
  keyPriorities: Record<string, number>,
): RoutingGroupConfig {
  const actions: Array<Record<string, unknown>> = orderedProviderIds.map((providerId, index) => ({
    type: 'set_provider_priority',
    provider_id: providerId,
    priority: index + 1,
  }))
  for (const [keyId, priority] of Object.entries(keyPriorities)) {
    if (Number.isFinite(priority) && priority > 0) {
      actions.push({ type: 'set_key_priority', key_id: keyId, priority })
    }
  }

  const priorityRule: RoutingRule = {
    id: PROVIDER_PRIORITY_RULE_ID,
    priority: 1,
    enabled: true,
    phase: 'client_request',
    conditions: {},
    actions,
    stop_processing: false,
  }

  return {
    allowed_models: [],
    default_policy: {
      priority_mode: 'provider',
      scheduling_mode: mode,
      keep_priority_on_conversion: false,
    },
    model_policies: [],
    rules: [priorityRule],
  }
}

export function findSystemDefaultRoutingGroup<
  T extends { is_system_default: boolean; enabled: boolean },
>(groups: T[]): T | null {
  return groups.find(group => group.is_system_default && group.enabled)
    ?? groups.find(group => group.enabled)
    ?? null
}

/** 拖拽/上移下移后的本地顺序 → 优先级表（1 起，按数组顺序递增）。 */
export function prioritiesFromOrder(orderedProviderIds: string[]): Record<string, number> {
  const result: Record<string, number> = {}
  orderedProviderIds.forEach((providerId, index) => {
    result[providerId] = index + 1
  })
  return result
}
