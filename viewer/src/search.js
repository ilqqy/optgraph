// Fuzzy search over option paths: substring matches rank first, then
// subsequence matches with fewer gaps and earlier/segment-start hits.

// Returns { score, hits: [indices] } or null. Higher score is better.
function fuzzyMatch(query, text) {
  if (!query) return { score: 0, hits: [] };
  const sub = text.indexOf(query);
  if (sub >= 0) {
    const atSegment = sub === 0 || text[sub - 1] === ".";
    const hits = [];
    for (let i = 0; i < query.length; i++) hits.push(sub + i);
    return { score: 1000 - sub - text.length * 0.1 + (atSegment ? 50 : 0), hits };
  }
  const hits = [];
  let score = 0;
  let last = -1;
  for (let qi = 0, ti = 0; qi < query.length; qi++) {
    const c = query[qi];
    while (ti < text.length && text[ti] !== c) ti++;
    if (ti === text.length) return null;
    if (last >= 0) score -= Math.min(ti - last - 1, 10);
    if (ti === 0 || text[ti - 1] === ".") score += 5;
    hits.push(ti);
    last = ti;
    ti++;
  }
  return { score: score - text.length * 0.1, hits };
}

// -> [{ index, hits }] sorted by score. For an empty query: all options in
// sections (overrides, set by non-nixpkgs modules, nixpkgs only), each
// preceded by a { header } row.
function searchOptions(model, rawQuery) {
  const q = rawQuery.trim().toLowerCase().replace(/\s+/g, "");
  if (!q) {
    const a = model.analysis;
    const sections = [
      ["Overrides", a.overrides],
      ["Set by your modules", a.withUser],
      ["nixpkgs only", a.nixpkgsOnly],
    ];
    return sections.flatMap(([title, list]) =>
      list.length ? [{ header: `${title} (${list.length})` }, ...list.map((i) => ({ index: i, hits: [] }))] : [],
    );
  }
  const out = [];
  for (let i = 0; i < model.lowerPaths.length; i++) {
    const m = fuzzyMatch(q, model.lowerPaths[i]);
    if (m) out.push({ index: i, score: m.score, hits: m.hits });
  }
  if (q) out.sort((a, b) => b.score - a.score || a.index - b.index);
  return out;
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
