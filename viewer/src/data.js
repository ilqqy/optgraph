// graph.json -> indexed model. See docs/schema.md for the fields.

function buildModel(doc) {
  if (!doc || typeof doc !== "object" || !doc.meta || !Array.isArray(doc.modules) || !Array.isArray(doc.options)) {
    throw new Error("not an optgraph graph.json (expected meta, modules and options)");
  }
  const modById = new Map();
  for (const m of doc.modules) modById.set(m.id, m);

  // module id -> indices of the options it defines (listed definitions only)
  const modOptions = new Map();
  doc.options.forEach((o, i) => {
    const seen = new Set();
    for (const d of o.definitions) {
      if (d.module == null || seen.has(d.module)) continue;
      seen.add(d.module);
      if (!modOptions.has(d.module)) modOptions.set(d.module, []);
      modOptions.get(d.module).push(i);
    }
  });

  // Lower-cased paths for search, computed once.
  const lowerPaths = doc.options.map((o) => o.path.toLowerCase());
  const modIndex = new Map(doc.modules.map((m, i) => [m.id, i]));

  const model = { doc, meta: doc.meta, modules: doc.modules, options: doc.options, modById, modIndex, modOptions, lowerPaths };
  model.analysis = analyse(model);
  return model;
}

// Per-option facts for the start panel and the empty-query order.
//   override: a listed definition lost: active, known priority, not a winner,
//     to a listed winner or to omitted nixpkgs winners (omitted.nixpkgsWinners).
//     The option default losing does not count (that is just setting an
//     option), and neither do omitted nixpkgs definitions that lose.
//   userVsUser: winner and strongest loser both come from non-nixpkgs modules.
//   off: indices of mkIf-false definitions from non-nixpkgs modules.
function analyse(model) {
  const info = model.options.map((o) => {
    const winners = new Set(o.winners);
    let hasUser = false, loser = null;
    const off = [];
    o.definitions.forEach((d, i) => {
      if (d.kind !== "definition") return;
      const user = defOrigin(model, d) !== "nixpkgs";
      if (user) hasUser = true;
      if (winners.has(i)) return;
      if (!d.active) {
        if (user && d.condition === "mkIf-false") off.push(i);
      } else if (d.priority != null && (loser == null || d.priority < loser.priority)) {
        loser = d;
      }
    });
    const hidden = hiddenWinners(o);
    // No known winner (winners is empty, nothing omitted wins): no override.
    if (!loser || (!o.winners.length && !hidden)) return { hasUser, override: false, userVsUser: false, beats: null, beatsParts: null, off };
    const winner = o.winners.length ? o.definitions[o.winners[0]] : null;
    const wo = winner ? defOrigin(model, winner) : "nixpkgs";
    const lo = defOrigin(model, loser);
    return {
      hasUser,
      override: true,
      userVsUser: wo !== "nixpkgs" && lo !== "nixpkgs",
      // "mkForce (user) beats mkDefault (input:shared)"
      beats: `${prioName(o.highestPrio)} (${wo}) beats ${prioName(loser.priority)} (${lo})`,
      // "mkForce" in "hardening.nix" beats "mkDefault" in "dev-tools.nix"
      beatsParts: { win: prioName(o.highestPrio), winWho: winner ? defWho(model, winner) : `nixpkgs (${hidden} not listed)`, lose: prioName(loser.priority), loseWho: defWho(model, loser) },
      off,
    };
  });
  const byPath = (a, b) => a - b; // options are sorted by path
  const all = info.map((_, i) => i);
  const overrides = all.filter((i) => info[i].override).sort((a, b) => info[b].userVsUser - info[a].userVsUser || byPath(a, b));
  const withUser = all.filter((i) => !info[i].override && info[i].hasUser);
  const nixpkgsOnly = all.filter((i) => !info[i].override && !info[i].hasUser);
  const switchedOff = all.flatMap((i) => info[i].off.map((d) => ({ oi: i, di: d })));
  const errors = all.filter((i) => model.options[i].error != null);
  const definitions = model.options.reduce((n, o) => n + o.definitions.filter((d) => d.kind === "definition").length, 0);
  return {
    info,
    overrides,
    withUser,
    nixpkgsOnly,
    switchedOff,
    errors,
    stats: {
      modules: model.modules.length,
      options: model.options.length,
      definitions,
      overrides: overrides.length,
      switchedOff: switchedOff.length,
      errors: errors.length,
    },
  };
}

