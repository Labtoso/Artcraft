// Screenshots of the original website and the mirror, page by page, to check that they look alike.
// Usage: node tools/compare.js OUT_DIR
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const ORIGIN = 'https://getartcraft.com';
const MIRROR = (process.env.ARTCRAFT_SITE || 'https://labtoso.github.io/Artcraft').replace(/\/$/, '');
const PAGES = ['/', '/apps/', '/download/', '/support/', '/tutorials/', '/news/', '/faq/', '/press-kit/',
  '/apps/wordcraft/', '/apps/photocraft/', '/faq/models-supported/', '/news/welcome/', '/tutorials/2d-editor-basics/'];
const SIZES = [{ name: 'desktop', width: 1440, height: 900 }, { name: 'mobile', width: 390, height: 844 }];

(async () => {
  const out = process.argv[2] || 'compare';
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.launch();
  for (const size of SIZES) {
    for (const [label, base] of [['original', ORIGIN], ['mirror', MIRROR]]) {
      const ctx = await browser.newContext({ viewport: { width: size.width, height: size.height }, locale: 'en-US' });
      const page = await ctx.newPage();
      for (const p of PAGES) {
        const file = path.join(out, `${size.name}${p.replace(/\//g, '_')}${label}.png`);
        try {
          await page.goto(base + p, { waitUntil: 'networkidle', timeout: 60000 });
          // scroll through once so lazy images and scroll effects run
          await page.evaluate(async () => {
            for (let y = 0; y < document.body.scrollHeight; y += 600) { window.scrollTo(0, y); await new Promise(r => setTimeout(r, 120)); }
            window.scrollTo(0, 0);
          });
          await page.waitForTimeout(1500);
          await page.screenshot({ path: file, fullPage: true });
          if (label === 'original' && size.name === 'desktop') {
            // the page as the original's scripts left it, to rebuild what they draw
            fs.writeFileSync(file.replace(/\.png$/, '.html'), await page.content());
          }
          const h = await page.evaluate(() => document.body.scrollHeight);
          console.log(`${size.name} ${label} ${p} height=${h}`);
        } catch (e) {
          console.log(`${size.name} ${label} ${p} failed: ${e.message.split('\n')[0]}`);
        }
      }
      await ctx.close();
    }
  }
  await browser.close();
})();
