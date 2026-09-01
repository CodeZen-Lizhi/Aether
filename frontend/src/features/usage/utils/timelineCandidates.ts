import type { CandidateRecord } from '@/api/requestTrace'

export const TIMELINE_STATUS: CandidateRecord['status'][] = [
  'success',
  'failed',
  'skipped',
  'cancelled',
  'pending',
  'streaming',
  'available',
  'unused',
  'stream_interrupted',
]

export const makeAttemptKey = (candidateIndex: number, retryIndex: number): string => {
  return `${candidateIndex}:${retryIndex}`
}

export const isAttemptedCandidate = (
  candidate: Pick<CandidateRecord, 'status' | 'started_at'>,
): boolean => {
  switch (candidate.status) {
    case 'streaming':
    case 'success':
    case 'failed':
    case 'cancelled':
    case 'stream_interrupted':
      return true
    case 'pending':
      return Boolean(candidate.started_at)
    case 'available':
    case 'unused':
    case 'skipped':
    default:
      return false
  }
}

export const parseTimelineStatus = (value: unknown): CandidateRecord['status'] | null => {
  if (typeof value !== 'string') return null
  const normalized = value.trim().toLowerCase()
  if ((TIMELINE_STATUS as string[]).includes(normalized)) {
    return normalized as CandidateRecord['status']
  }
  return null
}
