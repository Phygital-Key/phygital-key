const { chromium } = await import("/Users/allan/Phygital-Key-Work/mcp/tests/e2e/node_modules/playwright/index.mjs");
const b = await chromium.launch();
for (const [id, theme] of JSON.parse(process.env.SET)) {
  const p = await b.newPage({ viewport: { width: 1280, height: 480 }, deviceScaleFactor: 2 });
  await p.goto("file://" + process.cwd() + "/banners.html");
  await p.evaluate(([id, theme]) => {
    document.getElementById(id).classList.add("on");
    if (theme === "light") document.documentElement.classList.add("light");
  }, [id, theme]);
  await p.evaluate(() => document.fonts.ready);
  await p.waitForTimeout(300);
  const clipped = await p.evaluate((id) => [...document.querySelectorAll(`#${id} pre, #${id} .card`)].some((e) => e.scrollWidth > e.clientWidth + 1 || e.scrollHeight > e.clientHeight + 1), id);
  console.log(id, theme, clipped ? "CLIPPED" : "fits");
  await p.screenshot({ path: `banner-${id}-${theme}.png` });
}
await b.close();
