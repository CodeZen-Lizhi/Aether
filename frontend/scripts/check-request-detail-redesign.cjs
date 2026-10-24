// 请求详情真实组件检查：验证横向请求链路、真实调用、状态色、固定高度及详情展开。
async (page) => {
  const consoleErrors = []
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  })

  const scenarios = [
    { name: 'success', query: 'status=final-success' },
    { name: 'upstream', query: 'status=upstream-failure' },
    { name: 'multi-provider', query: 'status=multi-provider' },
    { name: 'single-key-failure', query: 'status=single-key-failure' },
    { name: 'real-upstream-error', query: 'status=real-upstream-error' },
    { name: 'long-error', query: 'status=single-key-failure&long=true' },
  ]
  const reports = []

  const layoutChecks = []
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
      if (!root) throw new Error('执行路径缺失')
      const centerY = (node) => {
        const rect = node.getBoundingClientRect()
        return Math.round((rect.top + rect.bottom) / 2)
      }
      const rows = [...root.querySelectorAll('.waterfall-item')]
      const entry = root.querySelector('.path-entry')
      const providers = [...root.querySelectorAll('.path-provider')]
      const response = root.querySelector('.path-response')
      return {
        oldStructures: root.querySelectorAll('.waterfall-axis, .waterfall-groups, .waterfall-status-mark').length,
        oldInspector: document.querySelectorAll('[data-request-trace-inspector], .execution-navigator, .inspector-workspace').length,
        providerIds: providers.map(node => node.getAttribute('data-provider-id')),
        providerOrders: [...root.querySelectorAll('.waterfall-provider-order')].map(node => node.textContent?.trim() ?? ''),
        providerNames: [...root.querySelectorAll('.provider-record-name')].map(node => node.textContent?.trim() ?? ''),
        providerLabels: [...root.querySelectorAll('.path-provider-label')].map(node => node.textContent?.replace(/\s+/g, ' ').trim() ?? ''),
        entryText: entry?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        entryLineCount: entry?.querySelectorAll(':scope > .path-node-kicker, :scope > .path-entry-line').length ?? 0,
        entryMethod: root.querySelector('.path-method')?.textContent?.trim() ?? '',
        entryPath: root.querySelector('.path-endpoint')?.textContent?.trim() ?? '',
        responseTitle: root.querySelector('.path-response .path-node-kicker')?.textContent?.trim() ?? '',
        responseStatus: root.querySelector('.path-response-status')?.textContent?.trim() ?? '',
        responseStatusRightAligned: (() => {
          const head = root.querySelector('.path-response-head')
          const title = head?.querySelector('.path-node-kicker')
          const status = head?.querySelector('.path-response-status')
          if (!head || !title || !status) return false
          return status.getBoundingClientRect().left > title.getBoundingClientRect().right
        })(),
        entryColor: (() => {
          const method = root.querySelector('.path-method')
          return method ? getComputedStyle(method).color : ''
        })(),
        rowIds: rows.map(node => node.getAttribute('data-attempt-id')),
        rowStatuses: rows.map(node => node.getAttribute('data-status')),
        keyNames: [...root.querySelectorAll('.waterfall-key')].map(node => node.textContent?.trim() ?? ''),
        keyHeights: rows.map(node => Math.round(node.getBoundingClientRect().height)),
        keyClasses: rows.map(node => node.className),
        keyTexts: rows.map(node => node.textContent?.replace(/\s+/g, ' ').trim() ?? ''),
        summary: root.querySelector('[data-route-summary]')?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        centers: {
          entry: entry ? centerY(entry) : -1,
          providers: providers.map(centerY),
          response: response ? centerY(response) : -1,
        },
        positions: {
          entryLeft: entry?.getBoundingClientRect().left ?? -1,
          providerLefts: providers.map(node => node.getBoundingClientRect().left),
          responseLeft: response?.getBoundingClientRect().left ?? -1,
        },
        providerBorderStyles: providers.map(node => getComputedStyle(node).borderStyle),
        keyHopCount: root.querySelectorAll('.path-key-hop').length,
        keyHopColor: (() => {
          const hop = root.querySelector('.path-key-hop')
          return hop ? getComputedStyle(hop, '::before').backgroundColor : ''
        })(),
        links: [...root.querySelectorAll('.path-link')].map(node => ({
          label: node.innerText.trim(),
          width: Math.round(node.getBoundingClientRect().width),
          lineWidth: Math.round(Number.parseFloat(getComputedStyle(node, '::before').width)),
        })),
        linkGaps: (() => {
          const stage = root.querySelector('.path-stage')
          const nodes = [...stage.children].map(node => node.getBoundingClientRect())
          return nodes.slice(1).map((rect, index) => Math.round(rect.left - nodes[index].right))
        })(),
        scroll: (() => {
          const node = root.querySelector('[data-request-path-scroll]')
          return {
            clientWidth: node?.clientWidth ?? 0,
            scrollWidth: node?.scrollWidth ?? 0,
            overflowX: node ? getComputedStyle(node).overflowX : '',
          }
        })(),
        responseText: response?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        containsSkippedLanguage: /冷却跳过|未发送|未尝试|未进入/.test(root.textContent ?? ''),
      }
    })

    if (trace.oldStructures !== 0 || trace.oldInspector !== 0) {
      throw new Error(`${scenario.name}: 仍存在旧瀑布或检查器结构：${JSON.stringify(trace)}`)
    }
    if (trace.entryLineCount !== 2 || trace.entryMethod !== 'POST' || trace.entryPath !== '/v1/images/generations') {
      throw new Error(`${scenario.name}: 请求入口不是两行真实请求路径：${JSON.stringify(trace)}`)
    }
    if (trace.responseTitle !== '请求结果' || !trace.responseStatus || !trace.responseStatusRightAligned) {
      throw new Error(`${scenario.name}: 请求结果标题或状态码位置错误：${JSON.stringify(trace)}`)
    }
    if (scenario.name === 'success' && trace.entryColor !== 'rgb(21, 128, 61)') {
      throw new Error(`success: 请求入口不是成功绿：${JSON.stringify(trace.entryColor)}`)
    }
    if (scenario.name === 'upstream' && trace.entryColor !== 'rgb(220, 38, 38)') {
      throw new Error(`upstream: 请求入口不是失败正红：${JSON.stringify(trace.entryColor)}`)
    }
    if (!trace.responseText.includes('总耗时')) {
      throw new Error(`${scenario.name}: 请求结果缺少总耗时：${JSON.stringify(trace)}`)
    }
    if (trace.keyNames.some(name => !name || /^\S$/.test(name))) {
      throw new Error(`${scenario.name}: Key 名称缺失或被挤断：${JSON.stringify(trace.keyNames)}`)
    }
    if (trace.keyHeights.some(height => height !== 88)) {
      throw new Error(`${scenario.name}: Key 卡片高度不固定：${JSON.stringify(trace.keyHeights)}`)
    }
    if (trace.links.some(link => link.label)) {
      throw new Error(`${scenario.name}: 连接线仍包含文字：${JSON.stringify(trace.links)}`)
    }
    if (trace.linkGaps.some(gap => Math.abs(gap) > 1)) {
      throw new Error(`${scenario.name}: 路径节点之间未贴合：${JSON.stringify(trace.linkGaps)}`)
    }
    if (trace.providerBorderStyles.some(style => style !== 'dashed')) {
      throw new Error(`${scenario.name}: 供应商框不是中性虚线分组：${JSON.stringify(trace)}`)
    }
    if (new Set([trace.centers.entry, ...trace.centers.providers, trace.centers.response]).size !== 1) {
      throw new Error(`${scenario.name}: 请求入口、供应商与请求结果未垂直居中：${JSON.stringify(trace.centers)}`)
    }
    if (!(trace.positions.entryLeft < Math.min(...trace.positions.providerLefts)
      && Math.max(...trace.positions.providerLefts) < trace.positions.responseLeft)) {
      throw new Error(`${scenario.name}: 请求入口、供应商与请求结果顺序错误：${JSON.stringify(trace.positions)}`)
    }
    if (trace.scroll.overflowX !== 'auto') {
      throw new Error(`${scenario.name}: 路径容器未使用内部横向滚动：${JSON.stringify(trace.scroll)}`)
    }

    if (scenario.name === 'multi-provider') {
      if (JSON.stringify(trace.providerIds) !== JSON.stringify(['provider-a', 'provider-b'])) {
        throw new Error(`multi-provider: 供应商顺序错误：${JSON.stringify(trace)}`)
      }
      if (JSON.stringify(trace.rowIds) !== JSON.stringify([
        'multi-provider-a-key-1', 'multi-provider-a-key-2', 'multi-provider-b-key-2',
      ])) throw new Error(`multi-provider: 真实 Key 调度顺序错误：${JSON.stringify(trace.rowIds)}`)
      if (JSON.stringify(trace.providerOrders) !== JSON.stringify(['01', '02'])) {
        throw new Error(`multi-provider: 供应商序号错误：${JSON.stringify(trace.providerOrders)}`)
      }
      if (trace.providerNames.join('|') !== 'Magpie|Ocean') {
        throw new Error(`multi-provider: 供应商名称错误：${JSON.stringify(trace.providerNames)}`)
      }
      if (trace.providerLabels.some(label => label.includes('供应商'))) {
        throw new Error(`multi-provider: 供应商标题仍重复类型文字：${JSON.stringify(trace.providerLabels)}`)
      }
      if (trace.summary !== '2 家供应商，3 次上游请求') {
        throw new Error(`multi-provider: 摘要未表达真实上游请求数：${JSON.stringify(trace.summary)}`)
      }
      if (trace.keyHopCount !== 1 || trace.keyHopColor !== 'rgb(220, 38, 38)') {
        throw new Error(`multi-provider: 换 Key 箭头缺失或不是正红：${JSON.stringify(trace)}`)
      }
      if (trace.containsSkippedLanguage) {
        throw new Error(`multi-provider: 未发送 Key 混入实际路径：${JSON.stringify(trace.rowIds)}`)
      }
      const failedKey = trace.rowIds.indexOf('multi-provider-a-key-1')
      const successKey = trace.rowIds.indexOf('multi-provider-b-key-2')
      if (!trace.keyClasses[failedKey]?.includes('path-key--failed') || !trace.keyClasses[successKey]?.includes('path-key--success')) {
        throw new Error(`multi-provider: Key 成败语义类错误：${JSON.stringify(trace.keyClasses)}`)
      }
      if (!trace.keyTexts[failedKey]?.includes('500') || !trace.keyTexts[successKey]?.includes('200')) {
        throw new Error(`multi-provider: Key HTTP 状态错误：${JSON.stringify(trace.keyTexts)}`)
      }
      if (!trace.responseText.includes('200') || !trace.responseText.includes('110ms')) {
        throw new Error(`multi-provider: 请求结果不是成功 200 或总耗时错误：${JSON.stringify(trace.responseText)}`)
      }
    }

    if (scenario.name === 'long-error') {
      const failureColor = await page.evaluate(() => {
        const failed = document.querySelector('.path-key--failed')
        return {
          http: failed ? getComputedStyle(failed.querySelector('.path-key-http')).color : '',
          border: failed ? getComputedStyle(failed).borderColor : '',
        }
      })
      if (failureColor.http !== 'rgb(220, 38, 38)' || failureColor.border !== 'rgb(248, 113, 113)') {
        throw new Error(`long-error: 失败节点不是正红：${JSON.stringify(failureColor)}`)
      }
      const longReason = await page.evaluate(() => {
        const reason = document.querySelector('.path-key-reason')
        const card = document.querySelector('.path-key')
        if (!reason || !card) return null
        const style = getComputedStyle(reason)
        return {
          cardHeight: Math.round(card.getBoundingClientRect().height),
          reasonText: reason.textContent?.trim() ?? '',
          reasonTitle: reason.getAttribute('title') ?? '',
          clientWidth: reason.clientWidth,
          scrollWidth: reason.scrollWidth,
          whiteSpace: style.whiteSpace,
        }
      })
      if (!longReason || longReason.cardHeight !== 88
        || longReason.whiteSpace !== 'nowrap'
        || longReason.scrollWidth <= longReason.clientWidth
        || !longReason.reasonText.includes('upstream_response_error')
        || !longReason.reasonTitle.includes('upstream_response_error')) {
        throw new Error(`long-error: 长错误未单行截断或撑高卡片：${JSON.stringify(longReason)}`)
      }
    }

    const lifecycleBadge = await page.evaluate(() => {
      const badge = document.querySelector('[data-request-lifecycle-status]')
      if (!badge) return null
      const style = getComputedStyle(badge)
      return { text: badge.textContent?.trim() ?? '', color: style.color, background: style.backgroundColor }
    })
    if (scenario.name === 'success' && (
      lifecycleBadge?.text !== '200'
      || lifecycleBadge.color !== 'rgb(21, 128, 61)'
      || trace.responseStatus !== '200'
    )) {
      throw new Error(`success: 请求级成功色错误：${JSON.stringify({ lifecycleBadge, trace })}`)
    }
    if (scenario.name === 'upstream' && (
      lifecycleBadge?.text !== '500'
      || lifecycleBadge.color !== 'rgb(220, 38, 38)'
      || trace.responseStatus !== '500'
    )) {
      throw new Error(`upstream: 请求级失败色错误：${JSON.stringify({ lifecycleBadge, trace })}`)
    }

    await page.locator('.attempt-stop').first().evaluate(element => element.click())
    await page.waitForFunction(() => document.querySelectorAll('.waterfall-detail').length === 1)
    const opened = await page.evaluate(() => {
      const row = document.querySelector('.waterfall-item.is-open')
      const detail = document.querySelector('.waterfall-detail')
      return {
        rowId: row?.getAttribute('data-attempt-id') ?? '',
        detailId: detail?.getAttribute('data-attempt-id') ?? '',
        expanded: row?.getAttribute('aria-expanded'),
        hasDuplicateIdentity: Boolean(detail?.querySelector('.waterfall-detail-identity, .waterfall-detail-key, .waterfall-detail-provider')),
        detailText: detail?.textContent?.replace(/\s+/g, ' ').trim() ?? '',
        status: detail?.getAttribute('data-status') ?? '',
      }
    })
    if (!opened.rowId || opened.rowId !== opened.detailId || opened.expanded !== 'true' || opened.hasDuplicateIdentity) {
      throw new Error(`${scenario.name}: Key 详情未绑定同一记录：${JSON.stringify(opened)}`)
    }
    if (!opened.detailText.includes('耗时') || !opened.detailText.includes('HTTP')) {
      throw new Error(`${scenario.name}: Key 详情缺少单次调用事实：${JSON.stringify(opened)}`)
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
      await page.locator('.attempt-stop').first().evaluate(element => element.click())
    }

    for (const width of [800, 540, 390]) {
      await page.setViewportSize({ width, height: 900 })
      await page.waitForTimeout(60)
      const layout = await page.evaluate(() => {
        const drawer = document.querySelector('[data-request-detail-overview]')?.closest('.overflow-y-auto')
        const root = document.querySelector('[data-request-trace-waterfall]')
        const scroll = root?.querySelector('[data-request-path-scroll]')
        return {
          page: drawer ? { scrollWidth: drawer.scrollWidth, clientWidth: drawer.clientWidth } : null,
          root: root ? { scrollWidth: root.scrollWidth, clientWidth: root.clientWidth } : null,
          scroll: scroll ? {
            scrollWidth: scroll.scrollWidth,
            clientWidth: scroll.clientWidth,
            overflowX: getComputedStyle(scroll).overflowX,
          } : null,
          providers: root ? [...root.querySelectorAll('.path-provider')].map(node => {
            const rect = node.getBoundingClientRect()
            const centerY = Math.round((rect.top + rect.bottom) / 2)
            return { top: Math.round(rect.top), centerY, left: Math.round(rect.left) }
          }) : [],
          keyHeights: root ? [...root.querySelectorAll('.path-key')].map(node => Math.round(node.getBoundingClientRect().height)) : [],
          centers: root ? (() => {
            const entry = root.querySelector('.path-entry')
            const response = root.querySelector('.path-response')
            const centerY = (node) => {
              const rect = node.getBoundingClientRect()
              return Math.round((rect.top + rect.bottom) / 2)
            }
            return {
              entry: entry ? centerY(entry) : -1,
              providers: [...root.querySelectorAll('.path-provider')].map(centerY),
              response: response ? centerY(response) : -1,
            }
          })() : null,
        }
      })
      if (!layout.page || layout.page.scrollWidth > layout.page.clientWidth + 1
        || !layout.root || layout.root.scrollWidth > layout.root.clientWidth + 1
        || !layout.scroll || layout.scroll.overflowX !== 'auto'
        || (scenario.name === 'multi-provider' && width === 390
          && layout.scroll.scrollWidth <= layout.scroll.clientWidth)
        || new Set(layout.providers.map(provider => provider.centerY)).size !== 1
        || layout.keyHeights.some(height => height !== 88)
        || !layout.centers
        || new Set([layout.centers.entry, ...layout.centers.providers, layout.centers.response]).size !== 1) {
        throw new Error(`${scenario.name}/${width}px: 横向路径布局错误 ${JSON.stringify(layout)}`)
      }
      layoutChecks.push({ scenario: scenario.name, width, ...layout })
      await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}-${width}.png` })
    }

    await page.setViewportSize({ width: 1440, height: 900 })
    await page.waitForTimeout(100)
    await page.screenshot({ path: `/tmp/aether-request-detail-${scenario.name}.png` })
    reports.push({ scenario: scenario.name, entry: trace.entryText, summary: trace.summary })
  }

  await page.goto('http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?status=multi-provider&theme=dark', { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(document.querySelector('[data-request-trace-waterfall]')), undefined, { timeout: 5000 })
  await page.waitForTimeout(320)
  const dark = await page.evaluate(() => {
    const failed = document.querySelector('.path-key--failed')
    const success = document.querySelector('.path-key--success')
    return {
      failedBorder: failed ? getComputedStyle(failed).borderColor : '',
      successBorder: success ? getComputedStyle(success).borderColor : '',
      failedText: failed ? getComputedStyle(failed.querySelector('.path-key-http')).color : '',
      successText: success ? getComputedStyle(success.querySelector('.path-key-http')).color : '',
    }
  })
  if (!dark.failedBorder || dark.failedBorder === dark.successBorder
    || !dark.failedText || dark.failedText === dark.successText) {
    throw new Error(`深色主题：成功与失败路径不可辨：${JSON.stringify(dark)}`)
  }
  await page.screenshot({ path: '/tmp/aether-request-detail-dark.png' })

  await page.goto('http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?status=pending&lifecycle=streaming&locale=en-US', { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(document.querySelector('[data-request-trace-waterfall]')), undefined, { timeout: 5000 })
  await page.waitForTimeout(100)
  const englishPending = await page.evaluate(() => ({
    status: document.querySelector('.path-response-status')?.textContent?.trim() ?? '',
    reasons: [...document.querySelectorAll('.path-key-reason')].map(node => node.textContent?.trim() ?? ''),
    body: document.body.textContent?.replace(/\s+/g, ' ').trim() ?? '',
  }))
  if (englishPending.status !== 'In progress'
    || !englishPending.reasons.includes('Streaming')
    || englishPending.body.includes('进行中') || englishPending.body.includes('传输中')) {
    throw new Error(`英文界面动态状态未翻译：${JSON.stringify(englishPending)}`)
  }

  await page.goto('http://127.0.0.1:5173/scripts/fixtures/request-detail-layout.html?status=upstream-failure&locale=en-US', { waitUntil: 'domcontentloaded' })
  await page.waitForFunction(() => Boolean(document.querySelector('[data-request-trace-waterfall]')), undefined, { timeout: 5000 })
  await page.locator('.attempt-stop').first().evaluate(element => element.click())
  await page.waitForFunction(() => Boolean(document.querySelector('.waterfall-detail')))
  await page.waitForTimeout(100)
  const englishText = await page.evaluate(() => ({
    labels: [...document.querySelectorAll('.waterfall-detail dt')].map(node => node.textContent?.trim() ?? ''),
    body: document.body.textContent?.replace(/\s+/g, ' ').trim() ?? '',
  }))
  if (!englishText.labels.includes('Credential') || englishText.labels.includes('凭据')
    || !englishText.body.includes('Request entry') || !englishText.body.includes('Request result')
    || englishText.body.includes('请求入口') || englishText.body.includes('请求结果')) {
    throw new Error(`英文界面主标签未翻译：${JSON.stringify(englishText)}`)
  }
  await page.screenshot({ path: '/tmp/aether-request-detail-en.png' })

  if (consoleErrors.length > 0) throw new Error(`浏览器 Console 存在未解释错误：${JSON.stringify(consoleErrors.slice(0, 3))}`)
  return { reports, layoutChecks }
}
