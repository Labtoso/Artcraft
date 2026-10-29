// Saves the original website exactly as a browser loads it: every page and every file the
// pages ask for (scripts, styles, fonts, images), so the mirror can run the original scripts.
// Usage: node tools/capture.js OUT_DIR
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const ORIGIN = 'https://getartcraft.com';
const SKIP = /^\/(pricing|tutorials|news|faq|support)(\/|$)/i;
const MEDIA = /\.(mp4|webm|mov|m4v|zip|dmg|exe|msi)$/i;
const START = ['/', '/apps/', '/download/', '/press-kit/'];

function fileFor(out, url, type) {
  const u = new URL(url);
  let p = decodeURIComponent(u.pathname);
  if (u.pathname === '/_next/image') return null; // the mirror links the image itself
  if (/text\/html/.test(type) || p.endsWith('/')) p = p.replace(/\/?$/, '/') + 'index.html';
  if (u.search && !/text\/html/.test(type)) return null;
  const f = path.join(out, p);
  return f.startsWith(path.resolve(out)) || f.startsWith(out) ? f : null;
}

(async () => {
  const out = process.argv[2] || 'raw';
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch();
  const saved = new Set();
  const queue = [...START], seen = new Set(START);
  for (const size of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
    const ctx = await browser.newContext({ viewport: size, locale: 'en-US' });
    ctx.on('response', async (res) => {
      try {
        const url = res.url();
        if (!url.startsWith(ORIGIN) || res.request().method() !== 'GET' || res.status() !== 200) return;
        if (MEDIA.test(new URL(url).pathname) || new URL(url).searchParams.has('_rsc')) return;
        const type = res.headers()['content-type'] || '';
        // pages as the server sent them (before scripts ran), only from the page loads
        if (/text\/html/.test(type) && res.request().resourceType() !== 'document') return;
        const f = fileFor(out, url, type);
        if (!f || saved.has(f)) return;
        const body = await res.body();
        fs.mkdirSync(path.dirname(f), { recursive: true });
        fs.writeFileSync(f, body);
        saved.add(f);
      } catch (e) { /* a response that closed early */ }
    });
    const page = await ctx.newPage();
    const pages = size.width > 1000 ? queue : START;
    for (let i = 0; i < pages.length; i++) {
      const p = pages[i];
      try {
        await page.goto(ORIGIN + p, { waitUntil: 'networkidle', timeout: 60000 });
        await page.evaluate(async () => {
          for (let y = 0; y < document.body.scrollHeight; y += 500) { window.scrollTo(0, y); await new Promise(r => setTimeout(r, 100)); }
        });
        // menus and dropdowns load their parts on first use
        for (const b of await page.$$('button[aria-haspopup], button[aria-expanded]')) {
          try { await b.click({ timeout: 1000 }); await page.waitForTimeout(200); await page.keyboard.press('Escape'); } catch (e) {}
        }
        await page.waitForLoadState('networkidle', { timeout: 15000 }).catch(() => {});
        if (pages === queue) {
          const links = await page.$$eval('a[href]', as => as.map(a => a.href));
          for (const l of links) {
            if (!l.startsWith('https://getartcraft.com/')) continue;
            const u = new URL(l); let q = u.pathname.replace(/\/?$/, '/');
            if (/\.\w{2,5}\/$/.test(q) || SKIP.test(q) || seen.has(q)) continue;
            seen.add(q); queue.push(q);
          }
        }
        console.log(`${size.width} ${p}`);
      } catch (e) {
        console.log(`${size.width} ${p} failed: ${e.message.split('\n')[0]}`);
      }
    }
    await ctx.close();
  }
  await browser.close();
  console.log(`${saved.size} files`);
})();
