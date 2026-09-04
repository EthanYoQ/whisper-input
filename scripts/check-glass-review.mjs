// Run against Vite with synthetic data only. PLAYWRIGHT_MODULE may point to a
// preinstalled Playwright package; no browser or user profile is shared.
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const context = await browser.newContext({ viewport: { width: 1180, height: 800 } });
const page = await context.newPage();
const origin = process.env.GLASS_TEST_URL || 'http://localhost:1433';
const failures = [];
const evidence = new URL('../.runtime/.cache/glass-review-20260905/browser/', import.meta.url);
await mkdir(evidence, { recursive: true });
await writeFile(new URL('.vibe-owner.json', evidence), JSON.stringify({ owner: 'codex/glass-review-fixes', sourceProject: 'Whisper-input/redesign-glass', createdAt: new Date().toISOString(), ttlDays: 14, reason: 'Synthetic regression screenshots; no user history or credentials', cleanupCommand: `Remove-Item -LiteralPath '${fileURLToPath(evidence)}' -Recurse -Force` }, null, 2));
async function check(name, fn) {
  try { await fn(); console.log(`PASS ${name}`); }
  catch (error) { failures.push(name); console.error(`FAIL ${name}: ${error.message}`); }
}
try {
  await check('unresolved native material cannot block the UI', async () => {
    const stuck = await context.newPage();
    await stuck.route('**/src/lib/nativeMaterial.ts', route => route.fulfill({ contentType: 'text/javascript', body: 'export const initializeNativeMaterial = () => new Promise(() => {});' }));
    await stuck.goto(origin);
    await stuck.waitForSelector('.wi-side-nav', { timeout: 2000 });
    await stuck.close();
  });
  await page.goto(origin);
  await page.waitForSelector('.wi-side-nav');
  await check('all eight resize zones receive pointer hits above titlebar', async () => {
    const hits = await page.locator('[data-window-resize]').evaluateAll(els => els.map(el => {
      const r = el.getBoundingClientRect();
      return { direction: el.getAttribute('data-window-resize'), hit: document.elementFromPoint(r.x+r.width/2, r.y+r.height/2)?.getAttribute('data-window-resize') };
    }));
    assert.equal(hits.length, 8);
    assert(hits.every(h => h.direction===h.hit), JSON.stringify(hits));
  });
  await check('actual empty chart has no axes or points, populated chart does', async () => {
    await page.evaluate(async () => {
      const { default: React } = await import('/node_modules/.vite/deps/react.js');
      const { default: ReactDOM } = await import('/node_modules/.vite/deps/react-dom_client.js');
      const { WeekChart } = await import('/src/pages/Overview.tsx');
      const el = document.createElement('div'); el.id = 'chart-fixture'; el.className = 'wi-overview-page'; document.body.append(el);
      window.chartFixture = ReactDOM.createRoot(el);
      window.renderChartFixture = sessions => window.chartFixture.render(React.createElement(WeekChart, { days: Array.from({ length: 7 }, (_, i) => ({ key: String(i), label: `9/${i+1}`, shortLabel: String(i), sessions, chars: sessions * 20, durationMs: 100, isToday: i === 6 })) }));
      window.renderChartFixture(0);
    });
    await page.waitForSelector('#chart-fixture .wi-week-empty');
    assert.equal(await page.locator('#chart-fixture svg, #chart-fixture .wi-week-point, #chart-fixture .wi-week-y-axis').count(), 0);
    await page.evaluate(() => window.renderChartFixture(2));
    await page.waitForSelector('#chart-fixture svg');
    assert.equal(await page.locator('#chart-fixture .wi-week-point').count(), 7);
    assert.equal(await page.locator('#chart-fixture .wi-week-empty').count(), 0);
    await page.evaluate(() => { window.chartFixture.unmount(); document.querySelector('#chart-fixture').remove(); });
  });
  await check('empty chart paints no white overlay', async () => {
    // Exercise the actual stylesheet even before a component fixture is mounted.
    const background = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'wi-week-empty';
      document.body.append(el);
      const result = getComputedStyle(el).backgroundColor;
      el.remove();
      return result;
    });
    assert.equal(background, 'rgba(0, 0, 0, 0)');
  });
  await check('floating text surfaces retain a 90% floor', async () => {
    for (const theme of ['light', 'dark']) for (const scale of ['0.6', '1']) {
    const alpha = await page.evaluate(({theme, scale}) => {
      document.documentElement.dataset.theme = theme;
      document.documentElement.style.setProperty('--lg-alpha-scale', scale);
      const el = document.createElement('div');
      el.style.backgroundColor = 'var(--lg-float-bottom)';
      document.body.append(el);
      const color = getComputedStyle(el).backgroundColor;
      el.remove();
      return Number(color.match(/rgba\([^,]+,[^,]+,[^,]+,\s*([\d.]+)\)/)?.[1] ?? 1);
    }, {theme, scale});
    assert(alpha >= 0.9, `${theme}/${scale}: actual alpha ${alpha}`);
    }
  });
  await check('theme updates reach an already loaded document', async () => {
    const windows = [];
    for (const kind of ['capsule', 'qa', 'selection-polish']) {
      const other = await context.newPage();
      await other.goto(origin + '/?window=' + kind);
      await other.waitForFunction(() => document.querySelector('#root')?.childElementCount > 0);
      windows.push(other);
    }
    for (const theme of ['dark', 'light']) {
    await page.evaluate(async theme => {
      const { setThemePreference } = await import('/src/lib/themePreference.ts');
      setThemePreference(theme);
    }, theme);
    for (const other of windows) await other.waitForFunction(theme => document.documentElement.dataset.theme === theme, theme, { timeout: 1500 });
    }
    for (const other of windows) await other.close();
  });
  await check('settings tabs do not overlap command tools at 1180px', async () => {
    await page.locator('.wi-side-nav button').last().click();
    await page.waitForSelector('.wi-settings-tabs');
    const [tabs, tools] = await Promise.all([
      page.locator('.wi-settings-tabs').boundingBox(),
      page.locator('.wi-commandbar').boundingBox(),
    ]);
    assert(tabs && tools);
    const overlap = tabs.x < tools.x + tools.width && tabs.x + tabs.width > tools.x
      && tabs.y < tools.y + tools.height && tabs.y + tabs.height > tools.y;
    assert(!overlap, JSON.stringify({ tabs, tools }));
  });
  await check('settings header wraps across five locales and breakpoint widths', async () => {
    for (const locale of ['zh-CN', 'zh-TW', 'en', 'ja', 'ko']) {
      await page.evaluate(async locale => { const { default: i18n } = await import('/src/i18n/index.ts'); await i18n.changeLanguage(locale); }, locale);
      for (const width of [980, 1024, 1100, 1160, 1180, 1240]) {
        await page.setViewportSize({ width, height: 800 });
        const tabs = await page.locator('.wi-settings-tabs').boundingBox();
        const tools = await page.locator('.wi-commandbar').boundingBox();
        assert(tabs.x + tabs.width <= width && tools.x + tools.width <= width, `${locale}/${width} clipped`);
        assert(!(tabs.x < tools.x + tools.width && tabs.x + tabs.width > tools.x && tabs.y < tools.y + tools.height && tabs.y + tabs.height > tools.y), `${locale}/${width} overlaps`);
      }
    }
  });
  await check('explicit native fallback only; unknown status preserves transparency', async () => {
    for (const theme of ['light', 'dark']) {
      await page.evaluate(async theme => { const { setThemePreference } = await import('/src/lib/themePreference.ts'); setThemePreference(theme); }, theme);
      await page.evaluate(() => { delete document.documentElement.dataset.nativeMaterial; });
      assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).backgroundColor), 'rgba(0, 0, 0, 0)');
      await page.evaluate(async () => { const { applyNativeMaterial } = await import('/src/lib/nativeMaterial.ts'); applyNativeMaterial('fallback'); });
      assert.match(await page.evaluate(() => getComputedStyle(document.documentElement).backgroundColor), /^rgb\(/);
      await page.evaluate(() => { delete document.documentElement.dataset.nativeMaterial; });
      await page.screenshot({ path: fileURLToPath(new URL(`settings-${theme}.png`, evidence)) });
    }
  });
} finally { await browser.close(); }
if (failures.length) process.exitCode = 1;