// The ladder of an option: its definitions sorted by priority (lowest first,
// unknown last, then listing order), each with a status and a one-line
// reason computed from the data:
//   win   "beats dev-tools.nix: 50 < 1000", "merged with 5 others at 100"
//   lose  "beaten by hardening.nix: 50 < 1000",
//         "beaten by 3 nixpkgs definitions at 100 (not listed): 100 < 1000"
// Omitted nixpkgs winners (omitted.nixpkgsWinners) count as winners here.
//   off   "switched off: its mkIf condition is false"
//   unknown (priority unknown, or no known winner)
function ladder(model, o) {
  const winners = new Set(o.winners);
  const hp = o.highestPrio;
  const defs = o.definitions.map((d, i) => ({ d, i }));
  const active = defs.filter(({ d, i }) => d.active && d.priority != null && !winners.has(i));
  const strongestLoser = active.reduce((best, x) => (best == null || x.d.priority < best.d.priority ? x : best), null);
  const firstWinner = o.winners.length ? o.definitions[o.winners[0]] : null;
  const hidden = hiddenWinners(o);
  const winCount = o.winners.length + hidden;
  const notListed = hidden ? ` (${hidden} nixpkgs, not listed)` : "";
  const rows = defs.map(({ d, i }) => {
    let status;
    let reason;
    if (winners.has(i)) {
      status = "win";
      if (winCount > 1) reason = `merged with ${plural(winCount - 1, "other")} at priority ${hp}${notListed}`;
      else if (strongestLoser) reason = `beats ${defWho(model, strongestLoser.d)}: ${hp} < ${strongestLoser.d.priority}`;
      else if (d.kind === "default") reason = "nothing stronger is set: the default applies";
      else reason = "the only active definition";
    } else if (!d.active) {
      status = "off";
      reason = d.condition === "mkIf-error" ? "its mkIf condition threw or is not a bool" : "switched off: its mkIf condition is false";
    } else if (d.priority == null) {
      status = "unknown";
      reason = "reading its value threw: priority unknown";
    } else if (hp == null) {
      status = "unknown";
      reason = "the winner is unknown (see the error)";
    } else if (winCount === 0) {
      // A graph.json from before omitted.nixpkgsWinners.
      status = "unknown";
      reason = "the winners are not listed";
    } else {
      status = "lose";
      if (!o.winners.length) reason = `beaten by ${plural(hidden, "nixpkgs definition")} at ${hp} (not listed): ${hp} < ${d.priority}`;
      else if (winCount === 1) reason = `beaten by ${defWho(model, firstWinner)}: ${hp} < ${d.priority}`;
      else reason = `beaten by ${winCount} definitions at ${hp}${notListed}: ${hp} < ${d.priority}`;
    }
    return { d, i, status, reason };
  });
  return rows.sort((a, b) => (a.d.priority ?? Infinity) - (b.d.priority ?? Infinity) || a.i - b.i);
}

// Winners counted in `omitted` instead of listed (0 in graphs from before
// the field, and whenever there is a single winner: it is always listed).
function hiddenWinners(o) {
  return o.omitted.nixpkgsWinners ?? 0;
}

// A definition's file: its own, else its module's (docs/schema.md).
function defFile(model, d) {
  if (d.file != null) return d.file;
  const m = d.module != null ? model.modById.get(d.module) : null;
  return m ? m.file : null;
}

// Short name of who made a definition: the module's short label, or "the default".
function defWho(model, d) {
  if (d.kind === "default") return "the default";
  const m = d.module != null ? model.modById.get(d.module) : null;
  return m ? moduleShortLabel(m) : baseName(defFile(model, d));
}

function defOrigin(model, d) {
  const m = d.module != null ? model.modById.get(d.module) : null;
  return m ? m.origin : d.kind === "default" ? "declaration" : "unknown";
}

// Inline user modules carry nixpkgs' flake.nix as their file: label them by
// their place in nixosSystem's `modules` list (and position) instead.
function moduleLabel(m) {
  if (!m) return "?";
  if (m.origin === "user-inline") {
    const where = m.position ? ` (${baseName(m.position)})` : "";
    return m.modulesIndex != null ? `inline module #${m.modulesIndex}${where}` : `nested inline module${where}`;
  }
  const anon = m.id !== m.file;
  const p = m.position && anon ? m.position : m.file;
  return anon ? `${shortPath(p)} (anonymous)` : shortPath(p);
}

function isUserish(m) {
  return m.origin !== "nixpkgs";
}

// Short label for graph nodes: the file name, or the inline index.
function moduleShortLabel(m) {
  if (!m) return "?";
  if (m.origin === "user-inline") return m.modulesIndex != null ? `inline #${m.modulesIndex}` : "inline (nested)";
  const anon = m.id !== m.file;
  return anon ? `${baseName(m.position || m.file)} (anon)` : baseName(m.file);
}
