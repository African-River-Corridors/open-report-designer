// The WebAssembly package renders the example in all three formats, and each output equals the native
// `ord` program's (spec 0001 §12 "Bindings").
//   node tools/wasm_smoke.mjs <package dir (nodejs target)> <native output dir>
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, dirname, resolve } from "node:path";
import { strict as assert } from "node:assert";

const [pkg, nativeDir] = process.argv.slice(2);
const ord = createRequire(import.meta.url)(resolve(pkg, "ord.js"));
const docPath = "examples/screening-report.json";
const json = readFileSync(docPath, "utf8");
const doc = JSON.parse(json);
const assets = new Map(Object.entries(doc.assets).map(([id, a]) => [id, readFileSync(join(dirname(docPath), a.path))]));

let failed = 0;
for (const fmt of ["pdf", "html", "docx"]) {
  const t0 = Date.now();
  const out = Buffer.from(ord.render(json, fmt, assets));
  const native = readFileSync(join(nativeDir, `screening-report.${fmt}`));
  const same = out.equals(native);
  console.log(`${fmt}: ${out.length} bytes in ${Date.now() - t0} ms, ${same ? "equal to" : "DIFFERENT from"} native`);
  if (!same) failed++;
}
assert.deepEqual(JSON.parse(ord.check(json, assets)), []);
let threw = false;
try { ord.render(json.replace('"class": "report"', '"class": "memo"'), "html", assets); } catch (e) {
  threw = JSON.parse(e.message).some((f) => f.code === "ORD002");
}
assert.ok(threw, "a document with errors throws its findings");
assert.equal(JSON.parse(ord.schema()).title, "Open Report Designer document");
console.log(`version ${ord.version()}`);
process.exit(failed ? 1 : 0);
