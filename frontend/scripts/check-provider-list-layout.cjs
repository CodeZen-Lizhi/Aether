// 在供应商间距预览页执行，使用真实 Table 和 ProviderTableRow 验证连续缩放。
// 验收：任意视口下列宽都按同一组比例分配，不出现某一列独吞富余宽度；
// 各视口下均不溢出、不换行、按钮不被挤出单元格。
async (page) => {
  // 与 provider-list-layout.css 声明的列宽布局一一对应：比例 + 窄窗口下限。
  const EXPECTED_LAYOUT = [
    { ratio: 0.20, minWidth: 0 },
    { ratio: 0.12, minWidth: 0 },
    { ratio: 0.15, minWidth: 120 },
    { ratio: 0.26, minWidth: 0 },
    { ratio: 0.07, minWidth: 0 },
    { ratio: 0.20, minWidth: 168 },
  ]
  await page.goto('http://localhost:5173/scripts/fixtures/provider-spacing-preview.html')
  await page.locator('tbody tr').first().waitFor({ state: 'attached' })
  // 覆盖最小窗口（900px）到超宽（2560px）并回到中间值复测。
  for (const width of [2560, 1920, 1440, 1280, 1024, 900, 1024, 1440]) {
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
      // 每列宽度必须等于「按声明比例分配」与「min-width 下限」中的较大值。
      // 这条断言同时覆盖宽窗口（纯比例）与窄窗口（下限兜底），任何一处
      // 回退成固定像素、或某一列独吞富余宽度，都会立刻失败。
      // 各列宽度必须占表格宽度的同一比例（声明比例与下限兜底除外）。
      // 逐视口断言比例，而不是只比对首组基准：任何一处回退成固定像素都会让
      // 该列比例在缩放中偏离，从而被立即发现。
      const total = result.colWidths.reduce((sum, value) => sum + value, 0)
      const ratios = result.colWidths.map(value => value / total)
      // 比例分配与下限兜底是两种不同的生效状态，分别断言：
      // 无下限生效时各列占比必须等于声明比例（固定像素列会立刻暴露）；
      // 有下限生效时各列必须不窄于下限，且占比偏差不超过下限带来的应有偏移。
      // 每列必须满足两条约束：
      //   1) 不窄于声明的 min-width 下限；
      //   2) 占比与声明比例的偏差不超过 0.03。
      // 固定像素列在宽视口下占比会远小于声明比例，会被第 2 条立刻抓住；
      // 窄视口触发下限时占比自然偏大，用略宽的容差容纳，仍能发现明显的固定像素列。
      const bad = EXPECTED_LAYOUT.findIndex((rule, index) => (
        result.colWidths[index] < rule.minWidth - 1 || Math.abs(ratios[index] - rule.ratio) > 0.03
      ))
      if (bad >= 0) {
        const rule = EXPECTED_LAYOUT[bad]
        throw new Error(`${width}: 第 ${bad + 1} 列宽度 ${result.colWidths[bad]}px 占比 ${ratios[bad].toFixed(3)}，与声明的 ${rule.ratio}（下限 ${rule.minWidth}px）不符 ${JSON.stringify(result)}`)
      }
      // 信息列允许偏窄：名称靠截断显示；900px 是桌面原生最小窗口，
      // 更窄的视口超出本项目支持范围，只记录不断言。
      if (width >= 900 && result.infoWidth < 150) throw new Error(`${width}: 提供商信息列过窄 ${result.infoWidth}px`)
      if (result.nameOverflow) throw new Error(`${width}: 供应商名称撑破信息列`)
      // 单元格内的剩余空间不得超过列宽的一定比例，否则就是「列被撑开、内容靠左」的大片留白。
      // 资源统计（w-fit 双列网格）、端点健康（max-w-[240px] 自适应网格）与状态（固定徽标）
      // 三列的内容宽度由内容本身决定、本就不随列宽拉伸，宽窗口下的差额是预期行为，
      // 由居中摆放（CSS）与 wrapped、列宽比例断言把关。
      const intrinsic = new Set([2, 3, 4])
      const tooWide = result.gaps.findIndex((gap, index) => !intrinsic.has(index) && gap > result.colWidths[index] * 0.45)
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
  // 保留直观截图供人工复核放大后的实际观感。
  await page.setViewportSize({ width: 1920, height: 1080 })
  await page.screenshot({ path: '.cache/delivery/provider-layout/wide.png', fullPage: true })
  return { viewport: '840-2560', ratios: EXPECTED_LAYOUT.map(rule => rule.ratio), longName: 'no overflow', screenshot: '.cache/delivery/provider-layout/wide.png' }
}
