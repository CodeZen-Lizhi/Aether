import { describe, expect, it } from 'vitest'

import {
  createEmptyRoutingGroupConfig,
  modelSchedulingRuleId,
  upsertModelSchedulingRule,
  type RoutingGroupConfig,
} from '../utils/routingPolicy'
import {
  buildSchedulingStrategyConfig,
  findSystemDefaultRoutingGroup,
  normalizeSchedulingMode,
  parseSchedulingStrategy,
  prioritiesFromOrder,
} from '../utils/schedulingStrategy'

describe('normalizeSchedulingMode', () => {
  it('maps legacy load_balance to cache_affinity (R10 soft delete)', () => {
    expect(normalizeSchedulingMode('load_balance')).toBe('cache_affinity')
  })

  it('keeps fixed_order and cost_based, defaults unknown to cache_affinity', () => {
    expect(normalizeSchedulingMode('fixed_order')).toBe('fixed_order')
    expect(normalizeSchedulingMode('cost_based')).toBe('cost_based')
    expect(normalizeSchedulingMode('cache_affinity')).toBe('cache_affinity')
    expect(normalizeSchedulingMode(undefined)).toBe('cache_affinity')
    expect(normalizeSchedulingMode('nonsense')).toBe('cache_affinity')
  })
})

describe('parseSchedulingStrategy', () => {
  it('reads only the managed global provider priorities', () => {
    const config: RoutingGroupConfig = {
      allowed_models: [],
      default_policy: {
        priority_mode: 'provider',
        scheduling_mode: 'cost_based',
        keep_priority_on_conversion: false,
      },
      model_policies: [],
      rules: [
        {
          id: 'legacy',
          priority: 2,
          enabled: false,
          phase: 'client_request',
          conditions: {},
          actions: [{ type: 'set_provider_priority', provider_id: 'p-disabled', priority: 9 }],
          stop_processing: false,
        },
        {
          id: 'ui_provider_priority',
          priority: 1,
          enabled: true,
          phase: 'client_request',
          conditions: {},
          actions: [
            { type: 'set_provider_priority', provider_id: 'p-a', priority: 1 },
            { type: 'set_provider_priority', provider_id: 'p-b', priority: 2 },
            { type: 'set_key_priority', key_id: 'k-1', priority: 1 },
          ],
          stop_processing: false,
        },
        {
          id: 'conditional-priorities',
          priority: 3,
          enabled: true,
          phase: 'client_request',
          conditions: { field: 'headers.x-region', op: 'eq', value: 'eu' },
          actions: [{ type: 'set_provider_priority', provider_id: 'p-a', priority: 20 }],
          stop_processing: true,
        },
      ],
    }

    const state = parseSchedulingStrategy(config)
    expect(state.mode).toBe('cost_based')
    expect(state.providerPriorities).toEqual({ 'p-a': 1, 'p-b': 2 })
    expect(state).not.toHaveProperty('keyPriorities')
  })

  it('returns empty state for null config', () => {
    expect(parseSchedulingStrategy(null)).toEqual({
      mode: 'cache_affinity',
      providerPriorities: {},
    })
  })
})

