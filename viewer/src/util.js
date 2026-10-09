// DOM and formatting helpers.

const $ = (sel) => document.querySelector(sel);

// h("div", { class: "x", onclick: f }, child, ...): children are nodes or text.
function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === "class") el.className = v;
    else if (k === "text") el.textContent = v;
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? "" : String(v));
  }
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : String(c));
  }
  return el;
}

const STORE_RE = /^\/nix\/store\/[0-9a-z]{32}-[^/]+/;

// /nix/store/<hash>-source/nixos/modules/x.nix -> …/nixos/modules/x.nix
function shortPath(p) {
  if (!p) return "?";
  const m = STORE_RE.exec(p);
  if (!m) return p;
  const rest = p.slice(m[0].length);
  return rest ? "…" + rest : m[0].slice(44);
}

// The last n segments: …/networking/firewall.nix
function tailPath(p, n = 2) {
  const parts = shortPath(p).split("/");
  return parts.length > n + 1 ? "…/" + parts.slice(-n).join("/") : shortPath(p);
}

function baseName(p) {
  const s = shortPath(p);
  const i = s.lastIndexOf("/");
  return i >= 0 ? s.slice(i + 1) : s;
}

const PRIORITY_NAMES = {
  10: "mkVMOverride",
  50: "mkForce",
  60: "mkImageMediaOverride",
  100: "normal",
  1000: "mkDefault",
  1500: "default",
};

function priorityLabel(p) {
  if (p == null) return "? unknown";
  return PRIORITY_NAMES[p] ? `${p} ${PRIORITY_NAMES[p]}` : `${p} mkOverride`;
}

// Name only: "mkForce", "normal", "mkOverride 75".
function prioName(p) {
  if (p == null) return "unknown";
  return PRIORITY_NAMES[p] || `mkOverride ${p}`;
}

// Clipboard API where allowed (https, localhost), else the selection fallback
// (file:// pages). Resolves to whether it worked.
function copyText(text) {
  const fallback = () => {
    const ta = h("textarea", { style: "position:fixed;opacity:0" });
    ta.value = text;
    document.body.append(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (e) {
      ok = false;
    }
    ta.remove();
    return ok;
  };
  if (navigator.clipboard && window.isSecureContext) {
    return navigator.clipboard.writeText(text).then(() => true, () => fallback());
  }
  return Promise.resolve(fallback());
}

// "input:foo/bar" -> "input"; used for colours and CSS classes.
function originClass(origin) {
  if (!origin) return "unknown";
  if (origin.startsWith("input:")) return "input";
  return ["user", "user-inline", "nixpkgs", "unknown"].includes(origin) ? origin : "unknown";
}

function chip(origin) {
  return h("span", { class: `chip o-${originClass(origin)}`, title: "module origin" }, origin || "unknown");
}

// plural(2, "module") -> "2 modules"; plural(2, "is", "are") -> "2 are".
function plural(n, word, many) {
  return `${n.toLocaleString("en")} ${n === 1 ? word : many ?? `${word}s`}`;
}

// 16x16 stroke icons (currentColor).
const ICONS = {
  search: '<circle cx="7" cy="7" r="4.5"/><path d="m10.5 10.5 3.5 3.5"/>',
  override: '<path d="M8 13.5v-11M4 6.5l4-4 4 4"/>',
  off: '<path d="M8 2.5v5"/><path d="M4.9 4.4a5 5 0 1 0 6.2 0"/>',
  error: '<path d="M8 2.5 14 13H2z"/><path d="M8 6.5v3M8 11.4v.1"/>',
  option: '<path d="M6 2.5c-1.4 0-2 .6-2 2v1.6c0 .9-.5 1.5-1.5 1.9 1 .4 1.5 1 1.5 1.9v1.6c0 1.4.6 2 2 2M10 2.5c1.4 0 2 .6 2 2v1.6c0 .9.5 1.5 1.5 1.9-1 .4-1.5 1-1.5 1.9v1.6c0 1.4-.6 2-2 2"/>',
  module: '<path d="M8 1.8 13.5 5v6L8 14.2 2.5 11V5z"/><path d="M2.5 5 8 8.2 13.5 5M8 8.2v6"/>',
  copy: '<rect x="5.5" y="5.5" width="8" height="8" rx="2"/><path d="M10.5 5.5V4A1.5 1.5 0 0 0 9 2.5H4A1.5 1.5 0 0 0 2.5 4v5A1.5 1.5 0 0 0 4 10.5h1.5"/>',
  link: '<path d="M6.5 9.5l3-3"/><path d="M7 4.5l1-1a2.8 2.8 0 0 1 4 4l-1 1M9 11.5l-1 1a2.8 2.8 0 0 1-4-4l1-1"/>',
  check: '<path d="m3.5 8.5 3 3 6-7"/>',
  x: '<path d="m4.5 4.5 7 7M11.5 4.5l-7 7"/>',
  open: '<path d="M8 10.5v-8M5 5.5l3-3 3 3"/><path d="M2.5 10v2A1.5 1.5 0 0 0 4 13.5h8a1.5 1.5 0 0 0 1.5-1.5v-2"/>',
  plus: '<path d="M8 3.5v9M3.5 8h9"/>',
  minus: '<path d="M3.5 8h9"/>',
  fit: '<path d="M2.5 6V3.5a1 1 0 0 1 1-1H6M10 2.5h2.5a1 1 0 0 1 1 1V6M13.5 10v2.5a1 1 0 0 1-1 1H10M6 13.5H3.5a1 1 0 0 1-1-1V10"/>',
  graph: '<circle cx="4" cy="8" r="2"/><circle cx="12" cy="3.8" r="2"/><circle cx="12" cy="12.2" r="2"/><path d="M6 7.1l4-2.2M6 8.9l4 2.2"/>',
  diamond: '<path d="M8 2.5 13.5 8 8 13.5 2.5 8z"/>',
  chevron: '<path d="m4.5 6.5 3.5 3.5 3.5-3.5"/>',
  merge: '<circle cx="4" cy="3.5" r="1.5"/><circle cx="4" cy="12.5" r="1.5"/><circle cx="12" cy="8" r="1.5"/><path d="M4 5v6M4 5c0 2 2 3 6.5 3"/>',
};

function icon(name, cls) {
  const s = h("span", { class: `ico${cls ? " " + cls : ""}`, "aria-hidden": "true" });
  s.innerHTML = `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`;
  return s;
}
