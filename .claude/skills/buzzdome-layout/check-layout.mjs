// Layout check of every window in the UI preview (mocked backend).
//
// Opens the Director (each tab, with and without a show, light and dark),
// the quiz editor (each ribbon tab and properties page) and the projector
// (each preview scene) at a few window sizes, and reports what a
// quizmaster would see as broken layout:
//
//   page-scroll  the window itself scrolls, so bars meant to stay put
//                (tabs, status bar, ribbon) scroll away or land mid-window
//   bar          a pinned bar is not at its edge or not fully on screen
//   covered      a visible control is drawn under something else
//   off-screen   a control lies outside the window with no way to scroll to it
//
// Usage (from the repo root, with `cd app && npm run dev` running):
//   node .claude/skills/buzzdome-layout/check-layout.mjs [--url http://localhost:5173] [--only director|editor|host] [--lang cs|en] [--shots <dir>]
// The Director and the editor are checked in Czech and English (texts differ in length),
// the projector in Czech (its texts are the quiz's).
// Exits 1 when it finds a problem.

import { createRequire } from "node:module";
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = await (async () => {
  try {
    return require("playwright");
  } catch {
    return import("playwright");
  }
})();

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : fallback;
};
const base = option("url", "http://localhost:5173");
const only = option("only", null);
const shots = option("shots", null);
const LANGS = option("lang", null) ? [option("lang")] : ["cs", "en"];
if (shots) mkdirSync(shots, { recursive: true });

const repo = join(dirname(fileURLToPath(import.meta.url)), "../../..");
/** The preview's scene names, read from `app/src/dev/preview.tsx` so new scenes are checked too. */
const scenes = (() => {
  const source = readFileSync(join(repo, "app/src/dev/preview.tsx"), "utf8");
  const block = source.slice(source.indexOf("const SCENES"), source.indexOf("\n};", source.indexOf("const SCENES")));
  return [...block.matchAll(/^ {2}(\w+):/gm)].map((m) => m[1]);
})();

/** Window sizes: the smallest laptop the Director should work on, a common laptop, a full HD screen. */
const SIZES = [
  { width: 1024, height: 640 },
  { width: 1366, height: 768 },
  { width: 1920, height: 1080 },
];

/** Bars that keep their place while the rest scrolls, and the edge each belongs to. */
const BARS = [
  { selector: ".director > .workspace > .tabs", edge: "top" },
  { selector: ".director > .workspace > .status-bar", edge: "bottom" },
  { selector: ".director > .command-panel", edge: "left" },
  { selector: ".ribbon-tabs", edge: "top" },
  { selector: ".studio > .studio-status", edge: "bottom" },
];

const problems = [];

/** Runs in the page: every layout problem on screen now, as short strings. */
function inspect(bars) {
  const found = [];
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const root = document.scrollingElement;
  if (root.scrollHeight > vh + 1 || root.scrollWidth > vw + 1) {
    found.push(`page-scroll: the window scrolls (${root.scrollWidth}x${root.scrollHeight} in ${vw}x${vh})`);
  }
  const name = (el) => {
    const cls = typeof el.className === "string" && el.className.trim() ? "." + el.className.trim().split(/\s+/).join(".") : "";
    const text = (el.getAttribute("aria-label") || el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 40);
    return `${el.tagName.toLowerCase()}${cls}${text ? ` "${text}"` : ""}`;
  };
  for (const { selector, edge } of bars) {
    for (const el of document.querySelectorAll(selector)) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      const off = r.top < -1 || r.left < -1 || r.bottom > vh + 1 || r.right > vw + 1;
      const atEdge = { top: r.top <= 120, bottom: Math.abs(r.bottom - vh) <= 1, left: Math.abs(r.left) <= 1 }[edge];
      if (off || !atEdge) found.push(`bar: ${name(el)} is at ${Math.round(r.left)},${Math.round(r.top)}-${Math.round(r.right)},${Math.round(r.bottom)}, not on the ${edge} edge of ${vw}x${vh}`);
    }
  }
  // What a control's ancestors let show of it: their clipping boxes, intersected.
  const shown = (el) => {
    let r = el.getBoundingClientRect();
    let box = { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    for (let a = el.parentElement; a && a !== document.documentElement; a = a.parentElement) {
      const style = getComputedStyle(a);
      if (style.overflowX === "visible" && style.overflowY === "visible") continue;
      r = a.getBoundingClientRect();
      box = { left: Math.max(box.left, r.left), top: Math.max(box.top, r.top), right: Math.min(box.right, r.right), bottom: Math.min(box.bottom, r.bottom) };
    }
    return box;
  };
  const scrollable = (el) => {
    for (let a = el.parentElement; a; a = a.parentElement) {
      const style = getComputedStyle(a);
      if (/(auto|scroll)/.test(style.overflowY + style.overflowX) && (a.scrollHeight > a.clientHeight || a.scrollWidth > a.clientWidth)) return true;
    }
    return false;
  };
  // The content of a closed <details> is not drawn, though it still has a box.
  const hiddenInDetails = (el) => {
    const details = el.closest("details:not([open])");
    return details !== null && !el.closest("summary");
  };
  const controls = document.querySelectorAll("button, input, select, textarea, a[href], [role=tab]");
  for (const el of controls) {
    const style = getComputedStyle(el);
    if (style.visibility === "hidden" || style.display === "none" || el.closest("[hidden], [aria-hidden=true], dialog:not([open])") || hiddenInDetails(el)) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) continue;
    const box = shown(el);
    if (box.right - box.left < 2 || box.bottom - box.top < 2) {
      if (!scrollable(el) && (r.bottom > vh || r.right > vw)) found.push(`off-screen: ${name(el)} at ${Math.round(r.left)},${Math.round(r.top)}`);
      continue;
    }
    const x = Math.min(Math.max((box.left + box.right) / 2, 0), vw - 1);
    const y = Math.min(Math.max((box.top + box.bottom) / 2, 0), vh - 1);
    if (x < box.left || y < box.top) continue;
    const hit = document.elementFromPoint(x, y);
    if (!hit || el.contains(hit) || hit.contains(el)) continue;
    // A label or wrapper drawn around the control is the control's own.
    if (hit.closest("label") && hit.closest("label").contains(el)) continue;
    // Toasts and popups float over the window on purpose.
    if (hit.closest(".toasts, .toast, .popup, .menu, .dialog, [role=dialog], [role=menu], .context-menu")) continue;
    found.push(`covered: ${name(el)} lies under ${name(hit)} at ${Math.round(x)},${Math.round(y)}`);
  }
  return [...new Set(found)];
}

