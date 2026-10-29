import { createApp, h } from 'vue'
import { describe, expect, it, vi } from 'vitest'

import type { TestModelFailoverResponse } from '@/api/endpoints/providers'
import { useModelTest } from '../useModelTest'

const mocks = vi.hoisted(() => ({
  failover: vi.fn(),
  successToast: vi.fn(),
  errorToast: vi.fn(),
}))

vi.mock('@/api/endpoints/providers', () => ({
  testModel: vi.fn(),
  testModelFailover: mocks.failover,
}))

vi.mock('@/api/requestTrace', () => ({
  requestTraceApi: {
    getRequestTrace: vi.fn(),
  },
}))

vi.mock('../useToast', () => ({
  useToast: () => ({
    success: mocks.successToast,
    error: mocks.errorToast,
  }),
}))

describe('useModelTest', () => {
  it('stops the active request and ignores a late response', async () => {
    let resolveResponse!: (value: TestModelFailoverResponse) => void
    mocks.failover.mockReturnValue(new Promise<TestModelFailoverResponse>((resolve) => {
      resolveResponse = resolve
    }))

    let modelTest!: ReturnType<typeof useModelTest>
    const container = document.createElement('div')
    document.body.appendChild(container)
    const app = createApp({
      setup() {
        modelTest = useModelTest({ providerId: () => 'provider-1' })
        return () => h('div')
      },
    })
    app.mount(container)

    const running = modelTest.startTest({
      mode: 'global',
      modelName: 'gpt-5.6',
      displayLabel: 'gpt-5.6',
    })
    expect(modelTest.testing.value).toBe(true)

    modelTest.cancelTest()
    expect(modelTest.testing.value).toBe(false)
    expect(modelTest.testStopped.value).toBe(true)

    resolveResponse({
      success: true,
      model: 'gpt-5.6',
      provider: { id: 'provider-1', name: 'Provider' },
      attempts: [],
      total_candidates: 0,
      total_attempts: 0,
    })
    await running

    expect(modelTest.testResult.value).toBeNull()
    expect(modelTest.testing.value).toBe(false)
    expect(mocks.successToast).not.toHaveBeenCalled()
    expect(mocks.errorToast).not.toHaveBeenCalled()

    app.unmount()
    container.remove()
  })
})
