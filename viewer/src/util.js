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
  if (p == null) return "?";
  return PRIORITY_NAMES[p] ? `${p} ${PRIORITY_NAMES[p]}` : `${p} mkOverride`;
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

function plural(n, word) {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}
