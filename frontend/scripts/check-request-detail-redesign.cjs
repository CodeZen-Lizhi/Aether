// 请求详情真实组件检查：验证方案 B 纵向执行瀑布、记录身份、真实错误及响应式布局。
async (page) => {
  const consoleErrors = []
  page.on('console', message => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })

  const scenarios = [
    { name: 'success', query: 'status=final-success' },
    { name: 'upstream', query: 'status=upstream-failure' },
    { name: 'cooldown', query: 'status=cooldown' },
    { name: 'multi-provider', query: 'status=multi-provider' },
    { name: 'single-key-failure', query: 'status=single-key-failure' },
    { name: 'real-upstream-error', query: 'status=real-upstream-error' },
  ]
  const reports = []

  for (const scenario of scenarios) {
    await page.setViewportSize({ width: 1440, height: 900 })
    await page.goto(`http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?${scenario.query}&test_run=${Date.now()}`, { waitUntil: 'domcontentloaded' })
    await page.waitForFunction(() => Boolean(document.querySelector('[data-request-trace-waterfall]')), undefined, { timeout: 5000 })
    await page.waitForTimeout(120)

    const metrics = await page.locator('[data-request-detail-overview]').locator(':scope > div').allTextContents()
    if (metrics.length !== 5) throw new Error(`${scenario.name}: 首屏概览不是 5 项，实际 ${metrics.length}`)

    const detailsOpen = await page.evaluate(() => ({
      billing: document.querySelector('[data-request-billing-details]')?.open,
      context: document.querySelector('[data-request-detail-context]')?.open,
      raw: document.querySelector('[data-request-detail-raw-data]')?.open,
      detailCount: document.querySelectorAll('.waterfall-detail').length,
    }))
    if ((detailsOpen.billing !== undefined && detailsOpen.billing !== false)
      || detailsOpen.context !== false || detailsOpen.raw !== false || detailsOpen.detailCount !== 0) {
      throw new Error(`${scenario.name}: 次级详情或 Key 详情没有默认收起：${JSON.stringify(detailsOpen)}`)
    }

    const trace = await page.evaluate(() => {
      const root = document.querySelector('[data-request-trace-waterfall]')
      if (!root) throw new Error('执行瀑布缺失')
      const rows = [...root.querySelectorAll('.waterfall-item')]
      return {
        waterfallCount: document.querySelectorAll('[data-request-trace-waterfall]').length,
        navigatorCount: document.querySelectorAll('.execution-navigator, .inspector-workspace, [data-request-trace-inspector]').length,
        providerIds: [...root.querySelectorAll('.provider-record')].map(node => node.getAttribute('data-provider-id')),
        providerOrder: [...root.querySelectorAll('.waterfall-provider-order')].map(node => node.textContent?.trim() ?? ''),
        rowIds: rows.map(node => node.getAttribute('data-attempt-id')),
        rowLabels: [...root.querySelectorAll('.attempt-stop')].map(node => node.textContent?.replace(/\s+/g, ' ').trim() ?? ''),
        summary: root.querySelector('[data-route-summary]')?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        axisCount: root.querySelectorAll('.waterfall-axis').length,
        axisLabels: [...root.querySelectorAll('.waterfall-axis-track span')].map(node => node.textContent?.trim() ?? ''),
        bars: rows.map(row => {
          const track = row.querySelector('.waterfall-track')
          const bar = row.querySelector('.waterfall-bar')
          if (!track || !bar) return null
          const trackRect = track.getBoundingClientRect()
          const barRect = bar.getBoundingClientRect()
          return {
            id: row.getAttribute('data-attempt-id'),
            leftRatio: trackRect.width > 0 ? (barRect.left - trackRect.left) / trackRect.width : -1,
            widthRatio: trackRect.width > 0 ? barRect.width / trackRect.width : -1,
            background: getComputedStyle(bar).backgroundColor,
          }
        }).filter(Boolean),
        skipped: rows.filter(row => row.getAttribute('data-status') === 'skipped').map(row => ({
          id: row.getAttribute('data-attempt-id'),
          text: row.querySelector('.waterfall-not-sent')?.textContent?.trim() ?? '',
          hasBar: Boolean(row.querySelector('.waterfall-bar')),
        })),
      }
    })

    if (trace.waterfallCount !== 1 || trace.navigatorCount !== 0) {
      throw new Error(`${scenario.name}: 仍存在旧检查器结构：${JSON.stringify(trace)}`)
    }
    if (trace.axisCount !== 1 || trace.axisLabels.length !== 5) throw new Error(`${scenario.name}: 时间轴缺失`)
    if (trace.rowLabels.some(label => /失败|成功|HTTP|[✓✕×–]/.test(label))) {
      throw new Error(`${scenario.name}: Key 行重复展示状态或符号：${JSON.stringify(trace.rowLabels)}`)
    }
    if (trace.skipped.some(item => item.hasBar || !/冷却跳过|未发送/.test(item.text))) {
      throw new Error(`${scenario.name}: 冷却项错误绘制耗时条：${JSON.stringify(trace.skipped)}`)
    }

    if (scenario.name === 'multi-provider') {
      if (JSON.stringify(trace.providerIds) !== JSON.stringify(['provider-a', 'provider-b'])) {
        throw new Error(`multi-provider: 供应商顺序错误：${JSON.stringify(trace)}`)
      }
      if (JSON.stringify(trace.rowIds) !== JSON.stringify([
        'multi-provider-a-key-1', 'multi-provider-a-key-2', 'multi-provider-b-key-1', 'multi-provider-b-key-2',
      ])) throw new Error(`multi-provider: Key 调度顺序错误：${JSON.stringify(trace.rowIds)}`)
      if (trace.summary !== '2 家供应商3 次上游请求1 次冷却跳过') {
        throw new Error(`multi-provider: 摘要语义错误：${trace.summary}`)
      }
      if (trace.bars.length !== 3) throw new Error(`multi-provider: 上游请求条数量错误：${JSON.stringify(trace.bars)}`)
      for (let index = 1; index < trace.bars.length; index += 1) {
        if (trace.bars[index].leftRatio <= trace.bars[index - 1].leftRatio) {
          throw new Error(`multi-provider: 瀑布起点未按时间递增：${JSON.stringify(trace.bars)}`)
        }
      }
      const failedBar = trace.bars.find(bar => bar.id === 'multi-provider-a-key-1')
      const successBar = trace.bars.find(bar => bar.id === 'multi-provider-b-key-2')
      if (!failedBar || !successBar || failedBar.background === successBar.background) {
        throw new Error(`multi-provider: 成败时间条没有颜色差异：${JSON.stringify(trace.bars)}`)
      }
    }

    if (scenario.name === 'single-key-failure' && (
      trace.rowIds[0] !== 'single-key-429' || trace.summary !== '1 家供应商1 次上游请求'
    )) throw new Error(`single-key-failure: 单 Key 失败语义错误：${JSON.stringify(trace)}`)

    await page.locator('.attempt-stop').first().evaluate(element => element.click())
    await page.waitForFunction(() => document.querySelectorAll('.waterfall-detail').length === 1)
    const opened = await page.evaluate(() => {
      const row = document.querySelector('.waterfall-item.is-open')
      const detail = document.querySelector('.waterfall-detail')
      return {
        rowId: row?.getAttribute('data-attempt-id') ?? '',
        detailId: detail?.getAttribute('data-attempt-id') ?? '',
        expanded: row?.querySelector('.attempt-stop')?.getAttribute('aria-expanded'),
        keyText: detail?.querySelector('.waterfall-detail-key')?.textContent?.trim() ?? '',
      }
    })
    if (!opened.rowId || opened.rowId !== opened.detailId || opened.expanded !== 'true' || !opened.keyText) {
      throw new Error(`${scenario.name}: Key 详情未绑定同一记录：${JSON.stringify(opened)}`)
    }
    await page.locator('.attempt-stop').first().evaluate(element => element.click())
    await page.waitForFunction(() => document.querySelectorAll('.waterfall-detail').length === 0)

    if (scenario.name === 'upstream') {
      await page.locator('.attempt-stop').first().evaluate(element => element.click())
      await page.waitForFunction(() => Boolean(document.querySelector('.error-title')))
      const detailText = await page.locator('.waterfall-detail').textContent()
      const reason = await page.locator('.error-title').textContent()
      if (!reason?.includes('Failed to load model: CUDA out of memory')
        || (detailText?.match(/Failed to load model: CUDA out of memory/g) ?? []).length !== 1) {
        throw new Error(`upstream: 真实上游原因缺失或重复：${reason}`)
      }
      await page.locator('.attempt-stop').first().evaluate(element => element.click())
    }

    if (scenario.name === 'real-upstream-error') {
      await page.locator('.attempt-stop').first().evaluate(element => element.click())
      await page.waitForFunction(() => Boolean(document.querySelector('.error-title')))
      const detail = await page.evaluate(() => ({
        cause: document.querySelector('.error-title')?.textContent?.trim() ?? '',
        detailText: document.querySelector('.waterfall-detail')?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        toggle: document.querySelector('[data-upstream-response] summary')?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        forbidden: /技术详情|更多字段|原因说明/.test(document.body.textContent ?? ''),
      }))
      if (!detail.cause.includes('service info not found') || detail.detailText.includes('retryable status 400')
        || !detail.toggle.includes('上游响应') || detail.forbidden) {
        throw new Error(`real-upstream-error: 错误层级仍含包装信息或旧入口：${JSON.stringify(detail)}`)
      }
      await page.locator('[data-upstream-response] summary').evaluate(element => element.click())
      await page.locator('[data-upstream-response] .error-upstream-response-json').waitFor({ state: 'visible' })
    }

    for (const width of [800, 540, 390]) {
      await page.setViewportSize({ width, height: 900 })
      await page.waitForTimeout(60)
      const layout = await page.evaluate(() => {
        const drawer = document.querySelector('[data-request-detail-overview]')?.closest('.overflow-y-auto')
        const traceRoot = document.querySelector('[data-request-trace-waterfall]')
        return {
          page: drawer ? { scrollWidth: drawer.scrollWidth, clientWidth: drawer.clientWidth } : null,
          trace: traceRoot ? { scrollWidth: traceRoot.scrollWidth, clientWidth: traceRoot.clientWidth } : null,
          rows: [...document.querySelectorAll('.waterfall-row')].map(node => node.scrollWidth <= node.clientWidth + 1),
        }
      })
      if (!layout.page || layout.page.scrollWidth > layout.page.clientWidth + 1
        || !layout.trace || layout.trace.scrollWidth > layout.trace.clientWidth + 1
        || layout.rows.some(ok => !ok)) {
        throw new Error(`${scenario.name}/${width}px: 执行瀑布横向溢出 ${JSON.stringify(layout)}`)
      }
      if (scenario.name === 'multi-provider') {
        await page.screenshot({ path: `/tmp/aether-request-detail-multi-provider-${width}.png` })
      }
    }

    await page.setViewportSize({ width: 1440, height: 900 })
    await page.waitForTimeout(100)
    await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}.png` })
    reports.push({ scenario: scenario.name, metrics, summary: trace.summary })
  }

  await page.goto('http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?status=multi-provider&theme=dark', { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(document.querySelector('[data-request-trace-waterfall]')), undefined, { timeout: 5000 })
  await page.waitForTimeout(320)
  const darkBars = await page.evaluate(() => [...document.querySelectorAll('.waterfall-bar')].map(node => ({
    background: getComputedStyle(node).backgroundColor,
    image: getComputedStyle(node).backgroundImage,
  })))
  if (new Set(darkBars.map(bar => bar.image)).size < 2 || darkBars.some(bar => bar.image === 'none')) {
    throw new Error(`深色主题：状态条未可辨：${JSON.stringify(darkBars)}`)
  }
  await page.screenshot({ path: '/tmp/aether-request-detail-dark.png' })

  if (consoleErrors.length > 0) throw new Error(`浏览器 Console 存在未解释错误：${JSON.stringify(consoleErrors.slice(0, 3))}`)
  return reports
}
