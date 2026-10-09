// 使用真实控件与隔离 API 数据验证模式切换、失败保留值和窄屏布局。
async (page) => {
  await page.getByLabel('倍率来源').selectOption('upstream')
  await page.getByText('测试分组', { exact: false }).waitFor()
  if (await page.locator('output').textContent() !== '0.3') throw new Error('上游倍率未更新')
  await page.getByRole('button', { name: '同步上游倍率', exact: true }).click()
  await page.getByText('上游未返回该分组的有效倍率', { exact: true }).waitFor()
  if (await page.locator('output').textContent() !== '0.3') throw new Error('失败覆盖旧倍率')
  for (const width of [900, 390]) {
    await page.setViewportSize({ width, height: 640 })
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth)
    if (overflow) throw new Error('倍率控件水平溢出')
  }
  await page.getByLabel('倍率来源').selectOption('manual')
  await page.getByRole('button', { name: '同步上游倍率', exact: true }).waitFor({ state: 'hidden' })
  if (await page.locator('output').textContent() !== '0.3') throw new Error('切回手动丢失倍率')
  console.log('倍率控件模式、同步、失败保留及响应式检查通过')
}
