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
    // Record an already-saved macro while device status updates arrive.
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.waitForFunction(() => Keyboard.editor.recording && Keyboard.editor.captureCount === 4);
    assert.match(await page.locator('#kb-macro-edit .rec-live').innerText(), /4 inputs captured/);
    assert.equal(await page.locator('#kb-save').isDisabled(), true);
    const targetSurvived = await page.evaluate(() => {
      const target = Keyboard.editor.recordTarget;
      Keyboard.update(structuredClone(Keyboard.status));
      return Keyboard.draft.profile.macros['macro-1'] === target;
    });
    assert.equal(targetSurvived, true, 'status refresh must preserve the recording target');
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.waitForFunction(() => !Keyboard.editor.recording && !Keyboard.editor.recordBusy);
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.profile.macros['macro-1'].steps.slice(0, 3)), ['down LeftCtrl', 'wait 82', 'down C']);
    assert.equal(await page.locator('#kb-save').isDisabled(), false);
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving);
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
    console.log('G915 UI: recording, capture count, status refresh, remapping, macros, save, reset, banks and navigation passed.');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
