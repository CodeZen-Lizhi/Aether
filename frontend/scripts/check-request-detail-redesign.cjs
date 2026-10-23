// 请求详情重构的真实组件浏览器检查：覆盖成功、上游错误、本地冷却和多 Key 聚合。
async (page) => {
  const scenarios = [
    { name: 'success', query: 'status=final-success' },
    { name: 'upstream', query: 'status=upstream-failure' },
    { name: 'cooldown', query: 'status=cooldown' },
  ]
  const reports = []

  for (const scenario of scenarios) {
    await page.setViewportSize({ width: 1440, height: 900 })
    await page.goto(`http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?${scenario.query}&test_run=${Date.now()}`, { waitUntil: 'domcontentloaded' })
    await page.waitForFunction(() => Boolean(document.querySelector('[data-request-detail-overview]')), undefined, { timeout: 5000 })
    await page.waitForTimeout(500)

    const metrics = await page.locator('[data-request-detail-overview]').locator(':scope > div').allTextContents()
    if (metrics.length !== 5) throw new Error(`${scenario.name}: 首屏概览不是 5 项，实际 ${metrics.length}`)
    if (!metrics.join(' ').includes('总费用') || !metrics.join(' ').includes('输出 Token') || !metrics.join(' ').includes('输出速度') || !metrics.join(' ').includes('首字耗时')) {
      throw new Error(`${scenario.name}: 首屏关键指标缺失`)
    }

    const detailsOpen = await page.evaluate(() => ({
      billingOpen: document.querySelector('[data-request-billing-details]')?.open,
      rawOpen: document.querySelector('[data-request-detail-raw-data]')?.open,
    }))
    if ((detailsOpen.billingOpen !== undefined && detailsOpen.billingOpen !== false) || detailsOpen.rawOpen !== false) throw new Error(`${scenario.name}: 次级详情没有默认收起`)

    const snapshot = await page.evaluate(() => ({
      conclusion: document.querySelector('[data-request-conclusion]')?.textContent ?? '',
      routeCount: document.querySelectorAll('.route-map').length,
      providerCount: document.querySelectorAll('.provider-lane').length,
    }))
    const search = scenario.name === 'cooldown' ? '本地限流冷却' : scenario.name === 'upstream' ? 'HTTP 500' : '有效响应'
    const conclusionText = snapshot.conclusion
    if (!conclusionText?.includes(search)) throw new Error(`${scenario.name}: 请求结论缺失：${conclusionText}`)

    if (snapshot.routeCount !== 1) throw new Error(`${scenario.name}: 执行路径不可用`)
    const providerCount = snapshot.providerCount
    if (providerCount < 1) throw new Error(`${scenario.name}: 没有供应商路径`)

    if (scenario.name === 'upstream' || scenario.name === 'cooldown') {
      // Key 数量和聚合行由真实组件 DOM 直接验证。
      const attemptCount = await page.locator('.provider-lane').first().locator('.attempt-node').count()
      if (attemptCount !== 2) throw new Error(`${scenario.name}: 同供应商两个 Key 未聚合到一行`)
      await page.locator('.attempt-node').nth(1).evaluate(element => element.click())
      await page.waitForTimeout(30)
      if (!(await page.locator('.detail-panel').textContent())?.includes('Magpie 备用账号')) throw new Error(`${scenario.name}: 第二个 Key 没有切换详情`)
      if (scenario.name === 'cooldown') {
        const cooldownStatus = await page.locator('.status-tag').textContent()
        if (!cooldownStatus?.includes('未发送') || cooldownStatus.includes('HTTP')) throw new Error('cooldown: 本地跳过被错误标成 HTTP 失败')
      }
      await page.locator('.attempt-node').first().evaluate(element => element.click())
      await page.waitForTimeout(30)
      if (!(await page.locator('.detail-panel').textContent())?.includes('Magpie 主账号')) throw new Error(`${scenario.name}: 第一个 Key 没有重新展开详情`)
      if (scenario.name === 'upstream' && !(await page.locator('.detail-panel').textContent())?.includes('GPU 显存不足')) throw new Error('upstream: 已知上游原因没有转换为清晰结论')
      await page.waitForTimeout(50)
      await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}-open.png`, fullPage: false })
      await page.locator('[aria-label="收起当前尝试详情"]').evaluate(element => element.click())
      await page.waitForTimeout(30)
      if (await page.locator('.detail-panel').count() !== 0) throw new Error(`${scenario.name}: 尝试详情无法收起`)
    } else {
      await page.locator('[data-provider-id="provider-0"]').evaluate(element => element.click())
      await page.waitForTimeout(50)
      if (!(await page.locator('.detail-panel').textContent())?.includes('Key A')) throw new Error('success: 供应商没有默认打开第一个 Key')
      await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}-open.png`, fullPage: false })
    }

    for (const width of [800, 540, 390]) {
      await page.setViewportSize({ width, height: 900 })
      await page.waitForTimeout(30)
      const overflow = await page.evaluate(() => {
        const drawer = document.querySelector('[data-request-detail-overview]')?.closest('.overflow-y-auto')
        if (!drawer) return { missing: true }
        return { scrollWidth: drawer.scrollWidth, clientWidth: drawer.clientWidth }
      })
      if (overflow.missing || overflow.scrollWidth > overflow.clientWidth + 1) {
        throw new Error(`${scenario.name}/${width}px: 抽屉页面级横向溢出 ${JSON.stringify(overflow)}`)
      }
      const laneOverflow = await page.evaluate(() => [...document.querySelectorAll('.provider-lane')].map((lane) => ({
        scrollWidth: lane.scrollWidth,
        clientWidth: lane.clientWidth,
      })))
      if (laneOverflow.some(lane => lane.scrollWidth > lane.clientWidth + 1)) {
        throw new Error(`${scenario.name}/${width}px: 供应商路径横向溢出 ${JSON.stringify(laneOverflow)}`)
      }
    }
    await page.setViewportSize({ width: 1440, height: 900 })
    await page.waitForTimeout(100)

    const fixtureScroll = await page.evaluate(() => {
      const scrollContainer = document.querySelector('[data-request-detail-overview]')?.closest('.overflow-y-auto')
      if (!scrollContainer) return null
      const original = scrollContainer.scrollTop
      scrollContainer.scrollTop = 0
      return original
    })
    await page.waitForTimeout(50)
    await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}.png`, fullPage: false })
    if (fixtureScroll != null) {
      await page.evaluate((scrollTop) => {
        const scrollContainer = document.querySelector('[data-request-detail-overview]')?.closest('.overflow-y-auto')
        if (scrollContainer) scrollContainer.scrollTop = scrollTop
      }, fixtureScroll)
    }
    reports.push({ scenario: scenario.name, providerCount, metrics, conclusion: conclusionText.trim() })
  }

  return reports
}
