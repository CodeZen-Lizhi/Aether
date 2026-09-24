async (page) => {
  const variants = [
    {}, { status: 'cancelled' }, { upstream_is_stream: false, client_requested_stream: false },
    { status: 'pending', first_byte_time_ms: null }, { status: 'streaming' },
    { status: 'failed', status_code: 502 }, { is_websocket: true },
    { client_requested_stream: false },
  ]
  const records = variants.map((variant, index) => ({
    id: `usage-type-qa-${index}`, model: 'gpt-6-astra', reasoning_effort: 'high',
    provider: 'input', provider_key_name: '', rate_multiplier: 0.15,
    api_format: 'openai:responses', request_type: 'chat',
    input_tokens: 114600, output_tokens: 45, total_tokens: 114645,
    cache_read_input_tokens: 114000,
    cost: 0.122554, actual_cost: 0.018383, response_time_ms: 12580,
    first_byte_time_ms: 10210, is_stream: true, upstream_is_stream: true,
    client_requested_stream: true, status: 'completed', status_code: 200,
    created_at: new Date(Date.now() - index * 1000).toISOString(), ...variant,
  }))
  await page.unroute(url => url.pathname === '/api/admin/usage/records')
  await page.route(url => url.pathname === '/api/admin/usage/records', route => route.fulfill({
    json: { records, total: records.length, limit: 20, offset: 0, total_is_estimated: false },
  }))
  await page.setViewportSize({ width: 900, height: 640 })
  await page.goto('http://127.0.0.1:5175/admin/usage')
  await page.locator('.usage-records-table:visible tbody tr').first().waitFor()
  await page.evaluate(() => document.fonts.ready)
  return await page.locator('.usage-records-table:visible').innerText()
}
