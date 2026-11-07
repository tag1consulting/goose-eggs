// Cut the fixtures the unit and doc tests replay out of the browser HARs (see
// capture.ts), and capture the few pages no browser action reaches: the shop's
// second page, an out of stock product, checkout with an empty cart, and an
// order received page for a wrong order key.
//
// Every fixture has the site's absolute URLs (plain and JSON escaped) made
// relative, so it does not depend on the port it was captured from. Where a
// test needs a response header, it is saved next to the body as
// <fixture>.headers.
//
//   npm run capture          # first: the HARs
//   npm run fixtures         # then: the fixtures, from the HARs and the shop
import { chromium, request } from "playwright";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const BROWSER = join(ROOT, "tests/fixtures/browser");
const FIXTURES = join(ROOT, "tests/fixtures");
const WC_PORT = process.env.WOOCOMMERCE_PORT ?? "18082";
const WC = process.env.WOOCOMMERCE_BASE ?? `http://localhost:${WC_PORT}`;

function relative(text: string, base: string) {
  const escaped = base.replace(/\//g, "\\/");
  return text.split(`${base}/`).join("/").split(`${escaped}\\/`).join("\\/");
}

interface Entry {
  method: string;
  url: string;
  kind: string;
  body?: string;
  headers: Record<string, string>;
}

function har(site: string, action: string): { base: string; entries: Entry[] } {
  const json = JSON.parse(readFileSync(join(BROWSER, site, `${action}.har`), "utf8"));
  const base = (json.log.comment as string).match(/Captured from (\S+)\.$/)![1];
  const entries = (json.log.entries as any[]).map((e) => ({
    method: e.request.method,
    url: e.request.url,
    kind: e._kind,
    body: e.response.content.text,
    headers: Object.fromEntries((e.response.headers as any[]).map((h) => [h.name, h.value])),
  }));
  return { base, entries };
}

// Save the body of the nth (from 0) non-asset request matching `pick`.
function save(site: string, action: string, out: string, pick: (e: Entry) => boolean, n = 0, headers: string[] = []) {
  const { base, entries } = har(site, action);
  const entry = entries.filter((e) => e.kind !== "asset" && pick(e))[n];
  if (!entry?.body) throw new Error(`${site}/${action}: no body to save as ${out}`);
  writeFileSync(join(FIXTURES, out), relative(entry.body, base));
  if (headers.length > 0) {
    const lines = headers
      .map((h) => [h, Object.entries(entry.headers).find(([k]) => k.toLowerCase() === h.toLowerCase())?.[1]])
      .filter(([, v]) => v !== undefined)
      .map(([h, v]) => `${h}: ${v}`);
    writeFileSync(join(FIXTURES, `${out}.headers`), lines.join("\n") + "\n");
  }
  console.log(`${out} <- ${site}/${action} ${entry.method} ${new URL(entry.url).pathname}`);
}

const doc = (e: Entry) => e.kind === "document";
const path = (p: string) => (e: Entry) => new URL(e.url).pathname.endsWith(p);

function fromHars() {
  save("wordpress", "home", "wordpress/browser-home.html", doc);
  save("wordpress", "post", "wordpress/browser-post.html", doc);
  save("wordpress", "page", "wordpress/browser-page.html", doc);
  save("wordpress", "category", "wordpress/browser-category.html", doc);
  save("wordpress", "create-post", "wordpress/editor-new.html", doc);
  save("wordpress", "update-post", "wordpress/editor-edit.html", doc);
  save("wordpress", "create-post", "wordpress/editor-save.json", (e) => e.method === "POST" && /\/posts\/\d+$/.test(new URL(e.url).pathname));
  save("wordpress", "update-post", "wordpress/editor-update.json", (e) => e.method === "POST" && /\/posts\/\d+$/.test(new URL(e.url).pathname));
  save("wordpress", "log-in", "wordpress/dashboard-browser.html", doc, 2);

  save("woocommerce", "shop", "woocommerce/shop-block.html", doc);
  save("woocommerce", "shop", "woocommerce/store-cart-read.json", path("/cart"), 0, ["Nonce", "Cart-Token"]);
  save("woocommerce", "product", "woocommerce/product-block.html", doc);
  save("woocommerce", "add-to-cart-product-page", "woocommerce/product-added.html", (e) => doc(e) && e.method === "POST");
  save("woocommerce", "add-to-cart-shop", "woocommerce/store-batch-add.json", path("/batch"));
  save("woocommerce", "cart-page", "woocommerce/cart-block.html", doc);
  save("woocommerce", "update-cart-item", "woocommerce/store-update-item-block.json", path("/update-item"));
  save("woocommerce", "remove-cart-item", "woocommerce/store-batch-remove.json", path("/batch"));
  save("woocommerce", "checkout-page", "woocommerce/checkout-block.html", doc);
  save("woocommerce", "place-order", "woocommerce/store-batch-customer.json", path("/batch"), 1);
  save("woocommerce", "place-order", "woocommerce/store-checkout-block.json", path("/checkout"));
  save("woocommerce", "place-order", "woocommerce/order-received-block.html", doc);
}

// wp-cli on the demo shop, through its setup script.
function wp(...args: string[]) {
  return execFileSync(join(ROOT, "examples/wordpress-sites/woocommerce/setup.sh"), ["wp", ...args], {
    env: { ...process.env, PORT: WC_PORT },
    encoding: "utf8",
  });
}

async function edges() {
  const api = await request.newContext({ baseURL: WC });
  const write = async (out: string, url: string) => {
    const response = await api.get(url);
    writeFileSync(join(FIXTURES, out), relative(await response.text(), WC));
    console.log(`${out} <- GET ${url} (${response.status()})`);
  };
  await write("woocommerce/shop-block-page-2.html", "/shop/page/2/");
  await write("woocommerce/checkout-block-empty.html", "/checkout/");

  // The order received page for an order, with a key that is not the order's.
  const { entries } = har("woocommerce", "place-order");
  const received = entries.find((e) => e.kind === "document")!;
  const wrong = new URL(received.url);
  wrong.searchParams.set("key", "wc_order_notthekey");
  await write("woocommerce/order-received-wrong-key.html", wrong.pathname + wrong.search);

  // An out of stock product: its page, and what adding it anyway answers.
  const id = wp("post", "list", "--post_type=product", "--name=cap", "--field=ID").trim();
  const stock = (status: string) =>
    wp("eval", `\$p = wc_get_product(${id}); \$p->set_stock_status("${status}"); \$p->save();`);
  stock("outofstock");
  try {
    await write("woocommerce/product-out-of-stock.html", "/product/cap/");
    const browser = await chromium.launch();
    const page = await browser.newPage();
    const response = await page.request.post(`${WC}/product/cap/`, { form: { quantity: "1", "add-to-cart": id } });
    writeFileSync(join(FIXTURES, "woocommerce/product-add-out-of-stock.html"), relative(await response.text(), WC));
    console.log(`woocommerce/product-add-out-of-stock.html <- POST /product/cap/ (${response.status()})`);
    await browser.close();
  } finally {
    stock("instock");
  }
  await api.dispose();
}

fromHars();
if (!process.argv.includes("--hars-only")) await edges();
