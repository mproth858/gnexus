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
    // Physical geometry matters: G-column, separate navigation, media, and
    // double-height/wide numpad keys must survive both renderers and resizing.
    const checkGeometry = async (root, attribute) => {
      const result = await page.evaluate(({ root, attribute }) => {
        const board = document.querySelector(root);
        const rect = key => board.querySelector(`[${attribute}="${key}"]`).getBoundingClientRect();
        const g1 = board.querySelector('[data-key="G1"], [data-color-key="G1"]').getBoundingClientRect();
        const g5 = board.querySelector('[data-key="G5"], [data-color-key="G5"]').getBoundingClientRect();
        const grave = rect('Grave'), down = rect('Down'), one = rect('Numpad1');
        const media = ['MediaPrevious', 'MediaPlayPause', 'MediaNext', 'VolumeMute'].map(rect);
        const ids = [...board.querySelectorAll(`[${attribute}]`)].map(el => el.getAttribute(attribute));
        return {
          gColumn: g1.x < grave.x && Math.abs(g1.y - grave.y) < 1 && Math.abs(g5.y - down.y) < 1 && Math.abs(g5.x - g1.x) < 1,
          arrows: rect('Up').y < down.y && rect('Left').x < down.x && rect('Right').x > down.x,
          numpad: rect('NumpadAdd').height > one.height * 1.8 && rect('NumpadEnter').height > one.height * 1.8 && rect('Numpad0').width > one.width * 1.8,
          media: media.every((r, i) => r.x > rect('Pause').x && (!i || r.x > media[i - 1].x)) && board.querySelector('[data-hardware="VolumeRoller"]').getBoundingClientRect().y < media[0].y,
          unique: new Set(ids).size === ids.length,
          bounded: [...board.querySelectorAll('.g915-cap')].every(el => { const r = el.getBoundingClientRect(), b = board.querySelector('.g915-board').getBoundingClientRect(); return r.x >= b.x && r.right <= b.right && r.bottom <= b.bottom; }),
        };
      }, { root, attribute });
      assert.deepEqual(result, { gColumn: true, arrows: true, numpad: true, media: true, unique: true, bounded: true });
    };
    await checkGeometry('#kb-regular-layout', 'data-regular');
    await page.locator('#kb-regular-layout [data-key="G2"]').click();
    await page.locator('#picker-tabs [data-tab="keyboard"]').click();
    await page.locator('#picker [data-code="B"]').click();
    assert.equal(await page.evaluate(() => Keyboard.draft.profile.banks.M1.G2), 'B');
    assert.equal(await page.evaluate(() => Keyboard.draft.regular_keys.G2), undefined);
    await page.locator('#kb-regular-layout [data-regular="VolumeMute"]').click();
    await page.locator('#picker-tabs [data-tab="basic"]').click();
    await page.locator('#picker [data-code="F7"]').click();
    assert.equal(await page.evaluate(() => Keyboard.draft.regular_keys.VolumeMute), 'F7');
    await page.locator('#kb-regular-list [data-restore="VolumeMute"]').click();
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
    await checkGeometry('#kb-color-layout', 'data-color-key');
    await page.setViewportSize({ width: 980, height: 760 });
    await checkGeometry('#kb-color-layout', 'data-color-key');
    await page.setViewportSize({ width: 1180, height: 900 });
    // ISO variants keep the extra physical key and tall Enter without overlaps.
    await page.evaluate(() => { applyLayout('qwertz', 'de'); Keyboard.renderLighting(); });
    assert.equal(await page.locator('#kb-color-layout [data-color-key="IntlBackslash"]').count(), 1);
    assert.ok((await page.locator('#kb-color-layout [data-color-key="Enter"]').boundingBox()).height > (await page.locator('#kb-color-layout [data-color-key="A"]').boundingBox()).height * 1.8);
    await page.evaluate(() => { applyLayout('us', 'en'); Keyboard.renderLighting(); });
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
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.bank === 'M2');
    assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(), /Unassigned/);
    // M2 has independent lighting, per-key paint, regular overrides and G maps.
    await page.locator('#kb-lighting').check();
    await page.locator('#kb-color').fill('#00aaff');
    await page.locator('#kb-regular-enabled').check();
    await page.locator('#kb-regular-layout [data-regular="F6"]').click();
    await page.locator('#picker-tabs [data-tab="basic"]').click();
    await page.locator('#picker [data-code="F8"]').click();
    await page.locator('#kb-keys [data-key="G1"]').click();
    await page.locator('#picker-tabs [data-tab="keyboard"]').click();
    await page.locator('#picker [data-code="B"]').click();
    await page.locator('#kb-paint-none').click();
    await page.locator('#kb-color-layout [data-color-key="W"]').click();
    await page.locator('#kb-paint-color').fill('#ffff00');
    await page.locator('#kb-paint-apply').click();
    await page.locator('#kb-save').click();
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.status.bank === 'M2');
    await page.locator('#kb-color-layout [data-bank="M1"]').click();
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.bank === 'M1');
    assert.equal(await page.locator('#kb-color').inputValue(), '#1e56e4');
    assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(), /A/);
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.regular_keys), {});
    assert.deepEqual(await page.evaluate(() => Keyboard.draft.key_colors), {});
    await page.locator('#kb-regular-layout [data-bank="M2"]').click();
    await page.waitForFunction(() => !Keyboard.saving && Keyboard.bank === 'M2');
    assert.equal(await page.locator('#kb-color').inputValue(), '#00aaff');
    assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(), /B/);
    assert.equal(await page.evaluate(() => Keyboard.draft.regular_keys.F6), 'F8');
    assert.equal(await page.evaluate(() => Keyboard.draft.key_colors.W), '#FFFF00');
    assert.equal(await page.locator('#kb-color-layout [data-bank="M2"]').getAttribute('aria-pressed'), 'true');
    for (const key of ['MR', 'Lightspeed', 'Bluetooth', 'GameMode', 'VolumeRoller']) assert.equal(await page.locator(`#kb-color-layout [data-hardware="${key}"]`).count(), 1);
    // An incoming hardware bank event must not discard unsaved edits.
    await page.locator('#kb-color').fill('#ff00ff');
    await page.evaluate(() => Keyboard.update({ ...Keyboard.status, bank: 'M1', settings: { ...Keyboard.status.settings, active_bank: 'M1' } }));
    assert.equal(await page.evaluate(() => Keyboard.bank), 'M2');
    assert.equal(await page.locator('#kb-color').inputValue(), '#ff00ff');
    await page.locator('#kb-reset').click();
    assert.equal(await page.evaluate(() => Keyboard.bank), 'M1');

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
