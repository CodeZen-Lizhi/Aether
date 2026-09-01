import { describe, expect, it } from 'vitest'

import type { RoutingGroupConfig } from '../utils/routingPolicy'
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
  it('collects provider and key priorities across rules', () => {
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
      ],
    }

    const state = parseSchedulingStrategy(config)
    expect(state.mode).toBe('cost_based')
    expect(state.providerPriorities).toEqual({ 'p-a': 1, 'p-b': 2 })
    expect(state.keyPriorities).toEqual({ 'k-1': 1 })
  })

  it('returns empty state for null config', () => {
    expect(parseSchedulingStrategy(null)).toEqual({
      mode: 'cache_affinity',
      providerPriorities: {},
      keyPriorities: {},
    })
  })
})

describe('buildSchedulingStrategyConfig', () => {
  it('builds a round-trippable config', () => {
    const config = buildSchedulingStrategyConfig('cost_based', ['p-a', 'p-b'], { 'k-1': 2 })
    expect(config.default_policy.scheduling_mode).toBe('cost_based')
    expect(config.default_policy.priority_mode).toBe('provider')
    expect(config.allowed_models).toEqual([])
    expect(config.model_policies).toEqual([])

    const state = parseSchedulingStrategy(config)
    expect(state.mode).toBe('cost_based')
    expect(state.providerPriorities).toEqual({ 'p-a': 1, 'p-b': 2 })
    expect(state.keyPriorities).toEqual({ 'k-1': 2 })
  })

  it('drops non-positive key priorities', () => {
    const config = buildSchedulingStrategyConfig('cache_affinity', ['p-a'], {
      'k-keep': 3,
      'k-drop': 0,
      'k-drop-negative': -2,
    })
    const state = parseSchedulingStrategy(config)
    expect(state.keyPriorities).toEqual({ 'k-keep': 3 })
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
