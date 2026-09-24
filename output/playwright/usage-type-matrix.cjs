async (page) => {
  const results = []
  const inspect = async () => page.locator('.usage-records-table:visible').evaluate(table => {
    const heads = [...table.querySelectorAll('th')]
    const index = heads.findIndex(head => head.textContent.trim() === '类型')
    const errors = []
    const overflow = [table, ...table.querySelectorAll('th,td')].filter(el => el.scrollWidth > el.clientWidth + 1)
    if (overflow.length) errors.push(...overflow.map(el => `overflow:${el.textContent.trim()}`))
    if (index >= 0) {
      for (const cell of [heads[index], ...table.querySelectorAll(`tbody td:nth-child(${index + 1})`)]) {
        const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT)
        let node
        while ((node = walker.nextNode())) {
          const ys = new Set()
          for (let i = 0; i < node.textContent.length; i++) {
            if (!node.textContent[i].trim()) continue
            const range = document.createRange()
            range.setStart(node, i)
            range.setEnd(node, i + 1)
            ys.add(Math.round(range.getBoundingClientRect().y))
          }
          if (ys.size > 1) errors.push(`wrapped:${node.textContent}`)
        }
      }
    }
    return { width: table.getBoundingClientRect().width, typeWidth: heads[index]?.getBoundingClientRect().width, errors }
  })
  for (const width of [900, 1024, 1200, 1440, 1920, 900]) {
    await page.setViewportSize({ width, height: 640 })
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    const result = await inspect()
    results.push({ viewport: width, ...result })
    if (result.errors.length) throw new Error(JSON.stringify(results))
  }
  await page.getByRole('button', { name: '深色模式', exact: true }).click()
  await page.getByRole('button', { name: '跟随系统', exact: true }).click()
  const defaults = ['time', 'model', 'provider', 'api_format', 'status', 'tokens', 'cost', 'performance']
  for (const ids of [defaults.concat('client_family'), defaults.filter(id => id !== 'status'), ['status'], defaults]) {
    await page.evaluate(ids => localStorage.setItem('usage-records-visible-columns-admin', JSON.stringify(ids)), ids)
    await page.reload()
    await page.locator('.usage-records-table:visible tbody tr').first().waitFor()
    await page.evaluate(() => document.fonts.ready)
    const result = await inspect()
    results.push({ columns: ids, ...result })
    if (result.errors.length) throw new Error(JSON.stringify(results))
  }
  await page.setViewportSize({ width: 900, height: 1100 })
  await page.locator('.usage-records-table:visible').screenshot({ path: 'output/playwright/usage-type-final-900.png' })
  await page.setViewportSize({ width: 900, height: 640 })
  return results
}
