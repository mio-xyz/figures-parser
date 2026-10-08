import assert from "node:assert/strict";
import { createRequire } from "node:module";

const { extract, ungroundedFigures } = createRequire(import.meta.url)("./figures-parser.linux-x64-gnu.node");

const entities = extract("Total $1,250 and ١٢");
const has = (text, integer) => entities.some((e) => e.text === text && e.value.integer === integer);
assert.ok(has("1,250", "1250"), "reads the grouped amount");
assert.ok(has("١٢", "12"), "reads Arabic-Indic digits");
assert.throws(() => extract("1", { kinds: ["phone"] }), /unknown kind "phone"/);
assert.deepEqual(extract("a 😀 7,500").map((e) => [e.text, e.start, e.end]), [["7,500", 5, 10], ["7,500", 5, 10], ["7", 5, 6], ["500", 7, 10]]);
assert.deepEqual(ungroundedFigures("ARR is 1,010,000.", "Total: 101000"), ["1,010,000"]);
console.log("ok");