const browser = await chromium.launch();
async function check(label, url, size, lang, steps = async () => {}) {
  label = `${label} ${lang}`;
  const page = await browser.newPage({ viewport: size });
  // The UI's language is a preference (`i18n.ts`), read before the first render.
  await page.addInitScript((value) => localStorage.setItem("quiz-show.lang", value), lang);
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`${base}${url}`);
  await page.waitForSelector("#root > *", { timeout: 15000 });
  await page.waitForTimeout(400);
  await steps(page);
  const visit = async (what) => {
    await page.waitForTimeout(250);
    const where = `${label} ${what} @${size.width}x${size.height}`;
    for (const p of await page.evaluate(inspect, BARS)) problems.push(`${where}: ${p}`);
    if (shots) await page.screenshot({ path: join(shots, `${where.replace(/[^\w@.-]+/g, "_")}.png`) });
  };
  return { page, visit, errors, close: () => page.close() };
}

async function eachTab(page, selector, visit) {
  const count = await page.locator(selector).count();
  for (let i = 0; i < count; i++) {
    const tab = page.locator(selector).nth(i);
    const label = ((await tab.textContent()) || (await tab.getAttribute("title")) || `#${i}`).trim();
    await tab.click();
    await visit(`tab "${label}"`);
  }
}

for (const size of SIZES) {
  for (const lang of LANGS) {
    if (!only || only === "director") {
      for (const scene of ["none", "lobby", "running"]) {
        for (const dark of [false, true]) {
          const { page, visit, errors, close } = await check(`director scene=${scene}${dark ? " dark" : ""}`, `/preview.html?window=director&scene=${scene}`, size, lang);
          if (dark) await page.evaluate(() => document.querySelector(".director")?.classList.add("dark"));
          await eachTab(page, ".director > .workspace > .tabs [role=tab]", visit);
          for (const e of errors) problems.push(`director scene=${scene}: page error: ${e}`);
          await close();
        }
      }
    }
    if (!only || only === "editor") {
      const { page, visit, errors, close } = await check("editor", "/preview.html?window=director&scene=none", size, lang, async (p) => {
        await p.locator(".command-panel .command").nth(2).click();
        // The editor first asks what to work on: the preview's quiz, left open.
        await p.locator(".quiz-start-dialog .continue").click();
        await p.waitForSelector(".ribbon-tabs", { timeout: 10000 });
      });
      await eachTab(page, ".ribbon-tabs [role=tab]", visit);
      await eachTab(page, ".props-pages [role=tab]", visit);
      for (const e of errors) problems.push(`editor: page error: ${e}`);
      await close();
    }
    if ((!only || only === "host") && lang === LANGS[0]) {
      for (const scene of scenes) {
        const { visit, errors, close } = await check(`host scene=${scene}`, `/preview.html?window=host&scene=${scene}`, size, lang);
        await visit("");
        for (const e of errors) problems.push(`host scene=${scene}: page error: ${e}`);
        await close();
      }
    }
  }
}
await browser.close();

for (const p of problems) console.log(p);
console.log(problems.length ? `\n${problems.length} layout problem(s).` : "No layout problems found.");
process.exit(problems.length ? 1 : 0);
