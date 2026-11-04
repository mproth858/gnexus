const assert = require('node:assert/strict');
const {chromium} = require('playwright');
(async()=>{
    const browser = await chromium.launch({channel:'msedge',headless:true});
    try {
        const page = await browser.newPage({viewport:{width:1180,height:900}});
        const errors=[]; page.on('pageerror',error=>errors.push(error.message));
        await page.goto('http://127.0.0.1:8765/?lang=en&kb=us');
        await page.evaluate(()=>openSettings());
        await page.locator('#set-tabs [data-tab="lighting"]').click();
        await page.waitForFunction(()=>document.querySelector('#light-status').textContent.includes('disabled'));
        assert.equal(await page.locator('#light-idle').inputValue(),'10');
        await page.locator('label:has(#light-enabled)').click();
        await page.locator('#light-idle').fill('12');
        await page.waitForTimeout(1200); // incoming status must preserve unsaved edits
        assert.equal(await page.locator('#light-idle').inputValue(),'12');
        await page.locator('#light-save').click();
        assert.equal((await page.evaluate(()=>invoke('get_lighting'))).settings.idle_minutes,12);
        for (const mode of ['signalrgb','nexus','auto']) {
            await page.locator(`#light-modes [data-mode="${mode}"]`).click();
            assert.equal((await page.evaluate(()=>invoke('get_lighting'))).mode,mode);
        }
        await page.locator('#light-devices label:has([value="microphone"])').click();
        await page.locator('#light-save').click();
        assert.equal((await page.evaluate(()=>invoke('get_lighting'))).settings.devices.includes('microphone'),false);
        await page.evaluate(()=>closeSheets());
        await page.locator('#dev-tabs [data-dev="keyboard"]').click();
        await page.locator('#kb-keys [data-key="G1"]').click();
        await page.locator('#picker-tabs [data-tab="macros"]').click();
        await page.locator('#picker [data-lighting="signalrgb"]').click();
        assert.equal(await page.evaluate(()=>Keyboard.draft.profile.banks.M1.G1.lighting),'signalrgb');
        assert.match(await page.locator('#kb-keys [data-key="G1"]').innerText(),/SignalRGB/);
        assert.deepEqual(errors,[]);
        console.log('Lighting UI: idle timeout, draft preservation, device selection, source actions and G915 assignment passed.');
    } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exit(1);});
