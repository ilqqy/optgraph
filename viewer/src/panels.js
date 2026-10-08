// Top bar, warnings panel, option detail (priority ladder), module panel.

function renderMeta(model) {
  const m = model.meta;
  const items = [
    h("span", {}, "host ", h("b", {}, m.host ?? "?")),
    h("span", {}, "nixpkgs ", h("b", {}, m.nixpkgsVersion ?? "?")),
    h("span", { title: "aligned: modules lined up with nixosSystem's module lists; file-based: fallback attribution" }, "attribution ", h("b", { class: m.attribution === "aligned" ? null : "flag-bad" }, m.attribution)),
    h("span", { title: "false when optgraph ran out of retries or time: partial output" }, "complete ", h("b", { class: m.complete ? null : "flag-bad" }, String(m.complete))),
    h("span", {}, "scope ", h("b", {}, m.scope)),
    h("span", {}, `${model.modules.length} modules, ${model.options.length} options`),
  ];
  $("#meta").replaceChildren(...items);

  const btn = $("#warnings-btn");
  const n = m.warnings.length;
  btn.hidden = false;
  btn.textContent = plural(n, "warning");
  btn.classList.toggle("has", n > 0);
  $("#warnings-list").replaceChildren(
    ...(n
      ? m.warnings.map((w) =>
          h("div", { class: "warning" }, h("div", {}, h("code", {}, w.code), w.subject ? h("span", { class: "muted" }, "  ", w.subject) : null), h("pre", {}, w.message)),
        )
      : [h("div", { class: "warning muted" }, "No warnings.")]),
  );
}

// Sort key for the ladder: known priorities first (lowest wins), unknown last.
function ladderOrder(defs) {
  return defs
    .map((d, i) => ({ d, i }))
    .sort((a, b) => (a.d.priority ?? Infinity) - (b.d.priority ?? Infinity) || a.i - b.i);
}

// Start state (nothing selected): summary and the options worth a look.
const START_LIST_LIMIT = 200;

function renderStart(model, onOption) {
  const a = model.analysis;
  const s = a.stats;
  const tile = (n, label, title) => h("div", { class: "tile", title }, h("b", {}, n.toLocaleString("en")), h("span", {}, label));
  const row = (oi, extra, title) =>
    h("li", { onclick: () => onOption(oi), title }, h("span", { class: "path" }, model.options[oi].path), extra);
  // Two lines: the path, then a note (long notes would squeeze the path).
  const row2 = (oi, note, cls) =>
    h("li", { class: "two", onclick: () => onOption(oi), title: note }, h("span", { class: "path" }, model.options[oi].path), h("span", { class: `note ${cls || ""}` }, note));
  const list = (title, hint, items, render, emptyText) => [
    h("h3", {}, title, h("span", { class: "count" }, String(items.length)), hint ? h("span", { class: "hint" }, hint) : null),
    items.length
      ? h(
          "ul",
          { class: "picklist" },
          ...items.slice(0, START_LIST_LIMIT).map(render),
          items.length > START_LIST_LIMIT ? h("li", { class: "more" }, `${items.length - START_LIST_LIMIT} more: use the search`) : null,
        )
      : h("p", { class: "muted" }, emptyText),
  ];
  return [
    h(
      "div",
      { class: "tiles" },
      tile(s.modules, "modules"),
      tile(s.options, "options"),
      tile(s.definitions, "definitions", "listed definitions, without option defaults and without the nixpkgs definitions counted as +N"),
      tile(s.overrides, "overrides", "options where a listed definition lost to a stronger one (the option default losing does not count)"),
      tile(s.switchedOff, "switched off", "definitions from non-nixpkgs modules under a false mkIf"),
      tile(s.errors, "errors", "options with an error (a value or condition threw)"),
    ),
    h("p", { class: "muted intro" }, "Select an option to see who set it, at what priority, and why the others lost. Click a module in the graph to see what it sets."),
    ...list(
      "Overrides",
      "between your modules first",
      a.overrides,
      (oi) => row2(oi, a.info[oi].beats),
      "No listed definition lost.",
    ),
    ...list(
      "Switched off",
      "mkIf false",
      a.switchedOff,
      ({ oi, di }) => {
        const m = model.modById.get(model.options[oi].definitions[di].module);
        return row(oi, h("span", { class: "aside" }, moduleShortLabel(m)), moduleLabel(m));
      },
      "No definition is switched off.",
    ),
    ...list(
      "Errors",
      null,
      a.errors,
      (oi) => row2(oi, model.options[oi].error, "error"),
      "No option has an error.",
    ),
  ];
}

