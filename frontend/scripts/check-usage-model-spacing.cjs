// Open /scripts/fixtures/usage-model-spacing.html on a running Vite server, then
// run playwright-cli run-code --filename frontend/scripts/check-usage-model-spacing.cjs.
// This also works on real usage table, card, and request-detail pages.
// Checks rendered components, including their real font metrics, without reading CSS classes.
// eslint-disable-next-line @typescript-eslint/no-unused-expressions -- playwright-cli evaluates this function.
async (page) => {
  await page.locator('[data-usage-model-source]:visible').first().waitFor()
  await page.evaluate(() => document.fonts.ready)
  const results = await page.evaluate(() => {
    const results = []
    for (const model of document.querySelectorAll('[data-usage-model-layout]')) {
      if (!model.getClientRects().length) continue
      const source = model.querySelector('[data-usage-model-source]')
      const badges = [...model.querySelectorAll('[data-usage-model-badge]')]
      const range = document.createRange()
      range.selectNodeContents(source)
      const lines = [...range.getClientRects()].filter(rect => rect.width > 0)
      const last = lines.at(-1)
      const rect = model.getBoundingClientRect()
      const inline = model.getAttribute('data-usage-model-layout') === 'inline'
      const badge = badges[0]?.getBoundingClientRect()
      let gap = null
      let adjacent = true
      if (inline && badge) {
        const sameLine = badge.top < last.bottom && badge.bottom > last.top
        gap = Math.round((badge.left - last.right) * 100) / 100
        const lineHeight = Math.max(last.height, badge.height,
          Number.parseFloat(getComputedStyle(source).lineHeight) || 0)
        const newLine = badge.top >= last.bottom - 1
          && badge.top <= last.bottom + lineHeight
          && badge.left <= lines[0].left + 8
        adjacent = (sameLine && gap >= 0 && gap <= 8) || newLine
      }
      const target = model.querySelector('[data-usage-model-target]')
      const targetRange = document.createRange()
      if (target) targetRange.selectNodeContents(target)
      const overflow = [...lines, ...targetRange.getClientRects(), ...badges.map(item => item.getBoundingClientRect())]
        .some(item => item.left < rect.left - 1 || item.right > rect.right + 1)
      results.push({
        case: model.closest('[data-spacing-case]')?.getAttribute('data-spacing-case') ?? source.textContent,
        width: Math.round(rect.width), lines: lines.length, gap, adjacent, overflow,
      })
    }
    return results
  })
  if (!results.length) throw new Error('No visible usage models found')
  const failures = results.filter(result => !result.adjacent || result.overflow)
  if (failures.length) throw new Error(`Model badge geometry failed: ${JSON.stringify(failures)}`)
  return results
}
