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

  return { doc, meta: doc.meta, modules: doc.modules, options: doc.options, modById, modOptions, lowerPaths };
}

// A definition's file: its own, else its module's (docs/schema.md).
function defFile(model, d) {
  if (d.file != null) return d.file;
  const m = d.module != null ? model.modById.get(d.module) : null;
  return m ? m.file : null;
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
