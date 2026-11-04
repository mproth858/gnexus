// Requires playwright and the UI dev server on localhost:8765.
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1180, height: 900 }, locale: 'en-US' });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    // No language query: exercise the default, including after a reload.
    await page.goto('http://127.0.0.1:8765/');
    await page.waitForFunction(() => S.dto && Keyboard.draft);
    const checkEnglish = async label => {
      const leaks = await page.evaluate(() => {
        const originals = new Set(Object.entries(TR).filter(([fr, values]) => values[0] && values[0] !== fr).map(([fr]) => fr));
        const french = /[éèàçùêôîûÉÈÀÇÙÊÔÎÛ]|\b(?:BANQUE|SEULEMENT|actuel|aucun|aucune|touche|touches|souris|casque|profil|profils|enregistrer|supprimer|annuler|paramètres)\b/i;
        const found = new Set();
        const inspect = value => {
          const text = value.replace(/\s+/g, ' ').trim();
          if (text && (originals.has(text) || french.test(text))) found.add(text);
        };
        // Include hidden dialogs and inactive panes: a label should be ready in
        // English before the user opens it, not depend on a previous action.
        const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let n = walker.nextNode(); n; n = walker.nextNode()) {
          if (!n.parentElement.closest('script, style')) inspect(n.nodeValue);
        }
        for (const el of document.querySelectorAll('[title], [placeholder], [aria-label]')) {
          for (const name of ['title', 'placeholder', 'aria-label']) if (el.hasAttribute(name)) inspect(el.getAttribute(name));
        }
        return [...found];
      });
      assert.deepEqual(leaks, [], `${label}: untranslated UI text`);
      assert.equal(await page.locator('html').getAttribute('lang'), 'en');
    };
    await checkEnglish('startup');
    await page.reload();
    await page.waitForFunction(() => S.dto && Keyboard.draft);
    await checkEnglish('restart');

    for (const device of ['g13', 'mouse', 'headset', 'keyboard', 'microphone']) {
      await page.locator(`#dev-tabs [data-dev="${device}"]`).click();
      await checkEnglish(device);
    }
    await page.locator('#btn-settings').click();
    for (const tab of ['general', 'g13', 'mouse', 'headset', 'about']) {
      await page.locator(`#set-tabs [data-tab="${tab}"]`).click();
      await checkEnglish(`settings/${tab}`);
    }
    await page.evaluate(() => closeSheets());
    await page.locator('#dev-tabs [data-dev="g13"]').click();
    await page.evaluate(() => openPicker('G1'));
    for (const tab of ['keyboard', 'basic', 'media', 'mouse', 'pad', 'macros']) {
      await page.locator(`#picker-tabs [data-tab="${tab}"]`).click();
      await checkEnglish(`assignments/${tab}`);
    }
    await page.evaluate(() => { closeSheets(); openAdvanced('apps'); });
    await checkEnglish('G13 applications');
    await page.locator('#adv-tabs [data-tab="macros"]').click();
    await checkEnglish('G13 macros');
    await page.evaluate(() => closeSheets());
    await page.locator('#dev-tabs [data-dev="mouse"]').click();
    await page.locator('#ms-advanced').click();
    for (const tab of ['dpi', 'light', 'rate', 'onboard', 'macros']) {
      await page.locator(`#ms-adv-tabs [data-tab="${tab}"]`).click();
      await checkEnglish(`mouse/${tab}`);
    }
    for (const [path, label] of [['/?hsoff#offline', 'disconnected devices'], ['/#overlay', 'game overlay']]) {
      await page.goto('http://127.0.0.1:8765' + path);
      await page.waitForFunction(() => S.dto && Keyboard.draft);
      await checkEnglish(label);
    }
    assert.deepEqual(errors, []);
    console.log('English UI: default/reload, all five devices, settings, assignment tabs, macros and mouse controls passed.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
