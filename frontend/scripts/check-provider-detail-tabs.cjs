// 打开 /scripts/fixtures/provider-detail-tabs.html 后，用 playwright-cli run-code --filename 执行。
// 检查真实抽屉的全部密钥、纵向滚动及分区状态，API 数据和写入由 fixture 隔离。
// eslint-disable-next-line @typescript-eslint/no-unused-expressions -- playwright-cli 执行此函数。
async (page) => {
  const failures = []
  page.on('pageerror', error => failures.push(error.message))
  page.on('console', message => {
    if (message.text().includes('non-element root node')) failures.push(message.text())
  })
  await page.reload()
  await page.setViewportSize({ width: 1280, height: 900 })
  await page.getByRole('button', { name: '添加密钥', exact: true }).waitFor()

  /** 断言分区标题互斥，并检查公共供应商头部始终可见。 */
  async function checkSection(section, provider = 'alpha') {
    await page.getByRole('navigation').getByRole('button', { name: section }).click()
    const headings = (await page.locator('h3:visible').allTextContents()).map(text => text.trim())
    const expected = section === '密钥管理' && provider === 'alpha' ? ['订阅配额', section] : [section]
    if (JSON.stringify(headings) !== JSON.stringify(expected)) {
      throw new Error(`${section}: 可见标题 ${JSON.stringify(headings)}，应为 ${JSON.stringify(expected)}`)
    }
    if (!await page.getByRole('heading', { name: `Fixture ${provider}`, exact: true }).isVisible()) {
      throw new Error('公共供应商头部丢失')
    }
  }

  // 首次打开不得出现另两个分区，不能用点击密钥 Tab 来掩盖初始化错误。
  const initial = (await page.locator('h3:visible').allTextContents()).map(text => text.trim())
  if (JSON.stringify(initial) !== JSON.stringify(['订阅配额', '密钥管理'])) throw new Error(`首屏混入内容: ${initial}`)
  await page.getByText('alpha key 11', { exact: true }).waitFor()
  if (await page.locator('[class~="group/item"]').count() !== 12) throw new Error('密钥未完整加载')
  if (await page.getByRole('button', { name: /^[‹›]$/ }).count()) throw new Error('密钥分页控件仍存在')
  await checkSection('模型列表')
  await page.getByRole('button', { name: '›', exact: true }).filter({ visible: true }).click()
  const modelPage = await page.locator('table:visible').innerText()
  await checkSection('模型映射')
  await page.getByText('alpha model 0', { exact: true }).filter({ visible: true }).click()
  await page.getByText('alpha-upstream-0', { exact: true }).waitFor()
  await checkSection('密钥管理')
  if (!await page.getByText('alpha key 11', { exact: true }).isVisible()) throw new Error('切换 Tab 丢失完整密钥列表')
  await checkSection('模型列表')
  if (await page.locator('table:visible').innerText() !== modelPage) throw new Error('切换 Tab 丢失模型分页')
  await checkSection('模型映射')
  if (!await page.getByText('alpha-upstream-0', { exact: true }).isVisible()) throw new Error('切换 Tab 丢失映射展开状态')

  await page.getByRole('button', { name: '打开 beta', exact: true }).click()
  const switched = (await page.locator('h3:visible').allTextContents()).map(text => text.trim())
  if (JSON.stringify(switched) !== JSON.stringify(['密钥管理'])) throw new Error(`切换供应商未重置: ${switched}`)
  for (const section of ['密钥管理', '模型列表', '模型映射']) {
    await checkSection(section, 'beta')
    if ((await page.locator('.drawer-panel').innerText()).includes('alpha')) throw new Error('加载新供应商时仍显示旧供应商数据')
  }
  // 在 beta 请求仍未完成时返回 alpha，再释放旧响应，检查不会回写另一家供应商的数据。
  await page.getByRole('button', { name: '打开 alpha', exact: true }).click()
  await page.getByText('alpha key 0', { exact: true }).waitFor()
  await page.getByRole('button', { name: '释放 beta', exact: true }).click()
  for (const section of ['密钥管理', '模型列表', '模型映射']) {
    await checkSection(section)
    if ((await page.locator('.drawer-panel').innerText()).includes('beta')) throw new Error('旧响应覆盖当前供应商数据')
  }
  await page.getByRole('button', { name: '打开 beta', exact: true }).click()
  await page.getByText('beta key 0', { exact: true }).waitFor()
  for (const section of ['模型列表', '模型映射', '密钥管理']) await checkSection(section, 'beta')
  if (await page.getByText('alpha key 4', { exact: true }).isVisible()) throw new Error('旧供应商密钥残留')
  await page.getByRole('button', { name: '关闭 fixture', exact: true }).click()
  await page.getByRole('heading', { name: 'Fixture beta', exact: true }).waitFor({ state: 'hidden' })
  await page.getByRole('button', { name: '打开 alpha', exact: true }).click()
  await page.getByText('alpha key 0', { exact: true }).waitFor()
  await checkSection('密钥管理')

  // 每个分区各走一次真实管理调用链，并在重新读取后检查结果。
  const keyRow = page.locator('[class~="group/item"]').filter({ hasText: 'alpha key 11' })
  await keyRow.getByTitle('点击停用', { exact: true }).click()
  await keyRow.getByTitle('点击启用', { exact: true }).waitFor()
  await checkSection('模型列表')
  const modelRow = page.getByRole('row').filter({ has: page.getByText('alpha-model-0', { exact: true }) })
  await modelRow.getByTitle('点击停用', { exact: true }).click()
  await modelRow.getByTitle('点击启用', { exact: true }).waitFor()
  await checkSection('模型映射')
  await page.getByTitle('编辑映射', { exact: true }).first().click()
  await page.getByPlaceholder('搜索或添加自定义提供商模型...').fill('fixture-added-upstream')
  await page.getByText('添加自定义提供商模型', { exact: true }).click()
  await page.getByRole('button', { name: '保存映射', exact: true }).click()
  await page.getByRole('heading', { name: '编辑模型映射', exact: true }).waitFor({ state: 'hidden' })
  await page.getByText('alpha model 0', { exact: true }).filter({ visible: true }).click()
  await page.getByText('fixture-added-upstream', { exact: true }).waitFor()
  await page.getByRole('button', { name: '关闭 fixture', exact: true }).click()
  await page.getByRole('heading', { name: 'Fixture alpha', exact: true }).waitFor({ state: 'hidden' })
  await page.getByRole('button', { name: '打开 alpha', exact: true }).click()
  await keyRow.getByTitle('点击启用', { exact: true }).waitFor()
  await checkSection('模型列表')
  await modelRow.getByTitle('点击启用', { exact: true }).waitFor()
  await checkSection('模型映射')
  await page.getByText('alpha model 0', { exact: true }).filter({ visible: true }).click()
  await page.getByText('fixture-added-upstream', { exact: true }).waitFor()

  const layouts = []
  for (const width of [840, 1024, 1280, 1920, 390]) {
    await page.setViewportSize({ width, height: width === 840 ? 620 : 844 })
    for (const section of ['密钥管理', '模型列表', '模型映射']) {
      await checkSection(section)
      const overflow = await page.locator('.drawer-panel').evaluate(element => element.scrollWidth > element.clientWidth)
      if (overflow) throw new Error(`${width}px ${section} 横向溢出`)
      if (section === '密钥管理') {
        const lastKey = page.getByText('alpha key 11', { exact: true })
        await lastKey.scrollIntoViewIfNeeded()
        const scroll = await page.locator('.drawer-panel').evaluate(element => ({
          top: element.scrollTop, height: element.clientHeight, total: element.scrollHeight,
        }))
        const bounds = await lastKey.boundingBox()
        if (scroll.total <= scroll.height || scroll.top <= 0 || !bounds || bounds.y < 0 || bounds.y + bounds.height > page.viewportSize().height) {
          throw new Error(`${width}px 末尾密钥无法通过纵向滚动访问: ${JSON.stringify(scroll)}`)
        }
        await page.getByRole('button', { name: '添加密钥', exact: true }).scrollIntoViewIfNeeded()
      }
      layouts.push(`${width}:${section}`)
    }
  }
  if (failures.length) throw new Error(failures.join('\n'))
  return { initial, keys: 'all 12 keys rendered without pagination; last key scroll and toggle passed', switching: 'passed', reopen: 'passed', preservedState: 'passed', management: 'key/model toggle, mapping save and reread passed', layouts }
}
