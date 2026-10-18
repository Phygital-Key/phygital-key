const { chromium } = await import("/Users/allan/Phygital-Key-Work/mcp/tests/e2e/node_modules/playwright/index.mjs");
const b = await chromium.launch();
for (const [theme, scheme, page] of [["mcp", "dark", "hero-mcp.html"], ["a2a", "dark", "hero-a2a.html"], ["base", "dark", "hero-base.html"]]) {
  const p = await b.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 2, colorScheme: scheme });
  await p.goto("file://" + process.cwd() + "/" + page);
  if (false) await p.evaluate(() => document.documentElement.classList.add("light"));
  await p.evaluate(() => document.fonts.ready);
  await p.waitForTimeout(300);
  const over = await p.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.scrollHeight]);
  console.log(theme, "page size", over);
  await p.screenshot({ path: `hero-${theme}.png` });
}
await b.close();
