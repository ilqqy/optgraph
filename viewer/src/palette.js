// Command palette: `/` or Ctrl+K (⌘K) opens it, fuzzy search over options
// and modules with the matched characters highlighted, results grouped
// (Overrides, Switched off, Errors, Options, Modules). Arrows move, Enter
// opens, Esc closes, a click opens.

const PALETTE_ICONS = { overrides: "override", off: "off", errors: "error", options: "option", modules: "module" };

class Palette {
  constructor(onOption, onModule) {
    this.root = $("#palette");
    this.input = $("#palette-input");
    this.list = $("#palette-list");
    this.count = $("#palette-count");
    this.onOption = onOption;
    this.onModule = onModule;
    this.model = null;
    this.items = [];
    this.active = 0;
    this.input.addEventListener("input", () => this.run());
    this.input.addEventListener("keydown", (e) => this.key(e));
    this.root.querySelector(".palette-scrim").addEventListener("click", () => this.close());
    // Hover follows the pointer only when it moves, not when the list scrolls under it.
    this.list.addEventListener("mousemove", (e) => {
      const row = e.target.closest("[data-n]");
      if (row && Number(row.dataset.n) !== this.active) this.setActive(Number(row.dataset.n), false);
    });
  }

  get isOpen() {
    return !this.root.hidden;
  }

  setModel(model) {
    this.model = model;
    if (this.isOpen) this.run();
  }

  open(query = "") {
    if (!this.model) return;
    this.restore = document.activeElement;
    this.root.hidden = false;
    document.body.classList.add("palette-open");
    this.input.value = query;
    this.run();
    this.input.focus();
  }

  close() {
    if (!this.isOpen) return;
    this.root.hidden = true;
    document.body.classList.remove("palette-open");
    if (this.restore && this.restore !== document.body && document.contains(this.restore)) this.restore.focus();
    else this.input.blur();
  }

  // Types a query (the tour uses this too).
  setQuery(q) {
    this.input.value = q;
    this.run();
  }

  run() {
    const groups = paletteSearch(this.model, this.input.value);
    const model = this.model;
    this.items = [];
    const rows = [];
    for (const g of groups) {
      rows.push(
        h(
          "div",
          { class: `pal-group g-${g.key}`, role: "presentation" },
          icon(PALETTE_ICONS[g.key]),
          g.title,
          h("span", { class: "pal-total" }, g.total > g.items.length ? `${g.items.length} of ${g.total.toLocaleString("en")}` : String(g.total)),
        ),
      );
      for (const it of g.items) {
        const n = this.items.length;
        this.items.push(it);
        rows.push(this.row(model, g.key, it, n));
      }
    }
    if (!rows.length) rows.push(h("div", { class: "pal-empty" }, "Nothing matches ", h("b", {}, this.input.value.trim()), "."));
    this.list.replaceChildren(...rows);
    const total = groups.reduce((s, g) => s + g.total, 0);
    this.count.textContent = this.input.value.trim() ? `${total.toLocaleString("en")} ${total === 1 ? "match" : "matches"}` : "";
    this.setActive(0);
  }

  row(model, key, it, n) {
    let main;
    let meta;
    if (it.kind === "module") {
      const m = model.modules[it.index];
      const label = moduleShortLabel(m);
      const path = moduleLabel(m);
      main = h(
        "span",
        { class: "pal-main" },
        h("span", { class: "pal-label" }, ...(it.where === "label" ? highlighted(label, it.hits) : [label])),
        h("span", { class: "pal-sub" }, ...(it.where === "path" ? highlighted(path, it.hits) : [path])),
      );
      meta = chip(m.origin);
    } else {
      const o = model.options[it.index];
      main = h("span", { class: "pal-main" }, h("span", { class: "pal-path" }, ...highlighted(o.path, it.hits)));
      const a = model.analysis;
      if (key === "overrides") {
        const b = a.info[it.index].beatsParts;
        meta = h("span", { class: "pal-meta" }, h("span", { class: "t-win" }, b.win), " › ", h("span", { class: "t-lose" }, b.lose));
      } else if (key === "off") {
        const off = a.info[it.index].off;
        const m = model.modById.get(o.definitions[off[0]].module);
        meta = h("span", { class: "pal-meta" }, "mkIf false in ", moduleShortLabel(m), off.length > 1 ? ` +${off.length - 1}` : "");
      } else if (key === "errors") meta = h("span", { class: "pal-meta t-err" }, "error");
      else meta = h("span", { class: "pal-meta" }, o.type ?? "");
    }
    return h(
      "div",
      { class: "pal-row", role: "option", id: `pal-${n}`, "data-n": n, onclick: () => this.choose(n) },
      icon(PALETTE_ICONS[key], `pi-${key}`),
      main,
      meta,
      h("kbd", { class: "pal-enter", "aria-hidden": "true" }, "↵"),
    );
  }

  setActive(n, scroll = true) {
    if (!this.items.length) {
      this.active = -1;
      this.input.removeAttribute("aria-activedescendant");
      return;
    }
    this.active = (n + this.items.length) % this.items.length;
    for (const el of this.list.querySelectorAll(".pal-row.active")) {
      el.classList.remove("active");
      el.setAttribute("aria-selected", "false");
    }
    const el = $(`#pal-${this.active}`);
    el.classList.add("active");
    el.setAttribute("aria-selected", "true");
    this.input.setAttribute("aria-activedescendant", el.id);
    if (scroll) {
      // Keep the group header in view for the first row of a group.
      const target = el.previousElementSibling && el.previousElementSibling.classList.contains("pal-group") ? el.previousElementSibling : el;
      const top = target.offsetTop;
      const bottom = el.offsetTop + el.offsetHeight;
      const L = this.list;
      if (top < L.scrollTop) L.scrollTop = top;
      else if (bottom > L.scrollTop + L.clientHeight) L.scrollTop = bottom - L.clientHeight;
    }
  }

  // Row element of an item (the tour points at it).
  rowOf(kind, index) {
    const n = this.items.findIndex((it) => it.kind === kind && it.index === index);
    return n < 0 ? null : $(`#pal-${n}`);
  }

  choose(n) {
    const it = this.items[n];
    if (!it) return;
    this.close();
    if (it.kind === "module") this.onModule(this.model.modules[it.index]);
    else this.onOption(it.index);
  }

  key(e) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      this.setActive(this.active + (e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      this.choose(this.active);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      this.close();
    } else if (e.key === "Tab") e.preventDefault();
  }
}
