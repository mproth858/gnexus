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
    await page.locator('#kb-regular summary').click();
    await page.locator('#kb-regular-enabled').check();
    await page.locator('#kb-regular-layout [data-regular="F6"]').click();
    await page.locator('#picker-tabs [data-tab="basic"]').click();
    await page.locator('#picker [data-code="F5"]').click();
    assert.match(await page.locator('#kb-regular-list').innerText(), /F6.*F5/);
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.status.regular_active);
    assert.equal(await page.evaluate(() => Keyboard.status.settings.regular_keys.F6), 'F5');
    if (process.env.G915_REGULAR_SCREENSHOT) {
      await page.locator('#kb-regular').scrollIntoViewIfNeeded();
      await page.screenshot({ path: process.env.G915_REGULAR_SCREENSHOT });
    }
    await page.locator('#kb-regular-pause').click();
    await page.waitForFunction(() => !Keyboard.status.regular_active);
    assert.equal(await page.locator('#kb-save').isDisabled(), false, 'Apply resumes paused saved assignments');
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.status.regular_active);
    await page.locator('#kb-regular-list [data-restore="F6"]').click();
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving);
    assert.deepEqual(await page.evaluate(() => Keyboard.status.settings.regular_keys), {});
    // Per-key painting: group selection, individual colors, save, reset and mode switching.
    await page.locator('#kb-key-colors summary').click();
    await page.locator('#kb-paint-wasd').click();
    assert.match(await page.locator('#kb-paint-selection').innerText(), /4 selected/);
    await page.locator('#kb-paint-color').fill('#ff0000');
    await page.locator('#kb-paint-apply').click();
    assert.equal(await page.locator('#kb-per-key').isChecked(), true);
    assert.equal(await page.locator('#kb-lighting').isChecked(), true);
    for (const key of ['W', 'A', 'S', 'D']) assert.equal(await page.evaluate(k => Keyboard.draft.key_colors[k], key), '#FF0000');
    await page.locator('#kb-paint-none').click();
    await page.locator('#kb-color-layout [data-color-key="G1"]').click();
    await page.locator('#kb-color-layout [data-color-key="Logo"]').click();
    await page.locator('#kb-paint-color').fill('#00ff00');
    await page.locator('#kb-paint-apply').click();
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving);
    assert.equal(await page.evaluate(() => Keyboard.status.settings.key_colors.Logo), '#00FF00');
    assert.equal(await page.evaluate(() => Keyboard.status.settings.key_colors.G1), '#00FF00');
    if (process.env.G915_COLOR_SCREENSHOT) {
      await page.locator('#kb-key-colors').scrollIntoViewIfNeeded();
      await page.screenshot({ path: process.env.G915_COLOR_SCREENSHOT });
    }
    await page.locator('#kb-paint-reset').click();
    assert.equal(await page.evaluate(() => Keyboard.draft.key_colors.Logo), undefined);
    await page.locator('#kb-reset').click();
    assert.equal(await page.evaluate(() => Keyboard.draft.key_colors.Logo), '#00FF00');
    await page.locator('#kb-lighting').uncheck();
    assert.equal(await page.locator('#kb-per-key').isChecked(), false);
    assert.equal(await page.evaluate(() => Object.keys(Keyboard.draft.key_colors).length), 6, 'disabling lighting preserves stored paint');
    await page.locator('#kb-reset').click();
    await page.locator('#kb-paint-clear').click();
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.key_colors), {});
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving);
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
    // Reproduce the reported failure: native capture returns zero, but the app
    // receives real keyboard events. The local path must save Ctrl+C once.
    await page.goto('http://127.0.0.1:8765/?lang=en&kb=us&norecord=1');
    await page.locator('#dev-tabs [data-dev="keyboard"]').click();
    await page.locator('#kb-macros summary').click();
    await page.locator('#kb-macro-list [data-act="add"]').click();
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.waitForFunction(() => Keyboard.editor.recording);
    await page.keyboard.down('Control');
    await page.keyboard.press('c');
    await page.keyboard.up('Control');
    await page.waitForFunction(() => Keyboard.editor.captureCount === 4);
    await page.waitForTimeout(300); // a zero native count must not erase local progress
    assert.match(await page.locator('#kb-macro-edit .rec-live').innerText(), /4 inputs captured/);
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.waitForFunction(() => !Keyboard.editor.recordBusy);
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.profile.macros['macro-1'].steps.filter(s => !s.startsWith('wait '))), ['down LeftCtrl', 'down C', 'up C', 'up LeftCtrl']);
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.waitForFunction(() => Keyboard.editor.recording);
    await page.keyboard.down('Shift');
    await page.locator('#kb-macro-edit [data-act="record"]').click();
    await page.keyboard.up('Shift');
    await page.waitForFunction(() => !Keyboard.editor.recordBusy);
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.profile.macros['macro-1'].steps.filter(s => !s.startsWith('wait '))), ['down LeftShift', 'up LeftShift']);
    assert.deepEqual(errors, []);
    console.log('G915 UI: recording, capture count, status refresh, remapping, macros, save, reset, banks and navigation passed.');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
