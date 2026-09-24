async (page) => {
  const records = ['high', 'medium', 'high'].map((effort, index) => ({
    id: `badge-spacing-qa-${index}`, model: 'gpt-6-astra',
    requested_reasoning_effort: effort, reasoning_effort: effort,
    provider: 'input', provider_key_name: '', rate_multiplier: 0.15,
    api_format: 'openai:responses', request_type: 'chat',
    input_tokens: 114600, output_tokens: 45, total_tokens: 114645,
    cache_read_input_tokens: 114000,
    cost: 0.122554, actual_cost: 0.018383, response_time_ms: 12580,
    first_byte_time_ms: 10210, is_stream: true, upstream_is_stream: true,
    status: 'completed', status_code: 200, created_at: `2026-09-12T08:48:${41 - index * 7}Z`,
  }))
  await page.route(url => url.pathname === '/api/admin/usage/records', route => route.fulfill({
    json: { records, total: records.length, limit: 20, offset: 0, total_is_estimated: false },
  }))
  await page.setViewportSize({ width: 1024, height: 768 })
  await page.goto('http://127.0.0.1:5175/admin/usage')
  await page.locator('[data-usage-model-source]:visible').first().waitFor()
  await page.evaluate(() => document.fonts.ready)
  return await page.locator('main').innerText()
}
