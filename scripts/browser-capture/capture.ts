// Record what a real browser requests on the two demo sites, one HAR per
// action, as the ground truth the WordPress and WooCommerce helpers copy.
//
// Each action runs in a fresh browser context (an empty cache, as a Goose user
// has), after a setup that brings the context to where the action starts (logged
// in, a product in the cart, a page already open) and is not recorded. Recording
// starts with the action's first request and stops once the network has been
// quiet for QUIET_MS. Every request is recorded from Chromium's own network
// events, so each entry also says which script sent it.
//
// Documents and background requests keep their response bodies, so the tests
// can build their fixtures from the same capture; asset bodies are left out.
//
//   examples/wordpress-sites/theme-test-data/setup.sh start
//   examples/wordpress-sites/woocommerce/setup.sh start
//   cd scripts/browser-capture && npm ci && npx playwright install chromium
//   npm run capture                  # every action
//   npm run capture -- cart-page     # only the actions whose names contain this
//
// The HARs land in tests/fixtures/browser/, with README.md regenerated from them.
import { chromium, type Browser, type BrowserContext, type CDPSession, type Page } from "playwright";
import { mkdirSync, readFileSync, writeFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const OUT = join(ROOT, "tests/fixtures/browser");
const WP = process.env.WORDPRESS_BASE ?? `http://localhost:${process.env.WORDPRESS_PORT ?? "18081"}`;
const WC = process.env.WOOCOMMERCE_BASE ?? `http://localhost:${process.env.WOOCOMMERCE_PORT ?? "18082"}`;
const EDITOR = [process.env.EDITOR_USERNAME ?? "editor", process.env.EDITOR_PASSWORD ?? "editor-password"];
const CUSTOMER = [process.env.CUSTOMER_USERNAME ?? "customer", process.env.CUSTOMER_PASSWORD ?? "customer-password"];
const QUIET_MS = Number(process.env.QUIET_MS ?? 2500);
const MAX_MS = Number(process.env.MAX_MS ?? 30000);

type Kind = "document" | "asset" | "xhr";

interface Entry {
  id: string;
  started: number;
  wallTime: number;
  method: string;
  url: string;
  requestHeaders: Record<string, string>;
  postData?: string;
  cdpType: string;
  initiator: string;
  status?: number;
  statusText?: string;
  responseHeaders?: Record<string, string>;
  mimeType?: string;
  redirectedTo?: string;
  body?: string;
  failed?: string;
}

// The kind of request, as the README and the parity tests group them.
function kind(cdpType: string): Kind {
  if (cdpType === "Document") return "document";
  if (cdpType === "XHR" || cdpType === "Fetch" || cdpType === "EventSource") return "xhr";
  return "asset";
}

// Records every request a page makes between start() and stop().
class Recorder {
  private entries = new Map<string, Entry>();
  private order: Entry[] = [];
  private inflight = new Set<string>();
  private recording = false;
  private lastActivity = Date.now();
  private bodies: Promise<void>[] = [];

  constructor(private cdp: CDPSession, private base: string) {
    cdp.on("Network.requestWillBeSent", (e: any) => {
      this.lastActivity = Date.now();
      if (!/^https?:/.test(e.request.url)) return;
      if (!this.recording) {
        this.inflight.add(e.requestId);
        return;
      }
      const previous = this.entries.get(e.requestId);
      if (previous && e.redirectResponse) {
        // A redirect: close the previous hop and start a new entry for the next.
        previous.status = e.redirectResponse.status;
        previous.statusText = e.redirectResponse.statusText;
        previous.responseHeaders = e.redirectResponse.headers;
        previous.mimeType = e.redirectResponse.mimeType;
        previous.redirectedTo = e.request.url;
        previous.id = `${e.requestId}:${this.order.length}`;
      }
      const entry: Entry = {
        id: e.requestId,
        started: e.timestamp,
        wallTime: e.wallTime,
        method: e.request.method,
        url: e.request.url,
        requestHeaders: e.request.headers,
        postData: e.request.postData ?? postDataFromEntries(e.request.postDataEntries),
        cdpType: e.type ?? "Other",
        initiator: initiatorOf(e.initiator),
      };
      this.entries.set(e.requestId, entry);
      this.order.push(entry);
      this.inflight.add(e.requestId);
    });
    cdp.on("Network.responseReceived", (e: any) => {
      this.lastActivity = Date.now();
      const entry = this.entries.get(e.requestId);
      if (!entry) return;
      this.lastActivity = Date.now();
      entry.status = e.response.status;
      entry.statusText = e.response.statusText;
      entry.responseHeaders = e.response.headers;
      entry.mimeType = e.response.mimeType;
      if (e.type) entry.cdpType = e.type;
    });
    cdp.on("Network.loadingFinished", (e: any) => {
      this.lastActivity = Date.now();
      const entry = this.entries.get(e.requestId);
      this.inflight.delete(e.requestId);
      if (!entry) return;
      this.lastActivity = Date.now();
      if (kind(entry.cdpType) !== "asset") {
        this.bodies.push(
          cdp
            .send("Network.getResponseBody", { requestId: e.requestId })
            .then((r: any) => {
              entry.body = r.base64Encoded ? Buffer.from(r.body, "base64").toString("utf8") : r.body;
            })
            .catch(() => undefined),
        );
      }
    });
    cdp.on("Network.loadingFailed", (e: any) => {
      this.lastActivity = Date.now();
      const entry = this.entries.get(e.requestId);
      this.inflight.delete(e.requestId);
      if (!entry) return;
      this.lastActivity = Date.now();
      entry.failed = e.errorText;
    });
  }

  // Wait until the setup's requests have settled, then start recording.
  async start(page: Page) {
    await this.quiet(page);
    this.inflight.clear();
    this.recording = true;
    this.lastActivity = Date.now();
  }

  private async quiet(page: Page) {
    const begun = Date.now();
    await page.waitForLoadState("load").catch(() => undefined);
    while (Date.now() - begun < MAX_MS) {
      if (this.inflight.size === 0 && Date.now() - this.lastActivity >= QUIET_MS) return;
      await page.waitForTimeout(100);
    }
  }

  // Wait for QUIET_MS without a request in flight, then stop recording.
  async stop(page: Page) {
    await this.quiet(page);
    this.recording = false;
    await Promise.all(this.bodies);
  }

  // The recording as HAR 1.2, with `_kind` and `_initiator` on each entry.
  har(action: Action): object {
    const entries = this.order.map((e) => ({
      startedDateTime: new Date(e.wallTime * 1000).toISOString(),
      time: 0,
      request: {
        method: e.method,
        url: e.url,
        httpVersion: "HTTP/1.1",
        cookies: [],
        headers: headerList(e.requestHeaders),
        queryString: queryList(e.url),
        ...(e.postData !== undefined
          ? { postData: { mimeType: e.requestHeaders["Content-Type"] ?? e.requestHeaders["content-type"] ?? "", text: e.postData } }
          : {}),
        headersSize: -1,
        bodySize: e.postData?.length ?? 0,
      },
      response: {
        status: e.status ?? 0,
        statusText: e.statusText ?? (e.failed ? `failed: ${e.failed}` : ""),
        httpVersion: "HTTP/1.1",
        cookies: [],
        headers: headerList(e.responseHeaders ?? {}),
        content: {
          size: e.body?.length ?? -1,
          mimeType: e.mimeType ?? "",
          ...(e.body !== undefined ? { text: e.body } : {}),
        },
        redirectURL: e.redirectedTo ?? "",
        headersSize: -1,
        bodySize: -1,
      },
      cache: {},
      timings: { send: 0, wait: 0, receive: 0 },
      _resourceType: e.cdpType.toLowerCase(),
      _kind: kind(e.cdpType),
      _initiator: e.initiator,
    }));
    return {
      log: {
        version: "1.2",
        creator: { name: "goose-eggs scripts/browser-capture", version: "1" },
        browser: { name: "chromium", version: browserVersion },
        pages: [],
        comment: `${action.name}: ${action.description}. Captured from ${this.base}.`,
        entries,
      },
    };
  }
}

function postDataFromEntries(entries?: { bytes?: string }[]): string | undefined {
  if (!entries || entries.length === 0) return undefined;
  return entries.map((p) => (p.bytes ? Buffer.from(p.bytes, "base64").toString("utf8") : "")).join("");
}

// The script that sent a request: the innermost script on the stack, or the
// document whose parser found it.
function initiatorOf(initiator: any): string {
  const frames = (stack: any): any[] => (stack ? [...(stack.callFrames ?? []), ...frames(stack.parent)] : []);
  const frame = frames(initiator?.stack).find((f) => f.url);
  if (frame) return frame.url;
  if (initiator?.url) return initiator.url;
  return initiator?.type ?? "other";
}

function headerList(headers: Record<string, string>) {
  return Object.entries(headers).map(([name, value]) => ({ name, value: String(value) }));
}

function queryList(url: string) {
  return [...new URL(url).searchParams.entries()].map(([name, value]) => ({ name, value }));
}

interface Action {
  name: string;
  site: "wordpress" | "woocommerce";
  description: string;
  // Brings the context to where the action starts; not recorded.
  setup?: (page: Page) => Promise<void>;
  // The action itself, recorded.
  act: (page: Page) => Promise<void>;
}

async function wpLogIn(page: Page, base: string, [username, password]: string[]) {
  await page.goto(`${base}/wp-login.php`);
  await page.fill("#user_login", username);
  await page.fill("#user_pass", password);
  await Promise.all([page.waitForNavigation(), page.click("#wp-submit")]);
}

async function myAccountLogIn(page: Page) {
  await page.goto(`${WC}/my-account/`);
  await page.fill("#username", CUSTOMER[0]);
  await page.fill("#password", CUSTOMER[1]);
  await Promise.all([page.waitForNavigation(), page.click('button[name="login"]')]);
}

// Put a product in the cart the way a shopper does, from its page.
async function addFromProductPage(page: Page, path: string, quantity = 1) {
  await page.goto(`${WC}${path}`);
  await page.fill('form.cart input[name="quantity"]', String(quantity));
  await Promise.all([page.waitForNavigation(), page.click('form.cart button[name="add-to-cart"]')]);
}

async function fillCheckout(page: Page) {
  const fields: [string, string][] = [
    ["#email", "goose@example.com"],
    ["#billing-first_name, #shipping-first_name", "Goose"],
    ["#billing-last_name, #shipping-last_name", "Shopper"],
    ["#billing-address_1, #shipping-address_1", "1 Main Street"],
    ["#billing-city, #shipping-city", "San Francisco"],
    ["#billing-postcode, #shipping-postcode", "94105"],
    ["#billing-phone, #shipping-phone", "5555550100"],
  ];
  for (const [selector, value] of fields) {
    const field = page.locator(selector).first();
    if ((await field.count()) > 0 && (await field.inputValue()) === "") {
      await field.fill(value);
      await field.blur();
    }
  }
}

const ACTIONS: Action[] = [
  // The theme test data site.
  {
    name: "home",
    site: "wordpress",
    description: "load the home page",
    act: async (page) => void (await page.goto(`${WP}/`)),
  },
  {
    name: "post",
    site: "wordpress",
    description: "load a post",
    act: async (page) => void (await page.goto(`${WP}/hello-world/`)),
  },
  {
    name: "page",
    site: "wordpress",
    description: "load a page",
    act: async (page) => void (await page.goto(`${WP}/about/`)),
  },
  {
    name: "category",
    site: "wordpress",
    description: "load a category archive",
    act: async (page) => void (await page.goto(`${WP}/category/markup/`)),
  },
  {
    name: "search",
    site: "wordpress",
    description: "search from the home page's search form",
    setup: async (page) => void (await page.goto(`${WP}/`)),
    act: async (page) => {
      await page.goto(`${WP}/?s=post`);
    },
  },
  {
    name: "comment",
    site: "wordpress",
    description: "post a comment as the logged in editor, from the post's comment form",
    setup: async (page) => {
      await wpLogIn(page, WP, EDITOR);
    },
    act: async (page) => {
      await page.goto(`${WP}/hello-world/`);
      await page.fill("#comment", `Recorded by the browser capture ${Date.now()}`);
      await Promise.all([page.waitForNavigation(), page.click("#commentform #submit")]);
    },
  },
  {
    name: "log-in",
    site: "wordpress",
    description: "log in as the editor through wp-login.php",
    act: async (page) => wpLogIn(page, WP, EDITOR),
  },
  {
    name: "log-out",
    site: "wordpress",
    description: "log out with the log out link on the dashboard",
    setup: async (page) => wpLogIn(page, WP, EDITOR),
    act: async (page) => {
      await page.goto(`${WP}/wp-admin/`);
      const href = await page.locator('a[href*="action=logout"]').first().getAttribute("href");
      await page.goto(href!);
    },
  },
  {
    name: "create-post",
    site: "wordpress",
    description: "open the block editor on a new post, give it a title, and save the draft",
    setup: async (page) => {
      await wpLogIn(page, WP, EDITOR);
      // Open the editor once so the welcome guide is dismissed for the user.
      await page.goto(`${WP}/wp-admin/post-new.php`);
      await closeWelcomeGuide(page);
    },
    act: async (page) => {
      await page.goto(`${WP}/wp-admin/post-new.php`);
      await closeWelcomeGuide(page);
      await editorTitle(page).fill(`Recorded by the browser capture ${Date.now()}`);
      await page.getByRole("button", { name: "Save draft" }).click();
      await page.getByRole("button", { name: "Saved" }).waitFor({ timeout: 15000 }).catch(() => undefined);
    },
  },
  {
    name: "update-post",
    site: "wordpress",
    description: "open an existing post in the block editor, change its title, and save it",
    setup: async (page) => {
      await wpLogIn(page, WP, EDITOR);
      await page.goto(`${WP}/wp-admin/post-new.php`);
      await closeWelcomeGuide(page);
    },
    act: async (page) => {
      await page.goto(`${WP}/wp-admin/post.php?post=1&action=edit`);
      await closeWelcomeGuide(page);
      await editorTitle(page).fill(`Hello world, edited ${Date.now()}`);
      await page.getByRole("button", { name: /^(Save|Update)$/ }).first().click();
      await page.getByRole("button", { name: /^(Saved|Update)$/ }).waitFor({ timeout: 15000 }).catch(() => undefined);
    },
  },

  // The WooCommerce shop.
  {
    name: "shop",
    site: "woocommerce",
    description: "load the shop page",
    act: async (page) => void (await page.goto(`${WC}/shop/`)),
  },
  {
    name: "product-category",
    site: "woocommerce",
    description: "load a product category archive",
    act: async (page) => void (await page.goto(`${WC}/product-category/accessories/`)),
  },
  {
    name: "product",
    site: "woocommerce",
    description: "load a product page",
    act: async (page) => void (await page.goto(`${WC}/product/beanie/`)),
  },
  {
    name: "add-to-cart-product-page",
    site: "woocommerce",
    description: "load a product page and add two to the cart with its add to cart form",
    act: async (page) => {
      await page.goto(`${WC}/product/beanie/`);
      await page.waitForTimeout(QUIET_MS);
      await page.fill('form.cart input[name="quantity"]', "2");
      await Promise.all([page.waitForNavigation(), page.click('form.cart button[name="add-to-cart"]')]);
    },
  },
  {
    name: "add-to-cart-shop",
    site: "woocommerce",
    description: "load the shop page and add a simple product with its Add to cart button",
    act: async (page) => {
      await page.goto(`${WC}/shop/`);
      await page.locator('button.add_to_cart_button[data-product_id="12"]').waitFor();
      await page.waitForTimeout(QUIET_MS);
      await page.locator('button.add_to_cart_button[data-product_id="12"]').click();
    },
  },
  {
    name: "cart-page",
    site: "woocommerce",
    description: "load the cart page with a product in the cart",
    setup: async (page) => addFromProductPage(page, "/product/beanie/"),
    act: async (page) => void (await page.goto(`${WC}/cart/`)),
  },
  {
    name: "update-cart-item",
    site: "woocommerce",
    description: "on the loaded cart page, raise a line's quantity by one with its + control",
    setup: async (page) => {
      await addFromProductPage(page, "/product/beanie/");
      await page.goto(`${WC}/cart/`);
      await page.locator(".wc-block-components-quantity-selector__button--plus").first().waitFor();
    },
    act: async (page) => {
      await page.locator(".wc-block-components-quantity-selector__button--plus").first().click();
    },
  },
  {
    name: "remove-cart-item",
    site: "woocommerce",
    description: "on the loaded cart page, remove a line with its remove control",
    setup: async (page) => {
      await addFromProductPage(page, "/product/beanie/");
      await addFromProductPage(page, "/product/cap/");
      await page.goto(`${WC}/cart/`);
      await page.locator(".wc-block-cart-item__remove-link").first().waitFor();
    },
    act: async (page) => {
      await page.locator(".wc-block-cart-item__remove-link").first().click();
    },
  },
  {
    name: "checkout-page",
    site: "woocommerce",
    description: "load the checkout page with a product in the cart",
    setup: async (page) => addFromProductPage(page, "/product/beanie/"),
    act: async (page) => void (await page.goto(`${WC}/checkout/`)),
  },
  {
    name: "place-order",
    site: "woocommerce",
    description: "on the loaded checkout page, fill in the address, pay with Cash on delivery and place the order",
    setup: async (page) => {
      await addFromProductPage(page, "/product/beanie/");
      await page.goto(`${WC}/checkout/`);
      await page.locator(".wc-block-components-checkout-place-order-button").waitFor();
    },
    act: async (page) => {
      await fillCheckout(page);
      // A shopper pauses before placing the order; the address changes are sent first.
      await page.waitForTimeout(QUIET_MS);
      const cod = page.locator('input[value="cod"]');
      if ((await cod.count()) > 0 && !(await cod.isChecked())) await cod.check();
      await Promise.all([
        page.waitForURL(/order-received/, { timeout: 30000 }),
        page.locator(".wc-block-components-checkout-place-order-button").click(),
      ]);
    },
  },
  {
    name: "my-account-log-in",
    site: "woocommerce",
    description: "load My Account and log in with its log in form",
    act: async (page) => {
      await page.goto(`${WC}/my-account/`);
      await page.waitForTimeout(QUIET_MS);
      await page.fill("#username", CUSTOMER[0]);
      await page.fill("#password", CUSTOMER[1]);
      await Promise.all([page.waitForNavigation(), page.click('button[name="login"]')]);
    },
  },
  {
    name: "my-account-orders",
    site: "woocommerce",
    description: "load My Account's orders list, logged in",
    setup: myAccountLogIn,
    act: async (page) => void (await page.goto(`${WC}/my-account/orders/`)),
  },
];

function editorTitle(page: Page) {
  // The title is in the editor canvas, an iframe when every block is API v3.
  return page
    .frameLocator('iframe[name="editor-canvas"]')
    .locator(".editor-post-title__input, [aria-label='Add title']")
    .first();
}

async function closeWelcomeGuide(page: Page) {
  const close = page.getByRole("button", { name: "Close" }).first();
  await page.waitForSelector(".edit-post-layout, .editor-editor-interface", { timeout: 30000 });
  if (await close.isVisible().catch(() => false)) await close.click().catch(() => undefined);
}

let browserVersion = "";

async function runAction(browser: Browser, action: Action) {
  const base = action.site === "wordpress" ? WP : WC;
  const context: BrowserContext = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Network.enable", { maxResourceBufferSize: 50_000_000, maxTotalBufferSize: 200_000_000 });
  const recorder = new Recorder(cdp, base);
  if (action.setup) await action.setup(page);
  await recorder.start(page);
  await action.act(page);
  await recorder.stop(page);
  const dir = join(OUT, action.site);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${action.name}.har`), JSON.stringify(recorder.har(action), null, 1) + "\n");
  await context.close();
}

// The README: per action, every request in order.
function readme() {
  const lines: string[] = [
    "# What a browser requests",
    "",
    "Recorded from headless Chromium by `scripts/browser-capture/capture.ts` on the two demo",
    "sites under `examples/wordpress-sites/`, one HAR per action. These are the specification",
    "the `wordpress` and `woocommerce` helpers follow, and the parity tests",
    "(`tests/sites/parity.rs`) compare each helper's requests with them.",
    "",
    "Each action starts where the shopper or editor already is (logged in, a product in the",
    "cart, the page open), which the capture sets up without recording. Paths are on the",
    "site; `{n}` marks a query value or path part that varies between runs. Type is",
    "`document` (a page the browser navigates to, including form posts and redirects),",
    "`asset` (a script, stylesheet, image or font) or `xhr` (a background request a script",
    "sends, with the script that sent it).",
    "",
  ];
  for (const site of ["wordpress", "woocommerce"]) {
    const dir = join(OUT, site);
    let files: string[] = [];
    try {
      files = readdirSync(dir).filter((f) => f.endsWith(".har"));
    } catch {
      continue;
    }
    const ordered = ACTIONS.filter((a) => a.site === site && files.includes(`${a.name}.har`));
    lines.push(`## ${site === "wordpress" ? "WordPress (theme test data site)" : "WooCommerce (demo shop)"}`, "");
    for (const action of ordered) {
      const har = JSON.parse(readFileSync(join(dir, `${action.name}.har`), "utf8"));
      const entries = har.log.entries as any[];
      const assets = entries.filter((e) => e._kind === "asset");
      lines.push(`### ${action.name}`, "", `${capitalize(action.description)}. File: \`${site}/${action.name}.har\`.`, "");
      lines.push("| # | Method | Path | Type | Sent by |", "|---|---|---|---|---|");
      let n = 0;
      for (const e of entries) {
        n++;
        if (e._kind === "asset") continue;
        const by = e._kind === "xhr" ? scriptName(e._initiator) : e.request.method === "GET" && n === 1 ? "navigation" : "navigation";
        lines.push(`| ${n} | ${e.request.method} | \`${pathOf(e.request.url)}\`${e.response.status >= 300 && e.response.status < 400 ? ` (${e.response.status})` : ""} | ${e._kind} | ${by} |`);
      }
      const local = assets.filter((e) => sameOrigin(e.request.url, har));
      lines.push(
        "",
        `Assets: ${assets.length} (${local.length} on the site), not listed row by row: ${summarizeAssets(assets)}.`,
        "",
      );
    }
  }
  writeFileSync(join(OUT, "README.md"), lines.join("\n"));
}

