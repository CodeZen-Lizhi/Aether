# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

<!--
Document your project's quality standards here.

Questions to answer:
- What patterns are forbidden?
- What linting rules do you enforce?
- What are your testing requirements?
- What code review standards apply?
-->

(To be filled by the team)

---

## Forbidden Patterns

<!-- Patterns that should never be used and why -->

(To be filled by the team)

---

## Required Patterns

### Provider-key circuit state transitions

- Circuit and health ownership is scoped by `(key_id, provider_api_format)`.
  Never skip every format on a key when only one format is open.
- A failure projection must append the current failed sample before evaluating
  the rolling success-rate threshold. A persisted success projection must
  preserve the existing window and append a successful sample; it must not
  rebuild the circuit payload and silently discard history. HTTP 429 remains
  outside the failure-health window.
- A successful half-open probe closes the circuit at health `0.75` with
  `ramp_remaining_successes = 3`. The next three consecutive successes produce
  health `0.833`, `0.917`, and `1.0`, decrementing the counter to zero.
- Any failure while the recovery counter is non-zero reopens the circuit
  immediately. Its probe interval advances from the retained exponential
  backoff step and remains capped by the key's configured maximum.
- Candidate plans are snapshots. After a request-local failure, a dynamic
  attempt loop must recheck the canonical circuit state before executing
  another candidate with the same `(key_id, provider_api_format)`. Reuse the
  compare-and-set half-open probe claim; do not duplicate its ownership logic.
- A catalog read failure during the last-moment circuit recheck follows the
  existing data-plane fail-open policy and must be logged without credentials.

---

## Testing Requirements

Circuit-breaker changes require both levels of regression coverage:

- Pure projection tests for the complete recovery sequence, rolling-window
  threshold boundary, and retained probe backoff.
- An execution-loop test proving that the failure effect opens the persisted
  circuit before a pre-materialized same-scope retry is considered, while a
  different API format remains eligible.

---

## Code Review Checklist

<!-- What reviewers should check -->

(To be filled by the team)
