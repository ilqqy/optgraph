// Top bar, sidebar lists, warnings panel, start summary, option detail
// (priority ladder), module panel.

// Status chip: a mark (✓ or !) and text, so the state never depends on colour.
function statusChip(ok, text, title, onclick) {
  return h(
    onclick ? "button" : "span",
    { class: `status-chip ${ok ? "ok" : "bad"}`, title, type: onclick ? "button" : null, onclick },
    h("span", { class: "mark", "aria-hidden": "true" }, ok ? "✓" : "!"),
    text,
  );
}

function renderMeta(model, onErrors) {
  const m = model.meta;
  $("#crumb").replaceChildren(
    h("b", { class: "host", title: "nixosConfigurations.<host>" }, m.host ?? "?"),
    h("span", { class: "sep", "aria-hidden": "true" }, "/"),
    h("span", { class: "ver", title: `nixpkgs ${m.nixpkgsRev ?? ""}`.trim() }, "nixpkgs ", m.nixpkgsVersion ?? "?"),
  );
  const n = m.warnings.length;
  const errs = model.analysis.errors.length;
  $("#status").replaceChildren(
    statusChip(m.attribution === "aligned", m.attribution, "aligned: modules lined up with nixosSystem's module lists; file-based: fallback attribution"),
    statusChip(m.complete, m.complete ? "complete" : "partial", "complete: false when optgraph ran out of retries or time (partial output)"),
    statusChip(n === 0, plural(n, "warning"), "meta.warnings: click to list them", () => ($("#warnings-panel").hidden = !$("#warnings-panel").hidden)),
    statusChip(errs === 0, plural(errs, "option error"), "options whose value or mkIf condition threw", errs ? onErrors : null),
  );
  $("#counts").textContent = `${model.modules.length.toLocaleString("en")} modules · ${model.options.length.toLocaleString("en")} options · scope ${m.scope}`;
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

// Sidebar: the options worth a look, as three lists with counts.
const SIDE_LIST_LIMIT = 200;

function renderLists(model, onOption) {
  const a = model.analysis;
  const section = (key, title, ico, items, render, emptyText) =>
    h(
      "section",
      { class: `side-list l-${key}`, "data-list": key },
      h("h2", {}, icon(ico, `i-${key}`), h("span", {}, title), h("span", { class: "count" }, items.length.toLocaleString("en"))),
      items.length
        ? h(
            "ul",
            {},
            ...items.slice(0, SIDE_LIST_LIMIT).map(render),
            items.length > SIDE_LIST_LIMIT ? h("li", { class: "more" }, `${(items.length - SIDE_LIST_LIMIT).toLocaleString("en")} more: use the search`) : null,
          )
        : h("p", { class: "empty-note" }, emptyText),
    );
  const row = (oi, title, data) =>
    h("li", { title, "data-oi": oi, ...data }, h("button", { type: "button", onclick: () => onOption(oi) }, h("span", { class: "path" }, model.options[oi].path)));
  return [
    section("overrides", "Overrides", "override", a.overrides, (oi) => row(oi, a.info[oi].beats), "No listed definition lost."),
    section(
      "off",
      "Switched off",
      "off",
      a.switchedOff,
      ({ oi, di }) => {
        const m = model.modById.get(model.options[oi].definitions[di].module);
        return row(oi, `mkIf false in ${moduleLabel(m)}`, { "data-di": di });
      },
      "No definition is switched off.",
    ),
    section("errors", "Errors", "error", a.errors, (oi) => row(oi, model.options[oi].error), "No option threw."),
  ];
}

// Start state (nothing selected): summary, the priority scale and the first
// overrides.
function renderStart(model, onOption) {
  const a = model.analysis;
  const s = a.stats;
  const tile = (n, label, title, cls) => h("div", { class: `stat${cls ? " " + cls : ""}`, title }, h("b", {}, n.toLocaleString("en")), h("span", {}, label));
  const tick = (p) => h("div", {}, h("b", {}, String(p)), h("span", {}, PRIORITY_NAMES[p]));
  const first = a.overrides.slice(0, 4);
  return [
    h("div", { class: "overline" }, "Overview"),
    h("h1", { class: "hello" }, "Who set what, and why it won."),
    h("p", { class: "lede" }, "Pick an option to see every definition stacked by priority, or click a module in the graph to see what it sets."),
    h(
      "div",
      { class: "stats" },
      tile(s.modules, "modules"),
      tile(s.options, "options"),
      tile(s.definitions, "definitions", "listed definitions, without option defaults and without the nixpkgs definitions counted as +N"),
      tile(s.overrides, "overrides", "options where a listed definition lost to a stronger one (the option default losing does not count)", s.overrides ? "lose" : null),
      tile(s.switchedOff, "switched off", "definitions from non-nixpkgs modules under a false mkIf"),
      tile(s.errors, "errors", "options with an error (a value or condition threw)", s.errors ? "err" : null),
    ),
    h("h2", { class: "subhead" }, "Priority: lowest number wins"),
    h(
      "div",
      { class: "scale", role: "img", "aria-label": "Priorities from strongest to weakest: 50 mkForce, 100 normal, 1000 mkDefault, 1500 default" },
      h("div", { class: "scale-cap" }, h("span", {}, "stronger"), h("span", {}, "weaker")),
      h("div", { class: "scale-bar" }),
      h("div", { class: "scale-ticks" }, tick(50), tick(100), tick(1000), tick(1500)),
    ),
    first.length ? h("h2", { class: "subhead" }, a.info[first[0]].userVsUser ? "Overrides between your modules" : "Overrides") : null,
    ...first.map((oi) => {
      const b = a.info[oi].beatsParts;
      return h(
        "button",
        { type: "button", class: "override-card", onclick: () => onOption(oi) },
        h("span", { class: "path" }, model.options[oi].path),
        h("span", { class: "beats" }, h("code", { class: "t-win" }, b.win), ` in ${b.winWho} beats `, h("code", { class: "t-lose" }, b.lose), ` in ${b.loseWho}`),
      );
    }),
    h(
      "div",
      { class: "keys" },
      h("span", {}, h("kbd", {}, "/"), "search"),
      h("span", {}, h("kbd", {}, "Esc"), "clear"),
    ),
  ].filter(Boolean);
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
