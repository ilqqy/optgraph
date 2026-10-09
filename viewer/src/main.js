// Loading (embedded, ?src=, file picker, drag and drop) and wiring.

let model = null;
let results = [];
let selectedOption = -1;

const graph = new ModuleGraph($("#stage"), $("#graph"), $("#graph-banner"), $("#graph-tip"), (m) => (m ? showModule(m) : null));
const resultList = new VirtualList($("#results"), (r) => {
  if (r.header) return h("div", { class: "section" }, r.header);
  const o = model.options[r.index];
  const n = o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive;
  return h(
    "div",
    { onclick: () => showOption(r.index) },
    h("span", { class: "path" }, ...highlighted(o.path, r.hits)),
    n ? h("span", { class: "badge", title: "nixpkgs definitions not listed" }, `+${n}`) : null,
    o.error ? h("span", { class: "badge cond", title: o.error }, "error") : null,
  );
});

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

function runSearch() {
  if (!model) return;
  const q = $("#search").value.trim();
  $("#results").hidden = !q;
  $("#lists").hidden = !!q;
  results = q ? searchOptions(model, q) : [];
  resultList.selected = results.findIndex((r) => r.index === selectedOption);
  resultList.setItems(results, "No option matches.");
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
  const q = params.toString();
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

function showOption(oi, reveal = false) {
  selectedOption = oi;
  const row = results.findIndex((r) => r.index === oi);
  resultList.setSelected(row);
  if (reveal && row >= 0) resultList.reveal(row);
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
  resultList.setSelected(-1);
  graph.select(m.id);
  graph.setHighlight(new Set([m.id]));
  $("#legend").classList.remove("focused");
  $("#detail").replaceChildren(...renderModule(model, m, (oi) => showOption(oi, true)));
  $("#inspector").scrollTop = 0;
  markLists();
  setUrl({ module: String(model.modIndex.get(m.id)) });
}

function showStart() {
  $("#detail").replaceChildren(...renderStart(model, (oi) => showOption(oi, true)));
  $("#inspector").scrollTop = 0;
  markLists();
}

function clearSelection() {
  selectedOption = -1;
  resultList.setSelected(-1);
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
  renderMeta(model, showErrorsList);
  $("#lists").replaceChildren(...renderLists(model, (oi) => showOption(oi, true)));
  renderLegend();
  graph.setModel(model);
  $("#search").disabled = false;
  selectedOption = -1;
  showStart();
  runSearch();
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
    showOption(oi, true);
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

$("#search").addEventListener("input", () => requestAnimationFrame(runSearch));
$("#zoom-in").addEventListener("click", () => graph.zoomBy(1.3));
$("#zoom-out").addEventListener("click", () => graph.zoomBy(1 / 1.3));
$("#zoom-fit").addEventListener("click", () => graph.fit());
for (const id of ["#file-input", "#file-input-empty"]) $(id).addEventListener("change", (e) => e.target.files[0] && loadFile(e.target.files[0]));
$("#warnings-close").addEventListener("click", () => ($("#warnings-panel").hidden = true));
// Esc: close the warnings panel, else clear the search text (when typing),
// else clear the selection.
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    const search = $("#search");
    if (!$("#warnings-panel").hidden) $("#warnings-panel").hidden = true;
    else if (document.activeElement === search && search.value) {
      e.preventDefault();
      search.value = "";
      runSearch();
    } else if (model) clearSelection();
  }
  if (e.key === "/" && document.activeElement !== $("#search")) {
    e.preventDefault();
    $("#search").focus();
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

// Startup: data embedded by `optgraph --html`, else ?src=<url> (same origin
// only, enforced by the page's Content-Security-Policy), else on a hosted
// page ./demo.json, else the picker.
(function start() {
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
})();
