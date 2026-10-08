// Loading (embedded, ?src=, file picker, drag and drop) and wiring.

let model = null;
let results = [];
let selectedOption = -1;

const graph = new ModuleGraph($("#graph"), $("#graph-banner"), (m) => (m ? showModule(m) : null));
const resultList = new VirtualList($("#results"), (r, i) => {
  const o = model.options[r.index];
  const n = o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive;
  return h(
    "div",
    { onclick: () => showOption(r.index, i) },
    h("span", { class: "path" }, ...highlighted(o.path, r.hits)),
    n ? h("span", { class: "badge", title: "nixpkgs definitions not listed" }, `+${n}`) : null,
    o.error ? h("span", { class: "badge cond", title: o.error }, "error") : null,
  );
});

function renderLegend() {
  const kinds = [
    ["user", "user"],
    ["user-inline", "user-inline"],
    ["input", "input:*"],
    ["nixpkgs", "nixpkgs"],
    ["unknown", "unknown"],
  ];
  $("#legend").replaceChildren(
    ...kinds.map(([cls, label]) => h("span", {}, h("span", { class: "dot", style: `background:var(--o-${cls})` }), label)),
    h("span", { class: "muted" }, "dashed: disabled"),
  );
}

function runSearch() {
  if (!model) return;
  results = searchOptions(model, $("#search").value);
  resultList.selected = results.findIndex((r) => r.index === selectedOption);
  resultList.setItems(results, "No option matches.");
}

function showOption(oi, row) {
  selectedOption = oi;
  if (row != null) resultList.setSelected(row);
  $("#detail").replaceChildren(...renderOption(model, oi, showModule));
  const ids = new Set(model.options[oi].definitions.map((d) => d.module).filter((x) => x != null));
  graph.setHighlight(ids);
}

function showModule(m) {
  graph.select(m.id);
  graph.setHighlight(new Set([m.id]));
  $("#detail").replaceChildren(
    ...renderModule(model, m, (oi) => {
      const row = results.findIndex((r) => r.index === oi);
      showOption(oi, row >= 0 ? row : null);
    }),
  );
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
  renderMeta(model);
  renderLegend();
  graph.setModel(model);
  $("#search").disabled = false;
  selectedOption = -1;
  $("#detail").replaceChildren(h("p", { class: "muted" }, "Select an option to see who set it, at what priority, and why the others lost. Click a module in the graph to see what it sets."));
  runSearch();
  $("#search").focus();
  applyDeepLink();
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
    const row = results.findIndex((r) => r.index === oi);
    showOption(oi, row >= 0 ? row : null);
    if (row >= 0) $("#results").scrollTop = Math.max(0, row * resultList.rowHeight - 60);
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
$("#file-input").addEventListener("change", (e) => e.target.files[0] && loadFile(e.target.files[0]));
$("#warnings-btn").addEventListener("click", () => ($("#warnings-panel").hidden = !$("#warnings-panel").hidden));
$("#warnings-close").addEventListener("click", () => ($("#warnings-panel").hidden = true));
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") $("#warnings-panel").hidden = true;
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
