import { afterEach, describe, expect, it, vi } from 'vitest'
import { createApp, defineComponent, h, nextTick, type App, shallowRef } from 'vue'

import type { CandidateRecord, RequestTrace } from '@/api/requestTrace'
import HorizontalRequestTimeline from '../HorizontalRequestTimeline.vue'

const requestTraceApiMock = vi.hoisted(() => ({
  getRequestTrace: vi.fn(),
}))

vi.mock('@/api/requestTrace', () => ({
  requestTraceApi: requestTraceApiMock,
}))

vi.mock('@/components/ui/card.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'CardStub',
      setup(_, { slots }) {
        return () => h('section', slots.default?.())
      },
    }),
  }
})

vi.mock('@/components/ui/badge.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'BadgeStub',
      setup(_, { slots }) {
        return () => h('span', slots.default?.())
      },
    }),
  }
})

vi.mock('@/components/ui/skeleton.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'SkeletonStub',
      setup() {
        return () => h('div')
      },
    }),
  }
})

vi.mock('../JsonContentPanel.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'JsonContentPanelStub',
      props: {
        data: {
          type: null,
          default: null,
        },
        title: {
          type: String,
          default: 'JSON',
        },
      },
      setup(props) {
        return () => h('pre', { 'data-title': props.title }, JSON.stringify(props.data))
      },
    }),
  }
})

vi.mock('lucide-vue-next', async () => {
  const { defineComponent, h } = await import('vue')
  const Icon = defineComponent({
    name: 'IconStub',
    setup() {
      return () => h('span')
    },
  })

  return {
    ChevronRight: Icon,
    ExternalLink: Icon,
    KeyRound: Icon,
    TriangleAlert: Icon,
  }
})

const mountedApps: Array<{ app: App, root: HTMLElement }> = []
type TimelineExpose = { refresh: () => Promise<void> | undefined }

function buildCandidate(overrides: Partial<CandidateRecord> = {}): CandidateRecord {
  return {
    id: 'cand-1',
    request_id: 'req-1',
    candidate_index: 0,
    retry_index: 0,
    provider_id: 'provider-1',
    provider_name: 'Provider 1',
    key_id: 'key-1',
    key_name: 'Key 1',
    status: 'failed',
    is_cached: false,
    created_at: '2026-05-06T12:00:00.000Z',
    started_at: '2026-05-06T12:00:00.000Z',
    finished_at: '2026-05-06T12:00:01.000Z',
    ...overrides,
  }
}

function buildTrace(candidates: CandidateRecord[]): RequestTrace {
  return {
    request_id: 'req-1',
    total_candidates: candidates.length,
    final_status: 'success',
    total_latency_ms: 1000,
    candidates,
  }
}

async function flushPendingUpdates() {
  await Promise.resolve()
  await nextTick()
  await Promise.resolve()
  await nextTick()
}

/** 等待 Vue 过渡类在下一宏任务中移除，避免把离场节点误判为仍在展示。 */
async function waitForTransitions() {
  await flushPendingUpdates()
  await new Promise(resolve => setTimeout(resolve, 0))
  await nextTick()
}

/** 通过真实 Key 节点选择一次尝试，覆盖用户可操作路径。 */
function clickAttempt(root: HTMLElement, id: string) {
  const target = root.querySelector<HTMLButtonElement>(`.waterfall-item[data-attempt-id="${id}"] > .attempt-stop`)
    ?? root.querySelector<HTMLButtonElement>(`.attempt-stop[data-attempt-id="${id}"]`)
  target?.click()
}

/** 读取组件实际 emit 的 Key 记录，验证路径加载不会隐式展开或选中。 */
function mountSelectionProbe(traceData: RequestTrace) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const selectedIds: Array<string | null> = []
  const app = createApp(HorizontalRequestTimeline, {
    requestId: traceData.request_id,
    traceData,
    onSelectAttempt: (attempt: CandidateRecord | null) => selectedIds.push(attempt?.id ?? null),
  })
  app.mount(root)
  mountedApps.push({ app, root })
  return { root, selectedIds }
}

/** 通过瀑布行的父容器读取真实 Key ID，避免把属性从按钮移到行后丢失身份。 */
function getAttemptId(node: Element | null): string | null {
  if (!node) return null
  return node.getAttribute('data-attempt-id')
    ?? node.closest('.waterfall-item')?.getAttribute('data-attempt-id')
    ?? node.querySelector('.waterfall-item')?.getAttribute('data-attempt-id')
    ?? null
}

function mountTimeline(
  traceData: RequestTrace,
  extraProps: Record<string, unknown> = {},
  options: { autoOpenDetail?: boolean } = {},
) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  const app = createApp(HorizontalRequestTimeline, {
    requestId: traceData.request_id,
    traceData,
    ...extraProps,
  })
  app.mount(root)
  mountedApps.push({ app, root })
  // 瀑布默认收起；旧交互测试需要先展开目标 Key，才能查看真实错误或进度。
  const candidates = [...traceData.candidates].sort((a, b) => (
    a.candidate_index - b.candidate_index || a.retry_index - b.retry_index
  ))
  const success = candidates.find(candidate => candidate.status === 'success')
  const active = candidates.find(candidate => candidate.status === 'streaming'
    || (candidate.status === 'pending' && Boolean(candidate.started_at)))
  const terminal = [...candidates].reverse().find(candidate => ['failed', 'cancelled', 'stream_interrupted', 'skipped'].includes(candidate.status))
  const target = success ?? active ?? terminal ?? candidates[0]
  if (target && options.autoOpenDetail !== false) clickAttempt(root, target.id)
  return root
}

