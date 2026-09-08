import {
  isGeneratedModelSchedulingRule,
  normalizeRoutingGroupConfig,
  type RoutingGroupConfig,
  type RoutingRule,
} from './routingPolicy'

/**
 * R11-8: 调度策略单页形态的纯逻辑。
 *
 * 页面只编辑默认调度模式和自己的无条件供应商优先级规则。
 * 外部规则保留条件与执行顺序；Key 优先级由 Key 实体管理。
 * 保存时清除 R11 退役维度，load_balance 归一为 cache_affinity。
 */

export type SchedulingStrategyMode = 'cache_affinity' | 'fixed_order' | 'cost_based'

export const PROVIDER_PRIORITY_RULE_ID = 'ui_provider_priority'
const MIN_RULE_PRIORITY = -2_147_483_648

export interface SchedulingStrategyState {
  mode: SchedulingStrategyMode
  /** provider_id -> priority（1 起，越小越靠前；未配置的供应商不在表内） */
  providerPriorities: Record<string, number>
}

export function normalizeSchedulingMode(mode: string | undefined | null): SchedulingStrategyMode {
  if (mode === 'fixed_order' || mode === 'cost_based') {
    return mode
  }
  // R10 soft delete: legacy load_balance reads as cache_affinity.
  return 'cache_affinity'
}

function isAction(action: unknown, type: string): action is Record<string, unknown> {
  return action !== null
    && typeof action === 'object'
    && !Array.isArray(action)
    && 'type' in action
    && action.type === type
}

function isManagedProviderPriorityRule(rule: RoutingRule): boolean {
  const prefix = `${PROVIDER_PRIORITY_RULE_ID}:`
  const managedId = rule.id === PROVIDER_PRIORITY_RULE_ID
    || (rule.id.startsWith(prefix) && /^\d+$/.test(rule.id.slice(prefix.length)))
  const conditions = rule.conditions
  const unconditional = conditions === undefined
    || (conditions !== null
      && typeof conditions === 'object'
      && !Array.isArray(conditions)
      && Object.keys(conditions).length === 0)
  return managedId
    && rule.enabled !== false
    && (rule.phase === undefined || rule.phase === 'client_request')
    && unconditional
}

function collectProviderPriorities(rule: RoutingRule | undefined): Record<string, number> {
  const result: Record<string, number> = {}
  for (const action of rule?.actions ?? []) {
    if (!isAction(action, 'set_provider_priority')) continue
    const { provider_id: id, priority } = action
    if (typeof id === 'string' && id && typeof priority === 'number' && Number.isFinite(priority)) {
      result[id] = priority
    }
  }
  return result
}

export function parseSchedulingStrategy(
  config: RoutingGroupConfig | null | undefined,
): SchedulingStrategyState {
  return {
    mode: normalizeSchedulingMode(config?.default_policy?.scheduling_mode),
    providerPriorities: collectProviderPriorities(config?.rules?.find(isManagedProviderPriorityRule)),
  }
}

function normalizeSurvivingActions(rule: RoutingRule): unknown[] {
  return (rule.actions ?? []).flatMap((action) => {
    if (isAction(action, 'restrict_models') || isAction(action, 'set_key_priority')) {
      return []
    }
    if (isAction(action, 'set_scheduling')) {
      if (isGeneratedModelSchedulingRule(rule)) return []
      return [{
        ...action,
        priority_mode: 'provider',
        ...(action.scheduling_mode === 'load_balance' ? { scheduling_mode: 'cache_affinity' } : {}),
      }]
    }
    return [action]
  })
}

export function buildSchedulingStrategyConfig(
  mode: SchedulingStrategyMode,
  orderedProviderIds: string[],
  baseConfig?: RoutingGroupConfig | null,
): RoutingGroupConfig {
  const config = normalizeRoutingGroupConfig(baseConfig)
  const priorityActions = orderedProviderIds.map((providerId, index) => ({
    type: 'set_provider_priority',
    provider_id: providerId,
    priority: index + 1,
  }))
  const rules = config.rules.map(rule => ({
    ...rule,
    // Keep even an empty rule: its stop_processing gate may still matter.
    actions: normalizeSurvivingActions(rule),
  }))
  const priorityRule = rules.find(isManagedProviderPriorityRule)
  if (priorityRule) {
    let replaced = false
    priorityRule.actions = priorityRule.actions.flatMap((action) => {
      if (!isAction(action, 'set_provider_priority')) return [action]
      if (replaced) return []
      replaced = true
      return priorityActions
    })
    if (!replaced) priorityRule.actions.push(...priorityActions)
  } else {
    // A conditional rule may already use the familiar ID. Keep it intact and
    // install a separate global default before the existing request rules.
    let id = PROVIDER_PRIORITY_RULE_ID
    for (let suffix = 1; rules.some(rule => rule.id === id); suffix++) {
      id = `${PROVIDER_PRIORITY_RULE_ID}:${suffix}`
    }
    const requestRules = rules.filter(rule => rule.phase === undefined || rule.phase === 'client_request')
    const firstPriority = requestRules.reduce((minimum, rule) => Math.min(minimum, rule.priority ?? 0), 2)
    if (firstPriority === MIN_RULE_PRIORITY) {
      // Rules sort by (priority, id), so array position cannot break a tie.
      // Move only consecutive occupied groups, preserving their execution order.
      const occupied = new Set(requestRules.map(rule => rule.priority ?? 0))
      let firstFreePriority = MIN_RULE_PRIORITY
      while (occupied.has(firstFreePriority)) firstFreePriority++
      for (const rule of requestRules) {
        const priority = rule.priority ?? 0
        if (priority < firstFreePriority) rule.priority = priority + 1
      }
    }
    rules.push({
      id,
      priority: Math.max(MIN_RULE_PRIORITY, firstPriority - 1),
      enabled: true,
      phase: 'client_request',
      conditions: {},
      actions: priorityActions,
      stop_processing: false,
    })
  }

  return {
    allowed_models: [],
    default_policy: {
      ...config.default_policy,
      priority_mode: 'provider',
      scheduling_mode: mode,
      keep_priority_on_conversion: false,
    },
    model_policies: [],
    rules,
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
