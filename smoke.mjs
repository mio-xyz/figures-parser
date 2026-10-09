import assert from "node:assert/strict";
import { createRequire } from "node:module";

const { extract, ungroundedFigures, ungroundedUrls, fabricatedUrls } = createRequire(import.meta.url)("./figures-parser.linux-x64-gnu.node");

const entities = extract("Total $1,250 and ١٢", { kinds: ["number"] });
const has = (text, integer) => entities.some((e) => e.text === text && e.value.integer === integer);
assert.ok(has("1,250", "1250"), "reads the grouped amount");
assert.ok(has("١٢", "12"), "reads Arabic-Indic digits");
assert.throws(() => extract("1", { kinds: ["phone"] }), /unknown kind "phone"/);
assert.deepEqual(extract("a 😀 7,500", { kinds: ["number"] }).map((e) => [e.text, e.start, e.end]), [["7,500", 5, 10], ["7,500", 5, 10], ["7", 5, 6], ["500", 7, 10]]);
const text = "😀 see <https://acme.io/a|A> and **https://acme.io/b**.";
const links = extract(text, { kinds: ["url"] });
assert.deepEqual(links.map((e) => [e.kind, e.text, text.slice(e.start, e.end)]), [["url", "https://acme.io/a", "https://acme.io/a"], ["url", "https://acme.io/b", "https://acme.io/b"]]);
assert.deepEqual(new Set(extract("Fee $1,250 at https://acme.io/x").map((e) => e.kind)), new Set(["number", "url"]));
assert.deepEqual(ungroundedUrls("See [#87](https://acme.io/repo) and https://acme.io/made-up.", "https://acme.io/repo"), ["https://acme.io/made-up", "[#87](https://acme.io/repo)"]);
assert.deepEqual(ungroundedUrls("Fix in [PR 512](https://example.com).", "user text", "[TASK-1](https://example.com)"), ["https://example.com"]);
assert.deepEqual(fabricatedUrls("https://acme.io/a and https://acme.io/b", "https://acme.io/a"), ["https://acme.io/b"]);
assert.deepEqual(ungroundedFigures("ARR is 1,010,000.", "Total: 101000"), ["1,010,000"]);
console.log("ok");
