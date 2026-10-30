// Expected derived data from the web viewer's own code, for the parity tests
// in crates/optgraph-core/tests:
//   node app/scripts/expect-analysis.js viewer/src graph.json > expected.json
const fs = require("fs");
const [src, file] = process.argv.slice(2);
const code = ["util.js", "data.js", "search.js"].map((f) => fs.readFileSync(`${src}/${f}`, "utf8")).join("\n");
const out = new Function("doc", `${code}\nconst model = buildModel(doc);
const a = model.analysis;
return {
  stats: a.stats,
  overrides: a.overrides, withUser: a.withUser, nixpkgsOnly: a.nixpkgsOnly,
  switchedOff: a.switchedOff.map(({oi, di}) => [oi, di]), errors: a.errors,
  options: model.options.map((o, i) => ({
    path: o.path,
    override: a.info[i].override, userVsUser: a.info[i].userVsUser, beats: a.info[i].beats,
    beatsModules: a.info[i].beatsParts ? \`\${a.info[i].beatsParts.win} in \${a.info[i].beatsParts.winWho} beats \${a.info[i].beatsParts.lose} in \${a.info[i].beatsParts.loseWho}\` : null,
    off: a.info[i].off,
    ladder: ladder(model, o).map((r) => [r.i, r.status, r.reason]),
    crumbs: breadcrumb(model, o),
  })),
  modules: model.modules.map((m) => [moduleShortLabel(m), moduleLabel(m)]),
  palette: ["", "fire", "nfe", "core", "boot.l", "hardening", "enable", "zz"].map((q) => [q, paletteSearch(model, q).map((g) => [g.key, g.total, g.items.map((it) => [it.kind, it.index, it.hits, it.where ?? null, it.score])])]),
};`)(JSON.parse(fs.readFileSync(file, "utf8")));
process.stdout.write(JSON.stringify(out, null, 1) + "\n");