function capitalize(s: string) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function sameOrigin(url: string, har: any) {
  const comment: string = har.log.comment;
  const base = comment.match(/Captured from (\S+)\.$/)?.[1] ?? "";
  return url.startsWith(base);
}

function pathOf(url: string) {
  const u = new URL(url);
  return (u.pathname + u.search).replace(/\|/g, "\\|");
}

function scriptName(url: string) {
  try {
    const u = new URL(url);
    return `\`${u.pathname.split("/").slice(-3).join("/")}\``;
  } catch {
    return url;
  }
}

function summarizeAssets(assets: any[]) {
  const by = new Map<string, number>();
  for (const a of assets) by.set(a._resourceType, (by.get(a._resourceType) ?? 0) + 1);
  return [...by.entries()].map(([t, n]) => `${n} ${t}`).join(", ") || "none";
}

async function main() {
  const filter = process.argv[2];
  const browser = await chromium.launch();
  browserVersion = browser.version();
  for (const action of ACTIONS) {
    if (filter && !action.name.includes(filter)) continue;
    process.stdout.write(`${action.site}/${action.name} ... `);
    try {
      await runAction(browser, action);
      console.log("ok");
    } catch (e) {
      console.log(`FAILED: ${(e as Error).message}`);
      process.exitCode = 1;
    }
  }
  await browser.close();
  readme();
}

await main();
