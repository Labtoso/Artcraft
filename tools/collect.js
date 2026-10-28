#!/usr/bin/env node
// Every text a visitor can see on the built website, for the German translation.
// Usage: node tools/collect.js <site folder> <base url> <out.json>
//   e.g. node tools/collect.js _site http://localhost:8000/Artcraft texts.json
// Opens each page on a desktop and a phone screen, scrolls it, opens the menus and asks the
// page's own debranding layer (window.__acTexts) what is shown.
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

const [siteDir, base, out] = process.argv.slice(2);
const root = path.join(siteDir, new URL(base).pathname);

function pages(dir, rel = '/') {
  let list = fs.existsSync(path.join(dir, 'index.html')) ? [rel] : [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory() && !e.name.startsWith('_') && !e.name.startsWith('.')) list = list.concat(pages(path.join(dir, e.name), rel + e.name + '/'));
  }
  return list;
}

async function texts(page) {
  return page.evaluate(() => (window.__acTexts ? window.__acTexts() : []));
}

async function openMenus(page, found) {
  const buttons = await page.$$('header button, nav button, [aria-haspopup], [aria-expanded]');
  for (const b of buttons) {
    try {
      if (!(await b.isVisible())) continue;
      if (/theme|dark|light/i.test((await b.getAttribute('aria-label')) || '')) continue;
      await b.click({ timeout: 2000 });
      await page.waitForTimeout(400);
      (await texts(page)).forEach(t => found.add(t));
      await page.keyboard.press('Escape');
      await page.waitForTimeout(200);
    } catch (e) { /* a button that went away while the menu changed */ }
  }
}

(async () => {
  const list = pages(root);
  const found = new Set();
  const browser = await chromium.launch();
  for (const [width, height] of [[1440, 900], [390, 844]]) {
    const ctx = await browser.newContext({ viewport: { width, height }, locale: 'en-US' });
    await ctx.addInitScript(() => { try { localStorage.setItem('artcraft-lang', 'en'); } catch (e) {} });
    for (const p of list) {
      const page = await ctx.newPage();
      try {
        await page.goto(base + p, { waitUntil: 'networkidle', timeout: 60000 });
        await page.evaluate(async () => {
          for (let y = 0; y < document.body.scrollHeight; y += 600) { window.scrollTo(0, y); await new Promise(r => setTimeout(r, 100)); }
          window.scrollTo(0, 0);
        });
        await page.waitForTimeout(800);
        (await texts(page)).forEach(t => found.add(t));
        await openMenus(page, found);
      } catch (e) {
        console.log(`::warning title=German::${p}: ${e.message.split('\n')[0]}`);
      }
      await page.close();
    }
    await ctx.close();
  }
  await browser.close();
  fs.writeFileSync(out, JSON.stringify([...found].sort(), null, 1));
  console.log(`${list.length} pages, ${found.size} texts`);
})();
