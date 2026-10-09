// Loading (embedded, ?src=, file picker, drag and drop) and wiring.

let model = null;
let selectedOption = -1;

const graph = new ModuleGraph($("#graph"), $("#graph-banner"), $("#graph-tip"), (m) => (m ? showModule(m) : null));
const palette = new Palette(
  (oi) => showOption(oi),
  (m) => showModule(m),
);

// Mobile (max-width 760px): the graph fills the screen, the sidebar's lists
// and the inspector are bottom sheets over it, and a tab bar replaces the
// sidebar. The inspector sheet peeks (its title), sits at half height (an
// option or module was picked) or is full; its handle toggles or drags
// between them. The graph keeps what it fits above the sheet.
const mobile = matchMedia("(max-width: 760px)");
const SHEET_PEEK = 132;

function sheetPx(state) {
  const room = $("#stage").clientHeight;
  return state === "full" ? room - 6 : state === "half" ? Math.round(room * 0.6) : SHEET_PEEK;
}

function setSheet(state, px) {
  const insp = $("#inspector");
  insp.dataset.sheet = state;
  if (!mobile.matches) {
    insp.style.height = "";
    graph.reserve.bottom = 44;
    return;
  }
  const height = px ?? sheetPx(state);
  insp.style.height = `${height}px`;
  graph.reserve.bottom = Math.min(height, sheetPx("half")) + 8;
}

function setTab(key) {
  for (const b of document.querySelectorAll("#tabs button")) b.classList.toggle("on", b.dataset.tab === key);
}

function openList(key) {
  const sb = $("#sidebar");
  if (document.body.classList.contains("m-lists") && sb.dataset.tab === key) return closeLists();
  sb.dataset.tab = key;
  $("#lists-title").textContent = { overrides: "Overrides", off: "Switched off", errors: "Errors" }[key];
  document.body.classList.add("m-lists");
  setTab(key);
}

function closeLists() {
  document.body.classList.remove("m-lists");
  setTab("graph");
}

$("#tabs").addEventListener("click", (e) => {
  const b = e.target.closest("button[data-tab]");
  if (!b || !model) return;
  const key = b.dataset.tab;
  if (key === "search") palette.open();
  else if (key === "graph") {
    closeLists();
    setSheet("peek");
    graph.fit();
  } else openList(key);
});
$("#lists-close").addEventListener("click", closeLists);
mobile.addEventListener("change", () => {
  closeLists();
  setSheet($("#inspector").dataset.sheet);
});
new ResizeObserver(() => mobile.matches && setSheet($("#inspector").dataset.sheet)).observe($("#stage"));

// The sheet handle: a tap toggles half and full, a drag resizes and snaps.
(() => {
  const handle = $("#sheet-handle");
  let drag = null;
  handle.addEventListener("pointerdown", (e) => {
    drag = { y: e.clientY, h: $("#inspector").offsetHeight, moved: false };
    handle.setPointerCapture(e.pointerId);
    $("#inspector").classList.add("dragging");
  });
  handle.addEventListener("pointermove", (e) => {
    if (!drag) return;
    const dy = e.clientY - drag.y;
    if (Math.abs(dy) > 5) drag.moved = true;
    if (drag.moved) $("#inspector").style.height = `${Math.max(SHEET_PEEK, Math.min(sheetPx("full"), drag.h - dy))}px`;
  });
  const end = () => {
    if (!drag) return;
    $("#inspector").classList.remove("dragging");
    const cur = $("#inspector").dataset.sheet;
    if (!drag.moved) setSheet(cur === "full" ? "half" : cur === "half" ? "full" : "half");
    else {
      const h = $("#inspector").offsetHeight;
      const near = ["peek", "half", "full"].map((s) => [s, Math.abs(sheetPx(s) - h)]).sort((a, b) => a[1] - b[1])[0][0];
      setSheet(near);
    }
    drag = null;
  };
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
})();

// Legend: origins (mark shape and colour), and while an option is selected
// the statuses of its defining modules.
function renderLegend() {
  const kinds = [
    ["user", "user"],
    ["user-inline", "inline"],
    ["input", "input"],
    ["nixpkgs", "nixpkgs"],
    ["unknown", "unknown"],
  ];
  $("#legend").replaceChildren(
    h(
      "div",
      { class: "legend-origins" },
      ...kinds.map(([cls, label]) => h("span", {}, originDot(cls === "input" ? "input:x" : cls), label)),
      h("span", {}, h("span", { class: "dash-sample", "aria-hidden": "true" }), "disabled"),
    ),
    h(
      "div",
      { class: "legend-status" },
      h("span", {}, h("span", { class: "ring win" }), "winner ✓"),
      h("span", {}, h("span", { class: "ring lose" }), "lost"),
      h("span", {}, h("span", { class: "ring off" }), "mkIf false"),
      h("span", { class: "muted" }, "import path"),
    ),
  );
}

