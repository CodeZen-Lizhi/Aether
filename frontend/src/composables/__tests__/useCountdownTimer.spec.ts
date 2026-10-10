import { describe, expect, it } from 'vitest'

import { formatCountdown, getProbeCountdown } from '@/composables/useCountdownTimer'

describe('useCountdownTimer helpers', () => {
  it('does not turn an elapsed probe deadline into visible status text', () => {
    expect(formatCountdown(0)).toBe('')
    expect(getProbeCountdown('2026-01-01T00:00:00.000Z', 0)).toBeNull()
  })
})
