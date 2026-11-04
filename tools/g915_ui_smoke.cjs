// Requires playwright. Start the UI dev server on localhost:8765 first.
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1180, height: 900 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.goto('http://127.0.0.1:8765/?lang=en&kb=us');
    await page.locator('#dev-tabs [data-dev="keyboard"]').click();
    await page.locator('#kb-keys [data-key="G5"]').waitFor();
    assert.match(await page.locator('#kb-state').innerText(), /LIGHTSPEED/);
    assert.equal(await page.locator('#kb-save').isDisabled(), true);
    await page.locator('#kb-enabled').check();
    await page.locator('#kb-lighting').check();
    await page.locator('#kb-keys [data-key="G1"]').click();
    await page.locator('#picker-tabs [data-tab="keyboard"]').click();
    await page.locator('#picker [data-code="A"]').click();
    assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(), /A/);
    await page.locator('#kb-macros summary').click();
    await page.locator('#kb-macro-list [data-act="add"]').click();
    assert.equal(await page.evaluate(() => Keyboard.editor.sel), 'macro-1');
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving);
    assert.equal(await page.locator('#kb-save').isDisabled(), true);
    assert.equal(await page.evaluate(() => Keyboard.status.settings.profile.banks.M1.G1), 'A');
    assert.ok(await page.evaluate(() => Keyboard.status.settings.profile.macros['macro-1']));
    await page.locator('#kb-banks [data-bank="M2"]').click();
    assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(), /Unassigned/);
    await page.locator('#kb-lighting').uncheck();
    await page.locator('#kb-reset').click();
    assert.equal(await page.locator('#kb-lighting').isChecked(), true);
    await page.locator('#dev-tabs [data-dev="g13"]').click();
    assert.equal(await page.locator('#keyboard-page').isVisible(), false);
    await page.locator('#dev-tabs [data-dev="keyboard"]').click();
    await page.locator('#kb-macros').evaluate(n => n.open = false);
    await page.locator('#keyboard-page').evaluate(n => n.scrollTop = 0);
    if (process.env.G915_SCREENSHOT) await page.screenshot({ path: process.env.G915_SCREENSHOT });
    assert.deepEqual(errors, []);
    console.log('G915 UI: remapping, macros, save, reset, bank selection and navigation passed.');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
