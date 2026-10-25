import { beforeEach, describe, expect, it, vi } from 'vitest'

const { getMock } = vi.hoisted(() => ({ getMock: vi.fn() }))

vi.mock('@/api/client', () => ({
  default: {
    get: getMock,
  },
}))

import { getAllProviderModels, getProviderModels } from '@/api/endpoints/models'

describe('provider model loading', () => {
  beforeEach(() => {
    getMock.mockReset()
  })

  it('keeps the existing single-page contract of getProviderModels', async () => {
    getMock.mockResolvedValue({ data: [{ id: 'model-page' }] })

    const models = await getProviderModels('provider-page')

    expect(models).toEqual([{ id: 'model-page' }])
    expect(getMock).toHaveBeenCalledOnce()
    expect(getMock).toHaveBeenCalledWith(
      '/api/admin/providers/provider-page/models',
      { params: undefined },
    )
  })

  it('loads every page for the full-model workbench without truncating at 100', async () => {
    const firstPage = Array.from({ length: 500 }, (_, index) => ({ id: `model-${index}` }))
    const secondPage = Array.from({ length: 37 }, (_, index) => ({ id: `model-${500 + index}` }))
    getMock
      .mockResolvedValueOnce({ data: firstPage })
      .mockResolvedValueOnce({ data: secondPage })

    const models = await getAllProviderModels('provider-large')

    expect(models).toHaveLength(537)
    expect(getMock).toHaveBeenNthCalledWith(
      1,
      '/api/admin/providers/provider-large/models',
      { params: { skip: 0, limit: 500 } },
    )
    expect(getMock).toHaveBeenNthCalledWith(
      2,
      '/api/admin/providers/provider-large/models',
      { params: { skip: 500, limit: 500 } },
    )
    expect(getMock).toHaveBeenCalledTimes(2)
  })
})