// Sidebar rows of the selected option are marked.
function markLists() {
  for (const li of $("#lists").querySelectorAll("li[data-oi]")) li.classList.toggle("sel", Number(li.dataset.oi) === selectedOption);
}

function showErrorsList() {
  const sec = $("#lists").querySelector('[data-list="errors"]');
  if (sec) sec.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

// The selection is mirrored in the URL (?opt= / ?module=), so the address
// bar is always a deep link. replaceState can fail on file:// pages.
function setUrl(sel) {
  const params = new URLSearchParams(location.search);
  params.delete("opt");
  params.delete("module");
  for (const [k, v] of Object.entries(sel)) params.set(k, v);
  const q = params.toString().replace(/=(?=&|$)/g, ""); // ?tour, not ?tour=
  try {
    history.replaceState(null, "", location.pathname + (q ? `?${q}` : "") + location.hash);
  } catch (e) {
    // keep the old URL
  }
}

// Defining module -> its best definition of the option: win > lose > off.
function focusOf(o) {
  const winners = new Set(o.winners);
  const map = new Map();
  o.definitions.forEach((d, i) => {
    if (d.module == null) return;
    const status = !d.active ? "off" : winners.has(i) ? "win" : d.priority == null || o.highestPrio == null ? "unknown" : "lose";
    const cur = map.get(d.module);
    if (!cur || RING_RANK[status] > RING_RANK[cur.status]) map.set(d.module, { status, priority: d.priority, condition: d.condition });
  });
  return map;
}

function showOption(oi) {
  selectedOption = oi;
  if (mobile.matches) {
    closeLists();
    setSheet("half");
  }
  graph.select(null);
  graph.setFocus(focusOf(model.options[oi]));
  $("#legend").classList.add("focused");
  $("#detail").replaceChildren(...renderOption(model, oi, showModule, () => copyText(location.href)));
  $("#inspector").scrollTop = 0;
  markLists();
  setUrl({ opt: model.options[oi].path });
}

function showModule(m) {
  selectedOption = -1;
  if (mobile.matches) {
    closeLists();
    setSheet("half");
  }
  graph.select(m.id);
  graph.setHighlight(new Set([m.id]));
  $("#legend").classList.remove("focused");
  $("#detail").replaceChildren(...renderModule(model, m, showOption));
  $("#inspector").scrollTop = 0;
  markLists();
  setUrl({ module: String(model.modIndex.get(m.id)) });
}

function showStart() {
  $("#detail").replaceChildren(...renderStart(model, showOption));
  $("#inspector").scrollTop = 0;
  markLists();
}

function clearSelection() {
  selectedOption = -1;
  setSheet("peek");
  graph.select(null);
  graph.setFocus(null);
  $("#legend").classList.remove("focused");
  graph.fit();
  showStart();
  setUrl({});
}

function load(doc, source) {
  try {
    model = buildModel(doc);
  } catch (e) {
    showError(`${source}: ${e.message}`);
    return;
  }
  $("#empty").classList.add("hidden");
  document.title = `optgraph: ${model.meta.host ?? "graph"}`;
  renderMeta(model, () => (mobile.matches ? openList("errors") : showErrorsList()));
  const a = model.analysis;
  for (const [key, n] of [["overrides", a.overrides.length], ["off", a.switchedOff.length], ["errors", a.errors.length]]) {
    $(`#tabs [data-tab="${key}"] .tab-count`).textContent = n ? n.toLocaleString("en") : "";
  }
  $("#lists").replaceChildren(...renderLists(model, showOption));
  renderLegend();
  setSheet("peek");
  graph.setModel(model);
  palette.setModel(model);
  $("#search-trigger").disabled = false;
  selectedOption = -1;
  showStart();
  applyDeepLink();
  if (new URLSearchParams(location.search).has("tour")) startTour();
}

// ?opt=<option path> selects an option; ?module=<index or id> a module.
// Works the same for embedded data and ?src=.
function applyDeepLink() {
  const params = new URLSearchParams(location.search);
  if (params.has("warnings")) $("#warnings-panel").hidden = false;
  const optPath = params.get("opt");
  const modRef = params.get("module");
  if (optPath != null) {
    const oi = model.options.findIndex((o) => o.path === optPath);
    if (oi < 0) {
      $("#detail").replaceChildren(h("p", { class: "error" }, `No option ${optPath} in this graph.`));
      return;
    }
    showOption(oi);
  } else if (modRef != null) {
    const m = /^\d+$/.test(modRef) ? model.modules[Number(modRef)] : model.modById.get(modRef);
    if (!m) {
      $("#detail").replaceChildren(h("p", { class: "error" }, `No module ${modRef} in this graph.`));
      return;
    }
    showModule(m);
  }
}

function showError(msg) {
  $("#empty").classList.remove("hidden");
  const el = $("#load-error");
  el.hidden = false;
  el.textContent = msg;
}

function loadText(text, source) {
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    showError(`${source}: not valid JSON (${e.message})`);
    return;
  }
  load(doc, source);
}

