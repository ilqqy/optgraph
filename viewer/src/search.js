// Fuzzy search over option paths: substring matches rank first, then
// subsequence matches with fewer gaps and earlier/segment-start hits.

// Returns { score, hits: [indices] } or null. Higher score is better.
// `orig` (the text before lower-casing) marks camelCase word starts.
function fuzzyMatch(query, text, orig = text) {
  if (!query) return { score: 0, hits: [] };
  const wordStart = (i) => i === 0 || text[i - 1] === "." || text[i - 1] === "/" || text[i - 1] === "-" || (orig[i] !== text[i] && orig[i - 1] === text[i - 1]);
  const sub = text.indexOf(query);
  if (sub >= 0) {
    const hits = [];
    for (let i = 0; i < query.length; i++) hits.push(sub + i);
    const atStart = text[sub - 1] === "." || sub === 0 ? 50 : wordStart(sub) ? 30 : 0;
    const atEnd = sub + query.length === text.length ? 25 : 0;
    return { score: 1000 - sub - text.length * 0.1 + atStart + atEnd, hits };
  }
  const hits = [];
  let score = 0;
  let last = -1;
  for (let qi = 0, ti = 0; qi < query.length; qi++) {
    const c = query[qi];
    while (ti < text.length && text[ti] !== c) ti++;
    if (ti === text.length) return null;
    if (last >= 0) score -= Math.min(ti - last - 1, 10);
    if (wordStart(ti)) score += 5;
    hits.push(ti);
    last = ti;
    ti++;
  }
  return { score: score - text.length * 0.1, hits };
}

// Palette groups for a query: options go to the first group they belong to
// (Overrides, Switched off, Errors, else Options), modules match their short
// label or their path. If some path or label contains the query, only those
// are listed. Each group sorted by score, cut to its limit; an empty query
// lists the start of each group.
//   -> [{ key, title, total, items: [{ kind: "option"|"module", index, hits, where }] }]
const PALETTE_GROUPS = [
  ["overrides", "Overrides", 6],
  ["off", "Switched off", 6],
  ["errors", "Errors", 6],
  ["options", "Options", 40],
  ["modules", "Modules", 8],
];

function paletteSearch(model, rawQuery) {
  const q = rawQuery.trim().toLowerCase().replace(/\s+/g, "");
  const a = model.analysis;
  if (!model.paletteSets) {
    model.paletteSets = {
      overrides: new Set(a.overrides),
      off: new Set(a.switchedOff.map((x) => x.oi)),
      errors: new Set(a.errors),
    };
    model.moduleKeys = model.modules.map((m) => [moduleShortLabel(m), moduleLabel(m)]);
  }
  const sets = model.paletteSets;
  const groupOf = (oi) => (sets.overrides.has(oi) ? "overrides" : sets.off.has(oi) ? "off" : sets.errors.has(oi) ? "errors" : "options");
  const buckets = { overrides: [], off: [], errors: [], options: [], modules: [] };
  if (!q) {
    const seen = new Set();
    const add = (key, oi) => !seen.has(oi) && (seen.add(oi), buckets[key].push({ kind: "option", index: oi, hits: [], score: 0 }));
    a.overrides.forEach((oi) => add("overrides", oi));
    a.switchedOff.forEach(({ oi }) => add("off", oi));
    a.errors.forEach((oi) => add("errors", oi));
    a.withUser.forEach((oi) => add("options", oi));
    model.modules.forEach((m, i) => isUserish(m) && buckets.modules.push({ kind: "module", index: i, hits: [], where: "label", score: 0 }));
  } else {
    for (let i = 0; i < model.lowerPaths.length; i++) {
      const m = fuzzyMatch(q, model.lowerPaths[i], model.options[i].path);
      if (m) buckets[groupOf(i)].push({ kind: "option", index: i, hits: m.hits, score: m.score });
    }
    model.moduleKeys.forEach(([label, path], i) => {
      const ml = fuzzyMatch(q, label.toLowerCase(), label);
      const mp = ml ? null : fuzzyMatch(q, path.toLowerCase(), path);
      if (ml || mp) buckets.modules.push({ kind: "module", index: i, hits: (ml || mp).hits, where: ml ? "label" : "path", score: (ml || mp).score - (ml ? 0 : 20) });
    });
    // Substring hits (scores near 1000) hide scattered subsequence ones.
    if (Object.values(buckets).some((list) => list.some((it) => it.score >= 500))) {
      for (const k of Object.keys(buckets)) buckets[k] = buckets[k].filter((it) => it.score >= 500);
    }
    for (const list of Object.values(buckets)) list.sort((x, y) => y.score - x.score || x.index - y.index);
  }
  return PALETTE_GROUPS.map(([key, title, limit]) => ({ key, title, total: buckets[key].length, items: buckets[key].slice(0, limit) })).filter((g) => g.items.length);
}

// Path with matched characters wrapped in <mark>.
function highlighted(path, hits) {
  if (!hits.length) return [path];
  const set = new Set(hits);
  const parts = [];
  let buf = "";
  let inMark = false;
  for (let i = 0; i < path.length; i++) {
    const m = set.has(i);
    if (m !== inMark) {
      if (buf) parts.push(inMark ? h("mark", {}, buf) : buf);
      buf = "";
      inMark = m;
    }
    buf += path[i];
  }
  if (buf) parts.push(inMark ? h("mark", {}, buf) : buf);
  return parts;
}