describe('buildSchedulingStrategyConfig', () => {
  it('builds a round-trippable config', () => {
    const config = buildSchedulingStrategyConfig('cost_based', ['p-a', 'p-b'])
    expect(config.default_policy.scheduling_mode).toBe('cost_based')
    expect(config.default_policy.priority_mode).toBe('provider')
    expect(config.allowed_models).toEqual([])
    expect(config.model_policies).toEqual([])

    const state = parseSchedulingStrategy(config)
    expect(state.mode).toBe('cost_based')
    expect(state.providerPriorities).toEqual({ 'p-a': 1, 'p-b': 2 })
    expect(buildSchedulingStrategyConfig('cost_based', ['p-a', 'p-b'], config)).toEqual(config)
  })

  it('preserves rule conditions, phases, order and stop gates while retiring old dimensions', () => {
    const base = createEmptyRoutingGroupConfig()
    base.allowed_models = ['legacy-model']
    base.model_policies = [{
      model: 'legacy-model',
      allowed_providers: ['p-legacy'],
      allowed_keys: ['k-legacy'],
      provider_priority_overrides: { 'p-legacy': 10 },
      key_priority_overrides: { 'k-legacy': 5 },
    }]
    base.default_policy.priority_mode = 'global_key'
    base.default_policy.scheduling_mode = 'load_balance'
    base.rules = [
      {
        // Even a familiar ID must not turn a conditional rule into a global rule.
        id: 'ui_provider_priority',
        priority: 0,
        enabled: true,
        phase: 'client_request',
        conditions: { all: [{ field: 'headers.x-region', op: 'eq', value: 'eu' }] },
        actions: [
          { type: 'patch_headers', patch: [{ op: 'set', name: 'x-before', value: 'kept' }] },
          { type: 'set_provider_priority', provider_id: 'p-b', priority: 7 },
          { type: 'set_key_priority', key_id: 'k-legacy', priority: 1 },
          { type: 'restrict_models', models: ['legacy-model'] },
          { type: 'set_scheduling', priority_mode: 'global_key', scheduling_mode: 'load_balance' },
          { type: 'json_patch_body', patch: [{ op: 'add', path: '/metadata', value: {} }] },
        ],
        stop_processing: true,
      },
      {
        id: 'provider-patch',
        priority: 5,
        enabled: true,
        phase: 'provider_request',
        conditions: { field: 'api_format', op: 'eq', value: 'openai:chat' },
        actions: [{ type: 'patch_headers', patch: [{ op: 'remove', name: 'x-private' }] }],
        stop_processing: false,
      },
      {
        id: 'disabled-patch',
        priority: 10,
        enabled: false,
        phase: 'client_request',
        conditions: {},
        actions: [{ type: 'json_patch_body', patch: [{ op: 'remove', path: '/metadata' }] }],
        stop_processing: true,
      },
    ]
    const original = JSON.stringify(base)
    expect(parseSchedulingStrategy(base).providerPriorities).toEqual({})

    const config = buildSchedulingStrategyConfig('cache_affinity', ['p-a', 'p-b'], base)
    expect(config.allowed_models).toEqual([])
    expect(config.model_policies).toEqual([])
    expect(config.default_policy.priority_mode).toBe('provider')
    expect(config.rules.slice(0, 3)).toEqual([
      {
        ...base.rules[0],
        actions: [
          base.rules[0].actions[0],
          base.rules[0].actions[1],
          { type: 'set_scheduling', priority_mode: 'provider', scheduling_mode: 'cache_affinity' },
          base.rules[0].actions[5],
        ],
      },
      base.rules[1],
      base.rules[2],
    ])
    const globalRule = config.rules[3]
    expect(globalRule.id).not.toBe(base.rules[0].id)
    expect(globalRule.conditions).toEqual({})
    expect(globalRule.priority).toBeLessThan(base.rules[0].priority)
    expect(parseSchedulingStrategy(config).providerPriorities).toEqual({ 'p-a': 1, 'p-b': 2 })
    expect(JSON.stringify(base)).toBe(original)

    // Subsequent saves update only managed actions, preserving mixed rules.
    globalRule.actions.push({ type: 'patch_headers', patch: [{ op: 'set', name: 'x-ui', value: 'kept' }] })
    globalRule.stop_processing = true
    const resaved = buildSchedulingStrategyConfig('fixed_order', ['p-b', 'p-a'], config)
    expect(resaved.rules.slice(0, 3)).toEqual(config.rules.slice(0, 3))
    expect(resaved.rules[3]).toEqual({
      ...globalRule,
      actions: [
        { type: 'set_provider_priority', provider_id: 'p-b', priority: 1 },
        { type: 'set_provider_priority', provider_id: 'p-a', priority: 2 },
        globalRule.actions[2],
      ],
    })

    // Use the actual legacy UI generator, including its encoded model ID.
    const legacyConfig = upsertModelSchedulingRule(config, 'gpt/5', {
      priority_mode: 'provider',
      scheduling_mode: 'fixed_order',
    })
    const legacyRule = legacyConfig.rules.find(rule => rule.id === modelSchedulingRuleId('gpt/5'))!
    const retainedPatch = { type: 'patch_headers', patch: [{ op: 'set', name: 'x-model', value: 'kept' }] }
    legacyRule.actions.push(retainedPatch)
    legacyRule.stop_processing = true
    const externalRule = { ...legacyRule, id: 'ui_model_scheduling_custom:gpt%2F5' }
    legacyConfig.rules.push(externalRule)

    const normalized = buildSchedulingStrategyConfig('cache_affinity', ['p-a', 'p-b'], legacyConfig)
    expect(parseSchedulingStrategy(legacyConfig).mode).toBe('cache_affinity')
    expect(normalized.rules.find(rule => rule.id === legacyRule.id)).toEqual({
      ...legacyRule,
      actions: [retainedPatch],
    })
    expect(normalized.rules.find(rule => rule.id === externalRule.id)).toEqual(externalRule)

    const minimumPriority = -2_147_483_648
    const boundaryBase: RoutingGroupConfig = {
      ...base,
      rules: [
        { ...base.rules[0], priority: minimumPriority },
        { ...base.rules[1], priority: minimumPriority },
        { ...base.rules[2], priority: minimumPriority + 1 },
        { ...base.rules[0], id: 'a-before-stop', priority: minimumPriority, stop_processing: false },
        { ...base.rules[0], id: 'next-group', priority: minimumPriority + 1 },
        { ...base.rules[0], id: 'later-group', priority: minimumPriority + 3 },
      ],
    }
    const boundarySnapshot = JSON.stringify(boundaryBase)
    const boundary = buildSchedulingStrategyConfig('cache_affinity', ['p-a'], boundaryBase)
    const order = (candidate: RoutingGroupConfig) => candidate.rules
      .filter(rule => rule.enabled && rule.phase === 'client_request')
      .sort((left, right) => {
        if (left.priority !== right.priority) return left.priority - right.priority
        if (left.id < right.id) return -1
        if (left.id > right.id) return 1
        return 0
      })
      .map(rule => rule.id)
    expect(order(boundary)).toEqual([boundary.rules[6].id, ...order(boundaryBase)])
    expect(boundary.rules.slice(0, 6).map(rule => rule.priority)).toEqual([
      minimumPriority + 1,
      minimumPriority,
      minimumPriority + 2,
      minimumPriority + 1,
      minimumPriority + 2,
      minimumPriority + 3,
    ])
    expect(boundary.rules[0].stop_processing).toBe(true)
    expect(boundary.rules[3].stop_processing).toBe(false)
    expect(boundary.rules.slice(0, 6).map(rule => rule.conditions)).toEqual(boundaryBase.rules.map(rule => rule.conditions))
    expect(JSON.stringify(boundaryBase)).toBe(boundarySnapshot)
  })
})

describe('findSystemDefaultRoutingGroup', () => {
  it('prefers the enabled system default, then any enabled, then null', () => {
    const groups = [
      { id: 'a', is_system_default: false, enabled: true },
      { id: 'b', is_system_default: true, enabled: false },
      { id: 'c', is_system_default: true, enabled: true },
    ]
    expect(findSystemDefaultRoutingGroup(groups)?.id).toBe('c')
    expect(
      findSystemDefaultRoutingGroup([
        { id: 'a', is_system_default: false, enabled: true },
        { id: 'b', is_system_default: true, enabled: false },
      ])?.id,
    ).toBe('a')
    expect(
      findSystemDefaultRoutingGroup([{ id: 'b', is_system_default: true, enabled: false }]),
    ).toBeNull()
  })
})

describe('prioritiesFromOrder', () => {
  it('assigns 1-based priorities in array order', () => {
    expect(prioritiesFromOrder(['x', 'y', 'z'])).toEqual({ x: 1, y: 2, z: 3 })
    expect(prioritiesFromOrder([])).toEqual({})
  })
})
