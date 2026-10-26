// Expected layouts from the web viewer's own layout.js, for the parity tests
// in crates/optgraph-core/tests/layout.rs:
//   node app/scripts/expect-layout.js viewer/src graph.json > expected.json
// Node sizes are fixed (label length based) so Rust and JS lay out the same
// input; edges are the module imports, as viewer/src/graph.js builds them.
const fs = require("fs");
const [src, file] = process.argv.slice(2);
const code = ["util.js", "data.js", "layout.js"].map((f) => fs.readFileSync(`${src}/${f}`, "utf8")).join("\n");
const run = new Function("doc", `${code}
const model = buildModel(doc);
const graphCase = (mods) => {
  const ids = new Map(mods.map((m, i) => [m.id, i]));
  const sizes = mods.map((m) => ({ w: 10 * moduleShortLabel(m).length + 52, h: 28 }));
  const edges = [];
  mods.forEach((m, i) => m.imports.forEach((t) => ids.has(t) && edges.push([i, ids.get(t)])));
  return { sizes, edges, out: layeredLayout(sizes, edges) };
};
// Synthetic: a cycle (0>1>2>0), a long edge (0>5 over 3 layers), a diamond,
// a self loop and 8 isolated nodes (packed into a grid).
const synth = () => {
  const sizes = Array.from({ length: 20 }, (_, i) => ({ w: 40 + 7 * (i % 5), h: 28 }));
  const edges = [[0, 1], [1, 2], [2, 0], [2, 3], [3, 4], [4, 5], [0, 5], [6, 7], [6, 8], [7, 9], [8, 9], [9, 9], [1, 6]];
  return { sizes, edges, out: layeredLayout(sizes, edges) };
};
// Dense: 60 nodes, edges from a fixed LCG (forward and some backward), so
// the sweeps have crossings to remove.
const dense = () => {
  let x = 12345;
  const rnd = (n) => ((x = (x * 1103515245 + 12345) % 2147483648), x % n);
  const sizes = Array.from({ length: 60 }, (_, i) => ({ w: 50 + rnd(80), h: 28 }));
  const edges = [];
  for (let i = 0; i < 110; i++) {
    const a = rnd(60), b = rnd(60);
    edges.push(rnd(10) === 0 ? [Math.max(a, b), Math.min(a, b)] : [Math.min(a, b), Math.max(a, b)]);
  }
  return { sizes, edges, out: layeredLayout(sizes, edges) };
};
return { demo: graphCase(model.modules), synthetic: synth(), dense: dense() };`);
process.stdout.write(JSON.stringify(run(JSON.parse(fs.readFileSync(file, "utf8")))) + "\n");