function mountTimelineFromApi(
  requestId: string,
  extraProps: Record<string, unknown> = {},
) {
  const root = document.createElement('div')
  document.body.appendChild(root)
  let timeline: TimelineExpose | null = null
  const Host = defineComponent({
    setup() {
      return () => h(HorizontalRequestTimeline, {
        ref: (value: unknown) => {
          timeline = value as TimelineExpose | null
        },
        requestId,
        ...extraProps,
      })
    },
  })
  const app = createApp(Host)
  app.mount(root)
  mountedApps.push({ app, root })
  return {
    root,
    refresh: async () => {
      await timeline?.refresh()
      await flushPendingUpdates()
    },
  }
}

afterEach(() => {
  requestTraceApiMock.getRequestTrace.mockReset()
  vi.useRealTimers()
  for (const { app, root } of mountedApps.splice(0)) {
    app.unmount()
    root.remove()
  }
})

describe('HorizontalRequestTimeline', () => {
  it('does not emit or open a detail until a Key is clicked', async () => {
    const { root, selectedIds } = mountSelectionProbe(buildTrace([buildCandidate()]))
    await flushPendingUpdates()

    expect(root.querySelector('.path-key')).not.toBeNull()
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
    expect(selectedIds).toEqual([null])

    clickAttempt(root, 'cand-1')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('cand-1')
    expect(selectedIds).toEqual([null, 'cand-1'])
  })

  it('uses the trace aggregate latency instead of the successful candidate latency', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-transport-timeout',
        provider_id: 'provider-timeout',
        provider_name: 'Provider Timeout',
        key_id: 'key-timeout',
        key_name: 'Timeout Key',
        candidate_index: 0,
        status: 'failed',
        latency_ms: 10_000,
        started_at: '2026-05-06T12:00:00.000Z',
        finished_at: '2026-05-06T12:00:10.000Z',
      }),
      buildCandidate({
        id: 'cand-success-after-failover',
        provider_id: 'provider-success',
        provider_name: 'Provider Success',
        key_id: 'key-success',
        key_name: 'Success Key',
        candidate_index: 1,
        status: 'success',
        latency_ms: 626,
        started_at: '2026-05-06T12:00:10.000Z',
        finished_at: '2026-05-06T12:00:10.626Z',
      }),
    ])
    trace.total_latency_ms = 10_626

    const root = mountTimeline(trace)
    await nextTick()

    const displayedLatency = root.querySelector('[data-trace-latency]')?.textContent?.trim()
    expect(displayedLatency).toBe('10.63s')
    expect(displayedLatency).not.toBe('626ms')
  })

  it('groups providers and keys in a horizontal path while switching the selected detail', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'provider-a-key-1',
        provider_id: 'provider-a',
        provider_name: 'Provider A',
        key_id: 'key-a-1',
        key_name: 'Key A1',
        candidate_index: 0,
        status: 'failed',
        status_code: 500,
      }),
      buildCandidate({
        id: 'provider-a-key-2',
        provider_id: 'provider-a',
        provider_name: 'Provider A',
        key_id: 'key-a-2',
        key_name: 'Key A2',
        candidate_index: 1,
        status: 'failed',
      }),
      buildCandidate({
        id: 'provider-b-key-1',
        provider_id: 'provider-b',
        provider_name: 'Provider B',
        key_id: 'key-b-1',
        key_name: 'Key B1',
        candidate_index: 2,
        status: 'failed',
      }),
      buildCandidate({
        id: 'provider-b-key-2',
        provider_id: 'provider-b',
        provider_name: 'Provider B',
        key_id: 'key-b-2',
        key_name: 'Key B2',
        candidate_index: 3,
        status: 'success',
      }),
    ])

    const root = mountTimeline(trace, {}, { autoOpenDetail: false })
    await nextTick()

    expect(root.querySelectorAll('.provider-record')).toHaveLength(2)
    expect(root.querySelector('[data-route-summary]')?.textContent?.replace(/\s+/g, ' ').trim())
      .toBe('2 家供应商，4 次上游请求')
    expect(root.textContent).not.toContain('选择供应商查看首个 Key')
    expect(root.textContent).not.toContain('第 1 轮')
    expect(root.querySelector('[data-request-conclusion]')).toBeNull()
    expect(root.querySelector('.final-status')).toBeNull()
    expect(root.querySelector('.lane-status')).toBeNull()
    expect([...root.querySelectorAll('.waterfall-provider-order')].map(element => element.textContent?.trim())).toEqual(['01', '02'])
    expect(root.textContent).not.toContain('所有候选均未完成请求')
    expect(root.querySelectorAll('.provider-record')[0]?.textContent).toContain('Key A1')
    expect(root.querySelectorAll('.provider-record')[0]?.textContent).toContain('Key A2')
    expect(root.querySelectorAll('.provider-record')[1]?.textContent).toContain('Key B1')
    expect(root.querySelectorAll('.provider-record')[1]?.textContent).toContain('Key B2')
    expect(root.querySelectorAll('.waterfall-item[data-status="failed"]')).toHaveLength(3)
    expect(root.querySelectorAll('.waterfall-item[data-status="success"]')).toHaveLength(1)
    expect(root.querySelectorAll('.path-provider')).toHaveLength(2)
    expect(root.querySelectorAll('.path-entry')).toHaveLength(1)
    expect(root.querySelectorAll('.path-response')).toHaveLength(1)
    expect(root.querySelectorAll('.path-key')).toHaveLength(4)
    expect(root.querySelectorAll('.path-link--provider')).toHaveLength(1)
    expect(Number(root.querySelector<HTMLElement>('.path-stage')?.style.gridTemplateColumns.match(/minmax/g)?.length)).toBe(7)
    expect(root.querySelector('.attempt-stop')?.getAttribute('data-status')).toBe('failed')
    expect(root.querySelector('.attempt-stop')?.textContent).toContain('500')
    expect(root.querySelector('.attempt-stop')?.textContent).not.toMatch(/失败|成功|HTTP/)
    expect(root.querySelector('.attempt-stop')?.textContent).not.toMatch(/[+−]/)
    expect(root.querySelector('.waterfall-toggle')).toBeNull()
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()

    clickAttempt(root, 'provider-a-key-2')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('provider-a-key-2')
    expect(root.querySelector('[data-execution-inspector]')?.textContent).not.toContain('Provider A')
    expect(root.querySelector('[data-execution-inspector] .waterfall-detail-identity')).toBeNull()
    expect(root.querySelector('.waterfall-item[data-attempt-id="provider-a-key-2"]')?.getAttribute('aria-expanded')).toBe('true')
    expect(root.querySelector('.waterfall-toggle')).toBeNull()
    clickAttempt(root, 'provider-a-key-2')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
  })

  it('orders attempted candidates by scheduling index and omits unattempted candidates', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-success',
        provider_id: 'provider-success',
        provider_name: 'Provider Success',
        key_id: 'key-success',
        key_name: 'Success Key',
        candidate_index: 4,
        status: 'success',
        started_at: '2026-05-06T12:00:04.000Z',
        finished_at: '2026-05-06T12:00:05.000Z',
      }),
      buildCandidate({
        id: 'cand-available',
        provider_id: 'provider-available',
        provider_name: 'Provider Available',
        key_id: 'key-available',
        key_name: 'Available Key',
        candidate_index: 0,
        status: 'available',
        started_at: undefined,
        finished_at: undefined,
      }),
      buildCandidate({
        id: 'cand-skipped',
        provider_id: 'provider-skipped',
        provider_name: 'Provider Skipped',
        key_id: 'key-skipped',
        key_name: 'Skipped Key',
        candidate_index: 1,
        status: 'skipped',
        started_at: undefined,
        finished_at: undefined,
      }),
      buildCandidate({
        id: 'cand-pending-unstarted',
        provider_id: 'provider-pending',
        provider_name: 'Provider Pending',
        key_id: 'key-pending',
        key_name: 'Pending Key',
        candidate_index: 2,
        status: 'pending',
        started_at: undefined,
        finished_at: undefined,
      }),
      buildCandidate({
        id: 'cand-failed',
        provider_id: 'provider-failed',
        provider_name: 'Provider Failed',
        key_id: 'key-failed',
        key_name: 'Failed Key',
        candidate_index: 3,
        status: 'failed',
        started_at: '2026-05-06T12:00:03.000Z',
        finished_at: '2026-05-06T12:00:04.000Z',
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    const labels = [...root.querySelectorAll('.provider-record-name')].map(element =>
      element.textContent?.trim(),
    )
    expect(labels).toEqual(['Provider Failed', 'Provider Success'])
    const nodes = [...root.querySelectorAll('.attempt-stop')]
    expect(nodes.map(getAttemptId)).toEqual([
      'cand-failed', 'cand-success',
    ])
  })

  it('expands the path grid for multiple providers without stacking them', async () => {
    const trace = buildTrace([
      buildCandidate({ id: 'provider-a-key-1', provider_id: 'provider-a', provider_name: 'Provider A', key_id: 'key-a-1', key_name: 'Key A1', candidate_index: 0, status: 'failed' }),
      buildCandidate({ id: 'provider-b-key-1', provider_id: 'provider-b', provider_name: 'Provider B', key_id: 'key-b-1', key_name: 'Key B1', candidate_index: 1, status: 'failed' }),
      buildCandidate({ id: 'provider-c-key-1', provider_id: 'provider-c', provider_name: 'Provider C', key_id: 'key-c-1', key_name: 'Key C1', candidate_index: 2, status: 'success' }),
    ])

    const root = mountTimeline(trace, {}, { autoOpenDetail: false })
    await flushPendingUpdates()

    const stage = root.querySelector<HTMLElement>('.path-stage')
    expect(root.querySelectorAll('.path-provider')).toHaveLength(3)
    expect(root.querySelectorAll('.path-link--provider')).toHaveLength(2)
    expect(stage?.style.gridTemplateColumns.match(/minmax/g)?.length).toBe(9)
    expect(root.querySelectorAll('.path-key')).toHaveLength(3)
  })

  it('keeps a provider revisit after another provider as a separate path node', async () => {
    const trace = buildTrace([
      buildCandidate({ id: 'a-1', provider_id: 'provider-a', provider_name: 'Provider A', key_id: 'a-1', key_name: 'A Key 1', candidate_index: 0, status: 'failed' }),
      buildCandidate({ id: 'b-1', provider_id: 'provider-b', provider_name: 'Provider B', key_id: 'b-1', key_name: 'B Key 1', candidate_index: 1, status: 'failed' }),
      buildCandidate({ id: 'a-2', provider_id: 'provider-a', provider_name: 'Provider A', key_id: 'a-2', key_name: 'A Key 2', candidate_index: 2, status: 'success' }),
    ])

    const root = mountTimeline(trace, {}, { autoOpenDetail: false })
    await flushPendingUpdates()

    expect([...root.querySelectorAll('.path-provider')].map(node => node.getAttribute('data-provider-id')))
      .toEqual(['provider-a', 'provider-b', 'provider-a'])
    const providerTones = [...root.querySelectorAll<HTMLElement>('.path-provider')]
      .map(node => node.getAttribute('data-provider-tone'))
    expect(providerTones).toEqual([providerTones[0], providerTones[1], providerTones[0]])
    expect(providerTones[0]).not.toBe(providerTones[1])
    expect([...root.querySelectorAll('.waterfall-provider-name')].map(node => node.textContent?.trim()))
      .toEqual(['Provider A', 'Provider B', 'Provider A'])
    expect([...root.querySelectorAll('.attempt-stop')].map(getAttemptId)).toEqual(['a-1', 'b-1', 'a-2'])
    expect(root.querySelector('[data-route-summary]')?.textContent?.replace(/\s+/g, ' ').trim())
      .toBe('2 家供应商，3 次上游请求')
  })

  it('keeps provider revisit nodes aligned after inserting an earlier provider', async () => {
    const first = buildCandidate({ id: 'a-1', provider_id: 'provider-a', provider_name: 'Provider A', key_id: 'a-1', key_name: 'A Key 1', candidate_index: 0, status: 'failed' })
    const second = buildCandidate({ id: 'b-1', provider_id: 'provider-b', provider_name: 'Provider B', key_id: 'b-1', key_name: 'B Key 1', candidate_index: 1, status: 'failed' })
    const third = buildCandidate({ id: 'a-2', provider_id: 'provider-a', provider_name: 'Provider A', key_id: 'a-2', key_name: 'A Key 2', candidate_index: 2, status: 'success' })
    const data = shallowRef(buildTrace([first, second, third]))
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({
      setup: () => () => h(HorizontalRequestTimeline, {
        traceData: data.value,
        requestId: data.value.request_id,
      }),
    }))
    app.mount(root)
    mountedApps.push({ app, root })
    await flushPendingUpdates()

    data.value = buildTrace([
      buildCandidate({ id: 'x-1', provider_id: 'provider-x', provider_name: 'Provider X', key_id: 'x-1', key_name: 'X Key 1', candidate_index: 0, status: 'failed' }),
      { ...first, candidate_index: 1 },
      { ...second, candidate_index: 2 },
      { ...third, candidate_index: 3 },
    ])
    await flushPendingUpdates()

    expect([...root.querySelectorAll('.path-provider')].map(node => node.getAttribute('data-provider-id')))
      .toEqual(['provider-x', 'provider-a', 'provider-b', 'provider-a'])
    expect([...root.querySelectorAll('.attempt-stop')].map(getAttemptId))
      .toEqual(['x-1', 'a-1', 'b-1', 'a-2'])
  })

  it('keeps successful runtime pool key visible when only pool_key_index is recorded', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'pool-skipped',
        provider_id: 'provider-pool',
        provider_name: 'CodexFree2',
        key_id: 'pool-group',
        key_name: 'CodexFree2',
        candidate_index: 0,
        status: 'skipped',
        started_at: undefined,
        finished_at: undefined,
        extra_data: { pool_group_id: 'provider-pool' },
      }),
      buildCandidate({
        id: 'pool-success',
        provider_id: 'provider-pool',
        provider_name: 'CodexFree2',
        key_id: 'key-success',
        key_name: 'Success Key',
        candidate_index: 1,
        status: 'success',
        extra_data: { pool_key_index: 0 },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelector('.provider-record')?.textContent).toContain('Success Key')
    expect(root.querySelector('.waterfall-item[data-attempt-id="pool-success"]')?.getAttribute('data-status')).toBe('success')
  })

  it('uses candidate terminal status for node colors instead of overriding with HTTP code', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-body-error',
        provider_id: 'provider-body-error',
        provider_name: 'Provider Body Error',
        key_id: 'key-body-error',
        key_name: 'Body Error Key',
        candidate_index: 0,
        status: 'failed',
        status_code: 200,
      }),
      buildCandidate({
        id: 'cand-success',
        provider_id: 'provider-success',
        provider_name: 'Provider Success',
        key_id: 'key-success',
        key_name: 'Success Key',
        candidate_index: 1,
        status: 'success',
        status_code: 200,
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-body-error"]')?.classList.contains('path-key--failed')).toBe(true)
    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-body-error"]')?.getAttribute('data-status')).toBe('failed')
    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-success"]')?.getAttribute('data-status')).toBe('success')
    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-body-error"]')?.textContent).toContain('200')
    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-body-error"]')?.textContent).not.toContain('失败')
    expect(root.querySelector('.waterfall-item[data-attempt-id="cand-body-error"]')?.textContent).not.toContain('HTTP')

    clickAttempt(root, 'cand-body-error')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-status')).toBe('failed')
    expect(root.querySelector('.status-tag')).toBeNull()
    expect(root.textContent).not.toContain('所有候选均未完成请求')
    expect(root.querySelector('[data-attempt-http-status]')?.textContent).toContain('200')
  })

  it('shows an in-progress request result without borrowing the failed attempt status code', async () => {
    const trace = buildTrace([
      buildCandidate({ id: 'failed-before-retry', status: 'failed', status_code: 504, latency_ms: 180_003 }),
    ])
    trace.final_status = 'failed'
    trace.total_latency_ms = 181_000

    const root = mountTimeline(trace, {
      requestStatus: 'pending',
      overrideStatusCode: 504,
    }, { autoOpenDetail: false })
    await flushPendingUpdates()

    expect(root.querySelector('.path-response-status')?.textContent?.trim()).toBe('进行中')
    expect(root.querySelector('.path-response-status')?.textContent).not.toContain('504')
  })

  it('does not invent an HTTP status for a successful candidate without status_code', async () => {
    const trace = buildTrace([
      buildCandidate({ id: 'success-without-code', status: 'success', status_code: undefined }),
    ])
    trace.final_status = 'success'

    const root = mountTimeline(trace, {}, { autoOpenDetail: false })
    await flushPendingUpdates()

    expect(root.querySelector('.path-key-http')?.textContent?.trim()).toBe('✓ -')
    expect(root.querySelector('.path-response-status')?.textContent?.trim()).toBe('-')
  })

  it('does not repeat success as the Key reason in visible or accessible text', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'success-without-error',
        key_name: 'Success Key',
        status: 'success',
        status_code: 200,
      }),
    ])

    const root = mountTimeline(trace, {}, { autoOpenDetail: false })
    await flushPendingUpdates()

    const key = root.querySelector<HTMLElement>('[data-attempt-id="success-without-error"]')
    expect(key?.querySelector('.path-key-reason')).toBeNull()
    expect(key?.getAttribute('aria-label')).toBe('Success Key，成功，查看详情')
  })

  it('keeps a stream-interrupted Key red through the shared status class', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-stream-interrupted',
        status: 'stream_interrupted',
        status_code: 200,
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    const marker = root.querySelector('.waterfall-item[data-attempt-id="cand-stream-interrupted"]')
    expect(marker?.getAttribute('data-status')).toBe('failed')
    expect(marker?.classList.contains('path-key--failed')).toBe(true)
  })

  it('renders Codex image progress from candidate image_progress', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-image-progress',
        provider_id: 'provider-image',
        provider_name: 'Codex Image',
        key_id: 'key-image',
        key_name: 'Image Key',
        candidate_index: 0,
        status: 'streaming',
        finished_at: undefined,
        image_progress: {
          phase: 'upstream_streaming',
          upstream_ttfb_ms: 3807,
          upstream_sse_frame_count: 12,
          partial_image_count: 1,
          last_upstream_event: 'response.output_item.added',
          last_upstream_frame_at_unix_ms: Date.now(),
          last_client_visible_event: 'image_generation.partial_image',
          downstream_heartbeat_count: 3,
          downstream_heartbeat_interval_ms: 15000,
          last_downstream_heartbeat_at_unix_ms: Date.now(),
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).toContain('图片生成进度')
    expect(root.textContent).toContain('上游生成中')
    expect(root.textContent).toContain('3.81s')
    expect(root.textContent).toContain('下游心跳')
    expect(root.textContent).toContain('15.00s')
    expect(root.textContent).toContain('response.output_item.added')
    expect(root.textContent).toContain('image_generation.partial_image')
  })

  it('treats 3xx terminal responses as failed for node display', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-redirect',
        provider_id: 'provider-redirect',
        provider_name: 'Provider Redirect',
        key_id: 'key-redirect',
        key_name: 'Redirect Key',
        candidate_index: 0,
        status: 'success',
        status_code: 302,
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    const inspector = root.querySelector<HTMLElement>('[data-execution-inspector]')
    expect(inspector?.getAttribute('data-status')).toBe('failed')
  })

  it.each(['pending', 'streaming'] as const)('keeps the %s request active with only a failed attempt during a retry gap', async (status) => {
    const onTraceState = vi.fn()
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-stale-failed',
        provider_id: 'provider-stale',
        provider_name: 'Provider Stale',
        key_id: 'key-stale',
        key_name: 'Stale Key',
        candidate_index: 0,
        status: 'failed',
        status_code: 504,
        latency_ms: 180_003,
      }),
    ])
    trace.final_status = 'failed'
    trace.total_latency_ms = 181_000

    const root = mountTimeline(trace, {
      requestStatus: status,
      overrideStatusCode: 504,
      onTraceState,
    })
    await nextTick()

    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-status')).toBe('failed')
    const lastCall = onTraceState.mock.calls[onTraceState.mock.calls.length - 1]?.[0]
    expect(lastCall).toMatchObject({
      finalStatus: status,
      latencyMs: 181_000,
    })
  })

  it('keeps removed technical fields out of the attempt detail', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-path',
        provider_id: 'provider-path',
        provider_name: 'Provider Path',
        key_id: 'key-path',
        key_name: 'Path Key',
        candidate_index: 0,
        status: 'failed',
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).not.toContain('技术详情')
    expect(root.textContent).not.toContain('更多字段')
    expect(root.querySelector('.request-path-code')).toBeNull()
  })

  it('does not render a separate extended-fields disclosure', async () => {
    const trace: RequestTrace = {
      ...buildTrace([
        buildCandidate({
          id: 'cand-trace-path',
          provider_id: 'provider-path',
          provider_name: 'Provider Path',
          key_id: 'key-path',
          key_name: 'Path Key',
          candidate_index: 0,
          status: 'failed',
        }),
      ]),
      request_path: '/v1/images/generations',
    }

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).not.toContain('技术详情')
    expect(root.querySelector('.extended-block')).toBeNull()
  })

  it('shows upstream response headers and body in one error envelope', async () => {
    const upstreamErrorMessage = 'This content was flagged for possible cybersecurity risk. If this seems wrong, try rephrasing your request. To get authorized for security work, join the Trusted Access for Cyber program: https://chatgpt.com/cyber'
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-upstream-response',
        provider_id: 'provider-upstream',
        provider_name: 'Provider Upstream',
        key_id: 'key-upstream',
        key_name: 'Upstream Key',
        candidate_index: 0,
        status: 'failed',
        error_message: 'execution runtime stream returned non-success status 400',
        extra_data: {
          upstream_response: {
            status_code: 400,
            headers: {
              'content-type': 'application/json',
              'x-request-id': 'req_usage-cyber-risk-demo',
            },
            body: {
              error: {
                type: 'invalid_request',
                message: upstreamErrorMessage,
                code: 400,
              },
            },
            body_ref: 'usage://request/req-1/response_body',
            body_state: 'reference',
          },
          error_flow: {
            source: 'upstream_response',
            status_code: 400,
            classification: 'use_default',
            decision: 'use_default',
            propagation: 'none',
            retryable: false,
            safe_to_expose: false,
            message: 'execution runtime stream returned non-success status 400',
          },
          client_response: {
            status_code: 502,
            headers: { 'content-type': 'application/json' },
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelector('.error-title')?.textContent).toContain(upstreamErrorMessage)
    expect(root.textContent).not.toContain('原因说明')
    expect(root.textContent).not.toContain('上游返回非成功状态 400')
    const upstreamResponse = root.querySelector<HTMLElement>('.error-upstream-response-json pre')
    expect(upstreamResponse?.dataset.title).toBe('上游响应')
    expect(JSON.parse(upstreamResponse?.textContent ?? '{}')).toEqual({
      header: {
        'content-type': 'application/json',
        'x-request-id': 'req_usage-cyber-risk-demo',
      },
      body: {
        error: {
          type: 'invalid_request',
          message: upstreamErrorMessage,
          code: 400,
        },
      },
    })
    expect(upstreamResponse?.textContent).not.toContain('"status_code"')
    expect(upstreamResponse?.textContent).not.toContain('"headers"')
    expect(upstreamResponse?.textContent).not.toContain('"body_ref"')
    expect(upstreamResponse?.textContent).not.toContain('"body_state"')
    expect(root.textContent).not.toContain('上游真实响应')
    expect(root.textContent).not.toContain('execution runtime stream returned non-success status 400')
    expect(root.textContent).not.toContain('真实请求错误')
    expect(root.textContent).not.toContain('返回客户端响应')
    expect(root.textContent).not.toContain('默认处理')
    expect(root.textContent).not.toContain('none')
    expect(root.textContent).not.toContain('不再重试')
    expect(root.textContent).not.toContain('该错误被标记为敏感上游错误')
  })

  it('shows only the timeout reason and collapses the upstream response', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-first-byte-timeout',
        status: 'failed',
        status_code: 503,
        error_message: 'Stream first byte timeout',
        extra_data: {
          upstream_response: {
            status_code: 503,
            headers: { 'content-type': 'application/json' },
            body_state: 'disabled',
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    const errorBlock = root.querySelector<HTMLElement>('.error-block')
    expect(errorBlock?.getAttribute('role')).toBe('alert')
    expect(errorBlock?.classList.contains('is-warning')).toBe(true)
    expect(errorBlock?.textContent).toContain('上游服务响应超时')
    expect(errorBlock?.textContent).not.toContain('建议稍后重试')
    expect(errorBlock?.textContent).not.toContain('HTTP 503')

    const upstreamResponse = errorBlock?.querySelector<HTMLDetailsElement>('[data-upstream-response]')
    expect(upstreamResponse?.open).toBe(false)
    expect(upstreamResponse?.textContent).toContain('上游响应')
    expect(upstreamResponse?.textContent).not.toContain('技术详情')
    expect(upstreamResponse?.querySelector<HTMLElement>('.error-upstream-response-json pre')?.dataset.title)
      .toBe('上游响应')
  })

  it('keeps local sync diagnostics visible when upstream response body capture is disabled', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-local-sync-diagnostic',
        provider_id: 'provider-local-sync',
        provider_name: 'Provider Local Sync',
        key_id: 'key-local-sync',
        key_name: 'Local Sync Key',
        candidate_index: 0,
        status: 'failed',
        status_code: 500,
        error_type: 'local_sync_attempt_aborted',
        error_message: 'Local sync attempt failed before terminal finalization: Internal("Unsupported provider stream event cannot be converted losslessly: field $.type = \\"response.future.delta\\"; fields: payload, response, type")',
        extra_data: {
          upstream_response: {
            status_code: 500,
            headers: { 'content-type': 'application/json' },
            body_state: 'disabled',
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelector('.attempt-stop')?.textContent).not.toContain('HTTP')
    expect(root.querySelector('[data-attempt-http-status]')?.textContent).toContain('500')
    expect(root.textContent).not.toContain('原因说明')
    expect(root.textContent).toContain('流式格式转换失败')
    expect(root.textContent).toContain('上游返回了当前不支持的 stream event')
    expect(root.textContent).toContain('字段 $.type = "response.future.delta"')
    expect(root.querySelector('.error-diagnostic-json')).toBeNull()
    expect(root.textContent).not.toContain('技术详情')
    expect(root.textContent).not.toContain('更多字段')
    expect(root.querySelector('.error-upstream-response-json')?.textContent).toContain('"header":{"content-type":"application/json"}')
  })

  it('keeps an unsent conversion rejection out of the actual request path', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-request-conversion',
        provider_id: 'provider-request-conversion',
        provider_name: 'Provider Request Conversion',
        key_id: 'key-request-conversion',
        key_name: 'Request Conversion Key',
        candidate_index: 0,
        status: 'skipped',
        skip_reason: 'provider_request_body_build_failed',
        extra_data: {
          failure_diagnostic: {
            kind: 'request_conversion',
            path: '$.n',
            message: 'lossy conversion blocked from openai:chat to openai:responses at n: multiple completions cannot be represented losslessly',
            safe_to_show: true,
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelectorAll('.attempt-stop')).toHaveLength(0)
    expect(root.textContent).toContain('没有已发送的上游请求')
    expect(root.textContent).not.toContain('$.n')
  })

  it('formats unsupported stream finish reasons with the failing field', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-finish-reason',
        provider_id: 'provider-finish-reason',
        provider_name: 'Provider Finish Reason',
        key_id: 'key-finish-reason',
        key_name: 'Finish Reason Key',
        candidate_index: 0,
        status: 'failed',
        status_code: 500,
        error_message: 'Internal("Unsupported provider stream finish reason cannot be converted losslessly: field $.finish_reason = \\"future_reason\\"")',
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).toContain('流式格式转换失败')
    expect(root.textContent).toContain('finish reason')
    expect(root.textContent).toContain('字段 $.finish_reason = "future_reason"')
    expect(root.querySelector('.error-diagnostic-json')).toBeNull()
    expect(root.textContent).not.toContain('技术详情')
  })

  it('uses conversion messages from error_flow as the diagnostic breakpoint source', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-error-flow-conversion',
        provider_id: 'provider-error-flow-conversion',
        provider_name: 'Provider Error Flow Conversion',
        key_id: 'key-error-flow-conversion',
        key_name: 'Error Flow Conversion Key',
        candidate_index: 0,
        status: 'failed',
        status_code: 500,
        error_message: 'execution runtime stream returned non-success status 500',
        extra_data: {
          upstream_response: {
            status_code: 500,
            body_state: 'disabled',
          },
          error_flow: {
            status_code: 500,
            message: 'lossy conversion blocked from openai:chat to openai:responses at n: multiple completions cannot be represented losslessly',
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).toContain('格式转换失败')
    expect(root.textContent).toContain('OpenAI Chat → OpenAI Responses')
    expect(root.textContent).toContain('字段 $.n 会丢失信息')
    expect(root.textContent).not.toContain('上游返回非成功状态 500')
    expect(root.querySelector('.error-diagnostic-json')).toBeNull()
    expect(root.textContent).not.toContain('技术详情')
  })

  it('shows failed diagnostic messages even when the only response panel data is diagnostic metadata', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-diagnostic-only',
        provider_id: 'provider-diagnostic-only',
        provider_name: 'Provider Diagnostic Only',
        key_id: 'key-diagnostic-only',
        key_name: 'Diagnostic Only Key',
        candidate_index: 0,
        status: 'failed',
        error_type: 'request_conversion_failed',
        extra_data: {
          failure_diagnostic: {
            kind: 'request_conversion',
            path: '$.temperature',
            message: 'unsupported field temperature in openai:responses: temperature cannot be represented',
            safe_to_show: true,
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).not.toContain('原因说明')
    expect(root.textContent).toContain('格式转换失败')
    expect(root.textContent).toContain('OpenAI Responses 不支持字段 $.temperature')
    expect(root.querySelector('.error-diagnostic-json')).toBeNull()
    expect(root.textContent).not.toContain('技术详情')
  })

  it('keeps the failure message when upstream response only records an empty body state', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-empty-body-state',
        provider_id: 'provider-empty-body-state',
        provider_name: 'Provider Empty Body State',
        key_id: 'key-empty-body-state',
        key_name: 'Empty Body State Key',
        candidate_index: 0,
        status: 'failed',
        error_type: 'stream_missing_terminal_event',
        error_message: 'execution runtime stream ended before provider terminal event',
        extra_data: {
          upstream_response: {
            body_state: 'none',
          },
        },
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.textContent).not.toContain('原因说明')
    expect(root.textContent).toContain('execution runtime stream ended before provider terminal event')
    expect(root.querySelector('.error-block .error-json')).toBeNull()
    expect(root.textContent).not.toContain('"body_state":"none"')
  })

  it('falls back to key id while an active candidate is missing key name', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-active-key-id',
        provider_id: 'provider-active',
        provider_name: 'Provider Active',
        key_id: 'key-active-id',
        key_name: undefined,
        candidate_index: 0,
        status: 'pending',
        finished_at: undefined,
      }),
    ])
    trace.final_status = 'pending'

    const root = mountTimeline(trace)
    await nextTick()

    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('cand-active-key-id')
    expect(root.querySelector('[data-execution-inspector] .waterfall-detail-identity')).toBeNull()
    expect(root.querySelector('.waterfall-key')?.textContent?.trim()).toBe('key-active-id')
  })

  it('uses latency to display the time range when recorded finish time equals start time', async () => {
    const trace = buildTrace([
      buildCandidate({
        id: 'cand-success-same-time',
        status: 'success',
        started_at: '2026-05-06T12:00:00.000Z',
        finished_at: '2026-05-06T12:00:00.000Z',
        latency_ms: 22200,
      }),
    ])

    const root = mountTimeline(trace)
    await nextTick()

    const detailText = root.querySelector('[data-execution-inspector]')?.textContent ?? ''
    expect(detailText).toContain('22.20s')
    expect([...root.querySelectorAll('.waterfall-detail-facts > div')].some(element => element.textContent?.includes('22.20s'))).toBe(true)
  })

  it('keeps a single attempt in the same card without navigation controls', async () => {
    const root = mountTimeline(buildTrace([buildCandidate({ status: 'skipped', skip_reason: 'key_circuit_open', started_at: undefined, status_code: 503 })]))
    await nextTick()
    expect(root.querySelectorAll('[data-execution-inspector]')).toHaveLength(0)
    expect(root.querySelectorAll('.attempt-stop')).toHaveLength(0)
    expect(root.textContent).toContain('没有已发送的上游请求')
    expect(root.querySelector('.status-tag')).toBeNull()
  })

  it('does not label a failed candidate as unsent based on its circuit reason alone', async () => {
    const root = mountTimeline(buildTrace([buildCandidate({ status: 'failed', skip_reason: 'key_circuit_open', status_code: 503 })]))
    await nextTick()
    expect(root.querySelector('.attempt-stop')?.classList.contains('path-key--failed')).toBe(true)
    expect(root.querySelector('.attempt-stop')?.textContent).not.toContain('本地冷却中')
  })

  it('preserves the manually selected attempt by identity when external data inserts earlier records', async () => {
    const first = buildCandidate({ id: 'first', candidate_index: 1, key_name: 'First Key' })
    const second = buildCandidate({ id: 'second', candidate_index: 2, key_name: 'Second Key', status: 'success' })
    const data = shallowRef(buildTrace([first, second]))
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({ setup: () => () => h(HorizontalRequestTimeline, { traceData: data.value, requestId: data.value.request_id }) }))
    app.mount(root)
    mountedApps.push({ app, root })
    await flushPendingUpdates()
    clickAttempt(root, 'first')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('first')
    data.value = buildTrace([buildCandidate({ id: 'inserted', candidate_index: 0 }), { ...first, key_name: 'Updated First Key' }, second])
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('first')
    expect(root.querySelector('.waterfall-item[data-attempt-id="first"] .waterfall-key')?.textContent?.trim()).toBe('Updated First Key')
    expect(root.querySelectorAll('.attempt-stop')).toHaveLength(3)
    data.value = { ...buildTrace([buildCandidate({ id: 'other', request_id: 'req-2' })]), request_id: 'req-2' }
    await flushPendingUpdates()
    expect(root.querySelectorAll('.provider-record')).toHaveLength(1)
    expect(getAttemptId(root.querySelector('.attempt-stop'))).toBe('other')
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
  })

  it('closes an opened detail when its record disappears instead of opening another Key', async () => {
    const first = buildCandidate({ id: 'first', candidate_index: 0, key_name: 'First Key' })
    const second = buildCandidate({ id: 'second', candidate_index: 1, key_name: 'Second Key', status: 'success' })
    const root = document.createElement('div')
    document.body.appendChild(root)
    const data = shallowRef(buildTrace([first, second]))
    const app = createApp(defineComponent({
      setup: () => () => h(HorizontalRequestTimeline, { traceData: data.value, requestId: data.value.request_id }),
    }))
    app.mount(root)
    mountedApps.push({ app, root })
    await flushPendingUpdates()
    clickAttempt(root, 'second')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('second')

    data.value = buildTrace([first])
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
    expect(root.querySelector('.waterfall-item.is-open')).toBeNull()
  })

  it('ignores an old response when switching requests during a trace load', async () => {
    let finishOld!: (trace: RequestTrace) => void
    const requestId = shallowRef('req-1')
    requestTraceApiMock.getRequestTrace
      .mockImplementationOnce(() => new Promise<RequestTrace>(resolve => { finishOld = resolve }))
      .mockResolvedValueOnce({ ...buildTrace([buildCandidate({ id: 'new-attempt', request_id: 'req-2' })]), request_id: 'req-2' })
    const root = document.createElement('div')
    document.body.appendChild(root)
    const app = createApp(defineComponent({ setup: () => () => h(HorizontalRequestTimeline, { requestId: requestId.value }) }))
    app.mount(root)
    mountedApps.push({ app, root })
    requestId.value = 'req-2'
    await flushPendingUpdates()
    expect(getAttemptId(root.querySelector('.attempt-stop'))).toBe('new-attempt')
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
    finishOld(buildTrace([buildCandidate({ id: 'old-attempt' })]))
    await flushPendingUpdates()
    expect(getAttemptId(root.querySelector('.attempt-stop'))).toBe('new-attempt')
    expect(root.querySelector('[data-execution-inspector]')).toBeNull()
  })

  it('keeps manual selection during API refresh and supports keyboard cycling', async () => {
    const first = buildCandidate({ id: 'first', candidate_index: 0, key_name: 'First Key' })
    const second = buildCandidate({ id: 'second', candidate_index: 1, key_name: 'Second Key', status: 'success' })
    requestTraceApiMock.getRequestTrace.mockResolvedValueOnce(buildTrace([first, second]))
    const { root, refresh } = mountTimelineFromApi('req-1')
    await flushPendingUpdates()
    clickAttempt(root, 'first')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('first')
    requestTraceApiMock.getRequestTrace.mockResolvedValueOnce(buildTrace([{ ...first, key_name: 'Updated Key' }, second]))
    await refresh()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('first')
    expect(root.querySelector('.waterfall-item[data-attempt-id="first"] .waterfall-key')?.textContent?.trim()).toBe('Updated Key')
    clickAttempt(root, 'second')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('second')
  })

  it('follows the active key when silent polling updates the trace', async () => {
    const initialTrace = buildTrace([
      buildCandidate({
        id: 'cand-available-a',
        provider_id: 'provider-a',
        provider_name: 'Provider A',
        key_id: 'key-a',
        key_name: 'Available Key',
        candidate_index: 0,
        status: 'available',
        started_at: undefined,
        finished_at: undefined,
      }),
      buildCandidate({
        id: 'cand-available-b',
        provider_id: 'provider-b',
        provider_name: 'Provider B',
        key_id: 'key-b',
        key_name: 'Streaming Key',
        candidate_index: 1,
        status: 'available',
        started_at: undefined,
        finished_at: undefined,
      }),
    ])
    initialTrace.final_status = 'pending'

    const activeTrace = buildTrace([
      initialTrace.candidates[0],
      buildCandidate({
        id: 'cand-streaming-b',
        provider_id: 'provider-b',
        provider_name: 'Provider B',
        key_id: 'key-b',
        key_name: 'Streaming Key',
        candidate_index: 1,
        status: 'pending',
        started_at: '2026-05-06T12:00:02.000Z',
        finished_at: undefined,
      }),
    ])
    activeTrace.final_status = 'streaming'

    requestTraceApiMock.getRequestTrace.mockResolvedValueOnce(initialTrace)
    const { root, refresh } = mountTimelineFromApi('req-1', {
      requestStatus: 'streaming',
    })
    await flushPendingUpdates()

    requestTraceApiMock.getRequestTrace.mockResolvedValueOnce(activeTrace)
    await refresh()

    clickAttempt(root, 'cand-streaming-b')
    await flushPendingUpdates()
    expect(root.querySelector('[data-execution-inspector]')?.getAttribute('data-attempt-id')).toBe('cand-streaming-b')
    expect(root.querySelector('[data-execution-inspector]')?.textContent).toContain('进行中')
    expect(root.querySelector('[data-execution-inspector] .waterfall-detail-identity')).toBeNull()
  })
})
