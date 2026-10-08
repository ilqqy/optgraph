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

function renderOption(model, oi, onModule) {
  const o = model.options[oi];
  const winners = new Set(o.winners);
  const omittedN = o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive;
  const head = [
    h("h2", {}, o.path),
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
