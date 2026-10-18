# Banner sources

The hero images and section banners are HTML pages in the brand style (Night Signal colours, Anybody, Hanken Grotesk and JetBrains Mono), rendered to PNG at 2x with Playwright.

| File | Makes |
|---|---|
| `hero-base.html` | The base hero (both products) |
| `hero-mcp.html`, `hero-a2a.html` | The product heroes |
| `hero.html` | The first hero (kept for reference) |
| `banners.html` | Every section banner, one `<section>` each (`how`, `check`, `reach`, `standing`, `mcp-connect`, `mcp-tools`, `a2a-states`, `a2a-card`) |

To render: put `fonts.css` (from `crates/phygital-key-gate/assets`, with the font paths pointing at that folder) and the logo SVGs (`docs/brand/`) next to the pages, set the Playwright import path in the `render*.mjs` scripts, then run for example `SET='[["how","light"]]' node render-banners.mjs`.