function loadFile(file) {
  file.text().then((t) => loadText(t, file.name), (e) => showError(`${file.name}: ${e.message}`));
}

$("#search-trigger").addEventListener("click", () => palette.open());
$("#zoom-in").addEventListener("click", () => graph.zoomBy(1.3));
$("#zoom-out").addEventListener("click", () => graph.zoomBy(1 / 1.3));
$("#zoom-fit").addEventListener("click", () => graph.fit());
for (const id of ["#file-input", "#file-input-empty"]) $(id).addEventListener("change", (e) => e.target.files[0] && loadFile(e.target.files[0]));
$("#warnings-close").addEventListener("click", () => ($("#warnings-panel").hidden = true));

// Keys: `/` and Ctrl+K (⌘K) open the palette (which handles its own keys);
// Esc closes the warnings panel, else clears the selection.
const typing = (el) => el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
document.addEventListener("keydown", (e) => {
  if (palette.isOpen) return;
  if ((e.key === "k" || e.key === "K") && (e.ctrlKey || e.metaKey) && !e.altKey) {
    e.preventDefault();
    palette.open();
  } else if (e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey && !typing(document.activeElement)) {
    e.preventDefault();
    palette.open();
  } else if (e.key === "Escape") {
    if (!$("#warnings-panel").hidden) $("#warnings-panel").hidden = true;
    else if (document.body.classList.contains("m-lists")) closeLists();
    else if (model) clearSelection();
  }
});
document.addEventListener("dragover", (e) => {
  e.preventDefault();
  document.body.classList.add("dragging");
});
document.addEventListener("dragleave", (e) => {
  if (!e.relatedTarget) document.body.classList.remove("dragging");
});
document.addEventListener("drop", (e) => {
  e.preventDefault();
  document.body.classList.remove("dragging");
  const f = e.dataTransfer.files[0];
  if (f) loadFile(f);
});

for (const el of document.querySelectorAll(".ico-slot")) el.replaceWith(icon(el.dataset.icon));

// The embedded fonts (OPTGRAPH_FONTS, from viewer/build.sh) are made from
// their bytes with the FontFace API: nothing is fetched, so the CSP needs no
// font-src. The graph measures its labels, so data loads after the fonts
// (or after 1.5 s, falling back to the system fonts).
function loadFonts() {
  if (typeof OPTGRAPH_FONTS === "undefined" || !window.FontFace) return Promise.resolve();
  const faces = OPTGRAPH_FONTS.map(([family, b64]) => {
    try {
      const face = new FontFace(family, Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)), { weight: "400 700" });
      document.fonts.add(face);
      return face.load().catch(() => null);
    } catch (e) {
      return null;
    }
  });
  return Promise.race([Promise.all(faces), new Promise((resolve) => setTimeout(resolve, 1500))]);
}

// Startup: data embedded by `optgraph --html`, else ?src=<url> (same origin
// only, enforced by the page's Content-Security-Policy), else on a hosted
// page ./demo.json, else the picker.
loadFonts().then(function start() {
  const embedded = $("#optgraph-data").textContent.trim();
  const placeholder = "/*OPTGRAPH_" + "DATA*/null"; // split so the embed step can't match it here
  if (embedded && embedded !== placeholder) {
    loadText(embedded, "embedded graph");
    return;
  }
  const src = new URLSearchParams(location.search).get("src");
  if (src) {
    fetch(src)
      .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`HTTP ${r.status}`))))
      .then((t) => loadText(t, src), (e) => showError(`${src}: ${e.message}`));
    return;
  }
  // Hosted page (GitHub Pages): show ./demo.json if the site has one. Same
  // origin only (CSP connect-src 'self'); file:// pages skip this.
  if (location.protocol === "http:" || location.protocol === "https:") {
    fetch("demo.json")
      .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`HTTP ${r.status}`))))
      .then((t) => loadText(t, "demo.json"), () => {}); // no demo: keep the drop zone
  }
});
