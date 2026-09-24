async (page) => {
  const result = await page.locator('.usage-records-table:visible').evaluate(table => {
    const bounds = rect => ({ x: rect.x, y: rect.y, width: rect.width, height: rect.height, right: rect.right })
    const getGlyphs = element => {
      const glyphs = []
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
      let node
      while ((node = walker.nextNode())) {
        for (let index = 0; index < node.textContent.length; index++) {
          if (!node.textContent[index].trim()) continue
          const range = document.createRange()
          range.setStart(node, index)
          range.setEnd(node, index + 1)
          glyphs.push({ char: node.textContent[index], ...bounds(range.getBoundingClientRect()) })
        }
      }
      return glyphs
    }
    const heads = [...table.querySelectorAll('thead th')]
    const statusIndex = heads.findIndex(head => ['类型', 'Type'].includes(head.textContent.trim()))
    if (statusIndex < 0) throw new Error('Missing type header')
    const cells = [heads[statusIndex], ...table.querySelectorAll(`tbody tr td:nth-child(${statusIndex + 1})`)]
    const labels = cells.map(cell => {
      const rect = cell.getBoundingClientRect()
      const glyphs = getGlyphs(cell)
      const lines = [...new Set(glyphs.map(glyph => Math.round(glyph.y)))].length
      return { label: cell.textContent.trim(), lines, rect: bounds(rect),
        outside: glyphs.some(glyph => glyph.x < rect.x - 1 || glyph.right > rect.right + 1),
      }
    })
    const overflows = [table, ...table.querySelectorAll('th,td')]
      .filter(el => el.scrollWidth > el.clientWidth + 1)
      .map(el => ({ tag: el.tagName, text: el.textContent.trim().slice(0, 100), client: el.clientWidth, scroll: el.scrollWidth }))
    return { labels, overflows, width: table.getBoundingClientRect().width }
  })
  const invalid = result.labels.filter(item => item.outside || (!item.label.includes('→') && item.lines !== 1))
  if (invalid.length || result.overflows.length) throw new Error(JSON.stringify(result))
  return result
}
