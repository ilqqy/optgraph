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
  // The note (shown on touch screens, a title elsewhere) says why it is listed.
  const row = (oi, note, data) =>
    h(
      "li",
      { title: note, "data-oi": oi, ...data },
      h("button", { type: "button", onclick: () => onOption(oi) }, h("span", { class: "path" }, model.options[oi].path), h("span", { class: "note" }, note)),
    );
  return [
    section(
      "overrides",
      "Overrides",
      "override",
      a.overrides,
      (oi) => {
        const b = a.info[oi].beatsParts;
        return row(oi, `${b.win} in ${b.winWho} beats ${b.lose} in ${b.loseWho}`);
      },
      "No listed definition lost.",
    ),
    section(
      "off",
      "Switched off",
      "off",
      a.switchedOff,
      ({ oi, di }) => {
        const m = model.modById.get(model.options[oi].definitions[di].module);
        return row(oi, `mkIf false in ${moduleShortLabel(m)}`, { "data-di": di });
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

// Copy button: copies text(), shows a check for a moment.
function copyButton(text, label, cls) {
  const btn = h("button", { type: "button", class: `copy-btn ${cls || ""}`, title: label, "aria-label": label }, icon("copy"));
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    Promise.resolve(copyText(text())).then((ok) => {
      btn.replaceChildren(icon(ok ? "check" : "x"));
      btn.classList.toggle("done", ok);
      setTimeout(() => {
        btn.replaceChildren(icon("copy"));
        btn.classList.remove("done");
      }, 1400);
    });
  });
  return btn;
}

const STATUS_BADGE = {
  win: ["check", "Winner"],
  lose: ["x", "Lost"],
  off: ["off", "Switched off"],
  unknown: ["error", "Unknown"],
};

// Path with its last segment emphasised: networking.firewall.<b>enable</b>.
function pathTitle(path) {
  const i = path.lastIndexOf(".");
  return i < 0 ? [h("b", {}, path)] : [h("span", { class: "pre" }, path.slice(0, i + 1)), h("b", {}, path.slice(i + 1))];
}

// Option detail: header, verdict, the priority ladder as stacked cards.
function renderOption(model, oi, onModule, onCopyLink) {
  const o = model.options[oi];
  const rows = ladder(model, o);
  const omittedN = o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive;
  const multi = o.winners.length > 1;
  const losers = rows.filter((r) => r.status === "lose").length;
  const offs = rows.filter((r) => r.status === "off").length;
  const winner = o.winners.length === 1 ? o.definitions[o.winners[0]] : null;

  const linkBtn = h("button", { type: "button", class: "btn small copy-link", title: "Copy a link to this option" }, icon("link"), h("span", {}, "Copy link"));
  linkBtn.addEventListener("click", () =>
    onCopyLink().then((ok) => {
      linkBtn.lastChild.textContent = ok ? "Copied" : "Copy failed";
      setTimeout(() => (linkBtn.lastChild.textContent = "Copy link"), 1500);
    }),
  );

  let verdict;
  if (o.highestPrio == null) verdict = h("p", { class: "verdict" }, "No known winner: ", o.error ? "an active definition could not be read." : "nothing active defines it.");
  else if (multi) verdict = null;
  else if (winner.kind === "default")
    verdict = h(
      "p",
      { class: "verdict" },
      "The ",
      h("b", {}, "option default"),
      " applies",
      offs ? `: ${plural(offs, "definition is", "definitions are")} switched off.` : ": nothing stronger is set.",
    );
  else
    verdict = h(
      "p",
      { class: "verdict" },
      h("b", {}, defWho(model, winner)),
      " wins with ",
      h("code", { class: "t-win" }, `${prioName(winner.priority)} ${winner.priority}`),
      losers ? `, over ${plural(losers, "weaker definition")}.` : ".",
    );

  const merged = multi
    ? h(
        "div",
        { class: "merged" },
        icon("merge"),
        h(
          "div",
          {},
          h("div", { class: "merged-title" }, `Merged from ${plural(new Set(o.winners.map((i) => o.definitions[i].module ?? "default")).size, "module")}`),
          h(
            "div",
            { class: "merged-sub" },
            `Every active definition at priority ${o.highestPrio} (${prioName(o.highestPrio)}) is a winner; the values are merged.`,
            o.omitted.nixpkgsActive ? ` Plus ${plural(o.omitted.nixpkgsActive, "nixpkgs definition")} not listed.` : "",
          ),
        ),
        h("div", { class: "merged-dots", "aria-hidden": "true" }, ...o.winners.slice(0, 8).map((i) => originDot(defOrigin(model, o.definitions[i])))),
      )
    : null;

  const cards = rows.map((r, k) => {
    const { d } = r;
    const mod = d.module != null ? model.modById.get(d.module) : null;
    const file = defFile(model, d);
    const isDefault = d.kind === "default";
    const [ico, label] = STATUS_BADGE[r.status];
    const who = isDefault
      ? h("span", { class: "mpill def", title: file ? shortPath(file) : "option default" }, icon("diamond"), "option default")
      : mod
        ? h("button", { type: "button", class: "mpill", title: `${moduleLabel(mod)}: show what this module sets`, onclick: () => onModule(mod) }, originDot(mod.origin), moduleShortLabel(mod))
        : h("span", { class: "mpill" }, baseName(file));
    const value =
      d.valuePreview == null
        ? h("div", { class: "novalue" }, !d.active ? "not evaluated: switched off" : d.priority == null ? "value threw (see the error)" : "no preview")
        : h("div", { class: `code${d.valuePreview === "<redacted>" ? " redacted" : ""}` }, h("pre", {}, d.valuePreview), copyButton(() => d.valuePreview, "Copy value"));
    return h(
      "li",
      { class: `card s-${r.status}${isDefault ? " is-default" : ""}`, style: `--i:${Math.min(k, 8)}` },
      h(
        "div",
        { class: "prio", title: "override priority: lower wins" },
        h("span", { class: "num" }, d.priority == null ? "—" : String(d.priority)),
        h("span", { class: "nm" }, d.priority == null ? (d.active ? "unknown" : "off") : prioName(d.priority)),
      ),
      h(
        "div",
        { class: "body" },
        h(
          "div",
          { class: "head" },
          who,
          isDefault ? null : chip(mod ? mod.origin : "unknown"),
          h("span", { class: `status st-${r.status}` }, icon(ico), label),
        ),
        !isDefault && mod && file && file !== mod.file ? h("div", { class: "where" }, "file ", shortPath(file)) : null,
        isDefault && file ? h("div", { class: "where", title: shortPath(file) }, "declared in ", tailPath(file)) : null,
        value,
        h("div", { class: "reason" }, r.reason),
      ),
    );
  });

  const facts = [
    ["type", h("code", {}, o.type ?? "?")],
    o.declaredIn.length ? ["declared in", h("div", {}, ...o.declaredIn.map((f) => h("code", { title: f }, shortPath(f))))] : null,
    omittedN
      ? [
          "not listed",
          h("span", {}, `${plural(omittedN, "nixpkgs definition")} (${o.omitted.nixpkgsActive} active, ${o.omitted.nixpkgsInactive} inactive); optgraph --include-all-definitions lists them`),
        ]
      : null,
  ].filter(Boolean);

  return [
    h("div", { class: "opt-head" }, h("div", { class: "overline" }, "Option ", h("span", { class: "type-chip" }, o.type ?? "?")), linkBtn),
    h("h1", { class: "opt-path" }, ...pathTitle(o.path)),
    verdict,
    o.error ? h("div", { class: "opt-error" }, icon("error"), h("span", {}, o.error)) : null,
    merged,
    h(
      "div",
      { class: "ladder-head" },
      h("b", {}, "Priority ladder"),
      h("span", {}, plural(rows.length, "definition")),
      omittedN ? h("span", { class: "badge", title: "nixpkgs definitions counted, not listed" }, `+${omittedN} nixpkgs`) : null,
      h("span", { class: "hint" }, "lower number wins"),
    ),
    rows.length ? h("ol", { class: `ladder${multi ? " multi" : ""}` }, ...cards) : h("p", { class: "muted" }, "No definitions listed."),
    h("dl", { class: "facts" }, ...facts.flatMap(([k, v]) => [h("dt", {}, k), h("dd", {}, v)])),
  ].filter(Boolean);
}

// This module's best status for option oi (win > lose > off) and its priority.
function moduleStatus(model, m, oi) {
  const o = model.options[oi];
  const winners = new Set(o.winners);
  let best = null;
  o.definitions.forEach((d, i) => {
    if (d.module !== m.id) return;
    const status = !d.active ? "off" : winners.has(i) ? "win" : o.highestPrio == null ? "unknown" : "lose";
    const rank = { win: 3, lose: 2, unknown: 1, off: 0 }[status];
    if (!best || rank > best.rank) best = { status, rank, priority: d.priority };
  });
  return best;
}

// Module detail: what it is, where it sits, what it sets and whether it wins.
function renderModule(model, m, onOption) {
  const opts = model.modOptions.get(m.id) || [];
  const importers = model.modules.filter((x) => x.imports.includes(m.id)).length;
  const list = opts.length ? h("div", { class: "vlist mod-options", style: `height:${Math.min(360, opts.length * 30 + 10)}px` }) : h("p", { class: "muted" }, "This module sets no listed option.");
  const facts = [
    ["origin", chip(m.origin)],
    ["file", h("code", {}, m.file)],
    m.id !== m.file ? ["id", h("code", {}, m.id)] : null,
    m.position ? ["position", h("code", {}, m.position)] : null,
  ].filter(Boolean);
  const head = [
    h("div", { class: "opt-head" }, h("div", { class: "overline" }, "Module ", m.disabled ? h("span", { class: "type-chip" }, "disabled") : null)),
    h("h1", { class: "mod-title" }, originDot(m.origin), moduleShortLabel(m)),
    h("p", { class: "mod-path" }, moduleLabel(m)),
    h(
      "div",
      { class: "stats three" },
      h("div", { class: "stat" }, h("b", {}, String(opts.length)), h("span", {}, "options set")),
      h("div", { class: "stat" }, h("b", {}, String(m.imports.length)), h("span", {}, "imports")),
      h("div", { class: "stat" }, h("b", {}, String(importers)), h("span", {}, "importers")),
    ),
    h("dl", { class: "facts top" }, ...facts.flatMap(([k, v]) => [h("dt", {}, k), h("dd", {}, v)])),
    h("div", { class: "ladder-head" }, h("b", {}, "Options it sets"), h("span", {}, plural(opts.length, "listed option"))),
    list,
  ];
  // The list is attached after the caller inserts `head`.
  if (opts.length)
    queueMicrotask(() => {
      const vl = new VirtualList(
        list,
        (oi) => {
          const st = moduleStatus(model, m, oi);
          return h(
            "div",
            { onclick: () => onOption(oi), title: st ? `${STATUS_BADGE[st.status][1]} at ${priorityLabel(st.priority)}` : null },
            h("span", { class: `mini st-${st ? st.status : "unknown"}` }, icon(STATUS_BADGE[st ? st.status : "unknown"][0])),
            h("span", { class: "path" }, model.options[oi].path),
            st && st.priority != null ? h("span", { class: "mini-prio" }, String(st.priority)) : null,
          );
        },
        30,
      );
      vl.setItems(opts);
    });
  return head;
}
