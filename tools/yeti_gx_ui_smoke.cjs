// Requires playwright and the UI dev server on localhost:8765.
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1180, height: 900 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('http://127.0.0.1:8765/?lang=en&kb=us');
    await page.waitForFunction(() => Microphone.status?.connected);
    await page.locator('#dev-tabs [data-dev="microphone"]').click();
    assert.equal(await page.locator('#microphone-page').isVisible(), true);
    assert.equal(await page.locator('#keyboard-page').isVisible(), false);
    assert.equal(await page.locator('#mic-meter-value').innerText(), '-12.0 dBFS');
    await page.locator('#mic-gain').evaluate(el => { el.value = '75'; el.dispatchEvent(new Event('input', { bubbles: true })); });
    await page.evaluate(() => Microphone.update({ ...Microphone.status, gain: 66 }));
    assert.equal(await page.locator('#mic-gain-value').innerText(), '75%');
    assert.equal(await page.locator('#mic-hardware-gain').innerText(), 'On mic: 66%');
    await page.locator('#mic-gain-apply').click();
    await page.waitForFunction(() => !Microphone.sending && Microphone.status.gain === 75);
    assert.equal(await page.locator('#mic-gain-value').innerText(), '75%');
    await page.locator('#mic-mute').click();
    await page.waitForFunction(() => !Microphone.sending && Microphone.status.muted);
    assert.equal(await page.locator('#mic-mute').innerText(), 'Unmute microphone');
    await page.locator('#mic-lock').check();
    await page.waitForFunction(() => !Microphone.sending && Microphone.status.smart_lock);
    await page.locator('#mic-lighting-enabled').check();
    await page.locator('#mic-zones [data-zone="0"]').fill('#ff1122');
    const survived = await page.evaluate(() => {
      const node = document.querySelector('#mic-zones [data-zone="0"]');
      Microphone.update({ ...Microphone.status, input: { ...Microphone.status.input, peak: 0.5 } });
      return node === document.querySelector('#mic-zones [data-zone="0"]');
    });
    assert.equal(survived, true, 'meter refresh must preserve the open color editor');
    assert.equal(await page.locator('#mic-zones [data-zone="0"]').inputValue(), '#ff1122');
    await page.locator('#mic-lighting-save').click();
    await page.waitForFunction(() => !Microphone.sending && !Microphone.dirty);
    assert.equal(await page.evaluate(() => Microphone.status.settings.colors[0]), '#ff1122');
    assert.equal(await page.evaluate(() => Microphone.status.muted), true, 'lighting never unmutes the mic');
    await page.locator('#mic-zones [data-zone="1"]').fill('#00ff00');
    await page.locator('#dev-tabs [data-dev="keyboard"]').click();
    await page.locator('#dev-tabs [data-dev="microphone"]').click();
    assert.equal(await page.locator('#mic-zones [data-zone="1"]').inputValue(), '#00ff00');
    await page.locator('#mic-lighting-reset').click();
    assert.equal(await page.locator('#mic-zones [data-zone="1"]').inputValue(), '#1e56e4');
    if (process.env.YETI_GX_SCREENSHOT) await page.screenshot({ path: process.env.YETI_GX_SCREENSHOT });
    await page.locator('#mic-lighting-enabled').uncheck();
    await page.locator('#mic-lighting-save').click();
    await page.waitForFunction(() => !Microphone.sending && Microphone.status.settings.colors === null);
    await page.setViewportSize({ width: 980, height: 760 });
    const gear = await page.locator('#btn-settings').boundingBox();
    assert.ok(gear.x + gear.width <= 980, 'all device tabs and settings must fit the minimum window');
    await page.goto('http://127.0.0.1:8765/?lang=en&kb=us&nomic');
    await page.waitForFunction(() => Microphone.status && !Microphone.status.connected);
    await page.locator('#dev-tabs [data-dev="microphone"]').click();
    for (const selector of ['#mic-gain', '#mic-gain-apply', '#mic-mute', '#mic-lock', '#mic-lighting-enabled', '#mic-lighting-save']) {
      assert.equal(await page.locator(selector).isDisabled(), true, `${selector}: unavailable hardware must not accept writes`);
    }
    assert.deepEqual(errors, []);
    console.log('Yeti GX UI: gain, mute, Smart Audio Lock, level meter, independent colors, drafts, absent device and window sizing passed.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
