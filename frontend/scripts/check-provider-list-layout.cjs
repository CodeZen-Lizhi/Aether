// 在供应商间距预览页执行，使用真实 Table 和 ProviderTableRow 验证连续缩放。
async (page) => {
  await page.goto('http://localhost:5173/scripts/fixtures/provider-spacing-preview.html')
  await page.locator('tbody tr').first().waitFor({ state: 'attached' })
  const baseline = {}
  for (const width of [840, 1024, 1280, 1920, 2560, 1024]) {
    await page.setViewportSize({ width, height: 900 })
    const result = await page.evaluate(() => {
      const table = document.querySelector('table')
      const container = document.querySelector('.provider-list')
      const rect = table.getBoundingClientRect()
      const cells = [...document.querySelector('tbody tr').children].map(cell => cell.getBoundingClientRect())
      const heads = [...document.querySelector('thead tr').children].map(cell => cell.getBoundingClientRect())
      return {
        visible: getComputedStyle(table.closest('.responsive-list-table')).display !== 'none',
        containerWidth: container.clientWidth,
        tableWidth: rect.width,
        overflow: document.documentElement.scrollWidth > innerWidth,
        aligned: cells.every((cell, i) => Math.abs(cell.x - heads[i].x) < 1 && Math.abs(cell.width - heads[i].width) < 1),
        // 列宽不得远超内容实际占用，避免出现截图里那种大片空白。
        gaps: [...document.querySelector('tbody tr').children].map(cell => {
          const content = cell.firstElementChild
          if (!content) return 0
          return Math.round(cell.getBoundingClientRect().width - content.getBoundingClientRect().width)
        }),
        // 资源统计为 w-fit 双列网格，列宽不足时文字会被挤成多行。
        wrapped: Array.from(document.querySelectorAll('tbody tr'))
          .flatMap(row => Array.from(row.children[2].querySelectorAll('span')))
          .filter(span => span.getBoundingClientRect().height > parseFloat(getComputedStyle(span).lineHeight) * 1.5)
          .map(span => span.textContent.trim()),
        // 缩放时列宽必须完全一致，且提供商信息列要保留可读宽度。
        colWidths: cells.map(cell => Math.round(cell.width)),
        infoWidth: Math.round(cells[0].width),
        // 供应商名称过长时必须截断，不得撑破信息列。
        nameOverflow: [...document.querySelectorAll('tbody tr')].some(row => {
          const cell = row.children[0].getBoundingClientRect()
          return [...row.children[0].querySelectorAll('span')].some(span => {
            if (!span.textContent.trim()) return false
            return span.getBoundingClientRect().width > cell.width
          })
        }),
        actionsRight: cells[5].right,
        tableRight: rect.right,
        buttonsFit: [...document.querySelectorAll('tbody tr')].every(row => {
          const cell = row.children[5].getBoundingClientRect()
          return [...row.children[5].querySelectorAll('button')].every(button => {
            const bounds = button.getBoundingClientRect()
            return bounds.left >= cell.left && bounds.right <= cell.right
          })
        }),
      }
    })
    if (result.overflow) throw new Error(`${width}: 页面横向溢出`)
    if (result.visible) {
      if (result.wrapped.length) throw new Error(`${width}: 资源统计文字换行 ${JSON.stringify(result.wrapped)}`)
      // 除信息列外各列宽度在缩放过程中不得变化：记录首组列宽，后续必须完全相同。
      const fixed = result.colWidths.slice(1).join(',')
      if (!baseline.fixed) baseline.fixed = fixed
      else if (baseline.fixed !== fixed) throw new Error(`${width}: 列宽随缩放变化 ${fixed}（基准 ${baseline.fixed}）`)
      // 信息列允许偏窄：名称靠截断显示；900px 是桌面原生最小窗口，
      // 更窄的视口超出本项目支持范围，只记录不断言。
      if (width >= 900 && result.infoWidth < 150) throw new Error(`${width}: 提供商信息列过窄 ${result.infoWidth}px`)
      if (result.nameOverflow) throw new Error(`${width}: 供应商名称撑破信息列`)
      // 允许的差额只是单元格 padding；列宽远大于内容说明列宽分配与内容不匹配。
      // 资源统计（第 3 列）例外：它的下限用于保证「端点/密钥/模型」不换行，
      // 内容为 w-fit 不会拉伸，因此差额偏大是预期行为，由 wrapped 断言把关。
      const tooWide = result.gaps.findIndex((gap, index) => index !== 0 && index !== 2 && gap > 48)
      if (tooWide > 0) throw new Error(`${width}: 第 ${tooWide + 1} 列过宽，留白 ${result.gaps[tooWide]}px ${JSON.stringify(result)}`)
      if (Math.abs(result.tableWidth - result.containerWidth) > 2 || !result.aligned || !result.buttonsFit || Math.abs(result.actionsRight - result.tableRight) > 2) {
        throw new Error(`${width}: 列宽或对齐错误 ${JSON.stringify(result)}`)
      }
    } else if (width >= 1024) throw new Error(`${width}: 表格意外隐藏`)
    console.log(JSON.stringify({ width, ...result }))
  }
  await page.getByRole('button', { name: '测试长名称' }).click()
  await page.setViewportSize({ width: 1024, height: 900 })
  if (await page.locator('tbody tr').nth(1).evaluate(row => row.scrollWidth > row.clientWidth)) throw new Error('长名称导致行溢出')
  await page.setViewportSize({ width: 1920, height: 1080 })
  await page.screenshot({ path: '.cache/delivery/provider-layout/wide.png', fullPage: true })
}