function renderOption(model, oi, onModule, onCopyLink) {
  const o = model.options[oi];
  const winners = new Set(o.winners);
  const omittedN = o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive;
  const copyBtn = h("button", { type: "button", class: "copy-link", title: "Copy a link to this option" }, "Copy link");
  copyBtn.addEventListener("click", () =>
    onCopyLink().then((ok) => {
      copyBtn.textContent = ok ? "Copied" : "Copy failed";
      setTimeout(() => (copyBtn.textContent = "Copy link"), 1500);
    }),
  );
  const head = [
    h("div", { class: "title-row" }, h("h2", {}, o.path), copyBtn),
    h("div", { class: "kv" }, "type ", h("code", {}, o.type ?? "?")),
    o.declaredIn.length ? h("div", { class: "kv" }, "declared in ", ...o.declaredIn.map((f) => h("code", { title: f }, shortPath(f), " "))) : null,
    h(
      "div",
      { class: "kv" },
      o.highestPrio == null ? "no known winner" : `winning priority ${priorityLabel(o.highestPrio)}`,
      omittedN
        ? h(
            "span",
            {
              class: "badge",
              style: "margin-left:8px",
              title: `${o.omitted.nixpkgsActive} active and ${o.omitted.nixpkgsInactive} inactive nixpkgs definitions are not listed (optgraph --include-all-definitions lists them)`,
            },
            `+${omittedN} nixpkgs`,
          )
        : null,
    ),
    o.error ? h("div", { class: "error" }, o.error) : null,
  ];
  const items = ladderOrder(o.definitions).map(({ d, i }) => {
    const mod = d.module != null ? model.modById.get(d.module) : null;
    const isWinner = winners.has(i);
    const cls = !d.active ? "inactive" : isWinner ? "winner" : "loser";
    const file = defFile(model, d);
    const where =
      d.kind === "default"
        ? h("div", { class: "where" }, "option default, ", shortPath(file))
        : h(
            "div",
            { class: "where" },
            mod ? h("span", { class: "linkish", onclick: () => onModule(mod) }, moduleLabel(mod)) : shortPath(file),
            mod && file && file !== mod.file ? ` (file ${shortPath(file)})` : null,
          );
    const preview =
      d.valuePreview == null
        ? h(
            "div",
            { class: "muted", style: "font-size:12px;margin-top:2px" },
            !d.active ? "not evaluated (inactive)" : d.priority == null ? "value threw (see error)" : "no preview",
          )
        : h("pre", { class: d.valuePreview === "<redacted>" ? "muted" : null }, d.valuePreview);
    return h(
      "li",
      { class: cls },
      h(
        "div",
        { class: "top" },
        h("span", { class: "prio", title: "override priority: lower wins" }, priorityLabel(d.priority)),
        d.kind === "default" ? h("span", { class: "chip o-nixpkgs" }, "default") : chip(mod ? mod.origin : "unknown"),
        isWinner ? h("span", { class: "win-tag" }, "winner") : null,
        d.condition ? h("span", { class: "badge cond", title: "enclosing mkIf condition" }, d.condition) : null,
      ),
      where,
      preview,
    );
  });
  return [
    ...head,
    items.length ? h("ol", { class: "ladder" }, ...items) : h("p", { class: "muted" }, "No definitions listed."),
  ].filter(Boolean);
}

function renderModule(model, m, onOption) {
  const opts = model.modOptions.get(m.id) || [];
  const list = h("div", { class: "vlist", style: "height:320px;border:1px solid var(--border);border-radius:6px;margin-top:8px" });
  const head = [
    h("h2", {}, moduleLabel(m)),
    h("div", { class: "kv" }, chip(m.origin), m.disabled ? h("span", { class: "badge cond", style: "margin-left:6px" }, "disabled") : null),
    h("div", { class: "kv" }, "id ", h("code", {}, m.id)),
    m.id !== m.file ? h("div", { class: "kv" }, "file ", h("code", {}, m.file)) : null,
    m.position ? h("div", { class: "kv" }, "position ", h("code", {}, m.position)) : null,
    h("div", { class: "kv" }, `imports ${m.imports.length}; sets ${plural(opts.length, "listed option")}`),
    list,
  ].filter(Boolean);
  // The list is attached after the caller inserts `head`.
  queueMicrotask(() => {
    const vl = new VirtualList(list, (oi) => h("div", { onclick: () => onOption(oi) }, h("span", { class: "path" }, model.options[oi].path)));
    vl.setItems(opts, "This module sets no listed option.");
  });
  return head;
}
