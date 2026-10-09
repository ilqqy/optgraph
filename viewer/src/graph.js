// Module import graph on a <canvas>: layered left-to-right layout
// (layout.js), modules as pills, curved edges, own pan/zoom and an eased
// camera. Canvas instead of SVG keeps a few hundred pills responsive.

const GRAPH_LIMIT = 300; // above this many modules only non-nixpkgs ones are drawn

// Rings around the modules that define the selected option.
const RING_RANK = { win: 3, lose: 2, off: 1 };

const PILL_H = 28;
const PILL_PAD = 12;
const PILL_FONT = '500 12px "Geist", ui-sans-serif, system-ui, sans-serif';
const CHIP_FONT = '600 11px "Geist Mono", ui-monospace, monospace';
const CHIP_H = 20;
const CAMERA_MS = 450;
const FADE_MS = 200;

// The clock of camera moves and fades: performance.now(), unless the tour
// drives it (then they are a function of the tour's time).
const ui = {
  now: () => performance.now(),
  reducedMotion: () => matchMedia("(prefers-reduced-motion: reduce)").matches,
};

const easeCamera = (p) => (p < 0.5 ? 4 * p * p * p : 1 - (-2 * p + 2) ** 3 / 2);

class ModuleGraph {
  constructor(stage, canvas, banner, tip, onSelect) {
    this.stage = stage;
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.banner = banner;
    this.tip = tip;
    this.onSelect = onSelect;
    this.t = { x: 0, y: 0, k: 1 };
    this.cam = null; // { from, to, at, dur }
    this.nodes = [];
    this.links = [];
    this.byId = new Map();
    this.parents = new Map();
    this.highlight = null; // Set of module ids (a selected module), or null
    this.lit = null; // the selected module and its neighbours
    this.focus = null; // Map module id -> { status, priority, condition } (a selected option), or null
    this.path = null; // Set of link indices leading to the lit modules
    this.via = null; // Set of module ids on those paths (half lit)
    this.fadeAt = -1e9;
    this.fade = 1;
    this.selected = null;
    this.hover = null;
    this.colors = {};
    this.pending = false;
    this.reserve = { top: 0, bottom: 44 }; // pixels kept free when fitting (legend, tour captions, mobile sheet)
    this.readColors();
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      this.readColors();
      this.requestDraw();
    });
    new ResizeObserver(() => this.resize()).observe(canvas);
    this.bindPointer();
  }

  readColors() {
    const s = getComputedStyle(document.documentElement);
    const v = (n) => s.getPropertyValue(n).trim();
    this.colors = {
      user: v("--o-user"),
      "user-inline": v("--o-user-inline"),
      input: v("--o-input"),
      nixpkgs: v("--o-nixpkgs"),
      unknown: v("--o-unknown"),
      edge: v("--edge"),
      text: v("--text"),
      text2: v("--text-2"),
      text3: v("--text-3"),
      accent: v("--accent"),
      s1: v("--s1"),
      s2: v("--s2"),
      s3: v("--s3"),
      line2: v("--line-2"),
      win: v("--win"),
      onWin: v("--on-win"),
      lose: v("--lose"),
      off: v("--off"),
    };
  }

  setModel(model) {
    this.model = model;
    const all = model.modules;
    const tooMany = all.length > GRAPH_LIMIT;
    const shown = tooMany ? all.filter(isUserish) : all;
    this.banner.hidden = !tooMany;
    if (tooMany) {
      this.banner.textContent = `${all.length.toLocaleString("en")} modules are too many to draw: showing the ${shown.length.toLocaleString("en")} non-nixpkgs modules only. Search and option details cover everything.`;
    }
    const ctx = this.ctx;
    ctx.font = PILL_FONT;
    const byId = new Map();
    this.nodes = shown.map((m, i) => {
      const label = moduleShortLabel(m);
      const n = { i, id: m.id, m, label, cls: originClass(m.origin), w: Math.ceil(ctx.measureText(label).width) + 2 * PILL_PAD + 14, h: PILL_H, x: 0, y: 0, chipW: 0 };
      byId.set(m.id, n);
      return n;
    });
    this.links = [];
    for (const m of shown) {
      for (const t of m.imports) {
        if (byId.has(t)) this.links.push({ source: byId.get(m.id), target: byId.get(t), dashed: byId.get(t).m.disabled });
      }
    }
    const lay = layeredLayout(
      this.nodes.map((n) => ({ w: n.w, h: n.h })),
      this.links.map((l) => [l.source.i, l.target.i]),
    );
    this.nodes.forEach((n, i) => {
      n.x = lay.x[i] - n.w / 2; // left edge
      n.y = lay.y[i]; // centre line
    });
    this.links.forEach((l, k) => (l.pts = lay.paths[k]));
    this.byId = byId;
    // Importers of each module, for the import paths to what is selected.
    this.parents = new Map(this.nodes.map((n) => [n.id, []]));
    this.links.forEach((l, k) => this.parents.get(l.target.id).push(k));
    this.highlight = this.lit = this.focus = this.path = this.via = null;
    this.selected = null;
    this.setHover(null);
    this.fit(false);
  }

  bounds(nodes) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const n of nodes) {
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y - n.h / 2);
      x1 = Math.max(x1, n.x + n.w + (n.chipW ? n.chipW + 8 : 0));
      y1 = Math.max(y1, n.y + n.h / 2);
    }
    return { x0, y0, x1, y1 };
  }

  // Camera that shows `nodes`, zoomed in at most to maxK.
  view(nodes, maxK) {
    if (!nodes.length) return { ...this.t };
    const b = this.bounds(nodes);
    const pad = Math.max(16, Math.min(56, this.width() * 0.07));
    const W = this.width() - 2 * pad;
    const top = this.reserve.top + (this.banner.hidden ? 0 : this.banner.offsetHeight + 8);
    const H = this.height() - 2 * pad - top - this.reserve.bottom;
    const k = Math.max(0.12, Math.min(maxK, W / Math.max(1, b.x1 - b.x0), H / Math.max(1, b.y1 - b.y0)));
    return {
      k,
      x: pad + (W - (b.x1 - b.x0) * k) / 2 - b.x0 * k,
      y: pad + top + (H - (b.y1 - b.y0) * k) / 2 - b.y0 * k,
    };
  }

  moveTo(target, animate = true) {
    const dur = animate && !ui.reducedMotion() ? CAMERA_MS : 0;
    this.cam = dur ? { from: { ...this.t }, to: target, at: ui.now(), dur } : null;
    if (!dur) this.t = target;
    this.requestDraw();
  }

  fit(animate = true) {
    this.moveTo(this.view(this.nodes, 1.3), animate);
  }

  // Zoom by a factor around the centre of the stage.
  zoomBy(f) {
    const t = this.cam ? this.cam.to : this.t;
    const k = Math.min(4, Math.max(0.12, t.k * f));
    const cx = this.width() / 2;
    const cy = this.height() / 2;
    this.moveTo({ k, x: cx - ((cx - t.x) / t.k) * k, y: cy - ((cy - t.y) / t.k) * k });
  }

  // A selected module: it and its direct imports and importers are lit.
  setHighlight(ids) {
    this.highlight = ids && ids.size ? ids : null;
    this.focus = this.lit = this.path = this.via = null;
    for (const n of this.nodes) n.chipW = 0;
    if (this.highlight) {
      const lit = new Set(this.highlight);
      const path = new Set();
      this.links.forEach((l, k) => {
        if (this.highlight.has(l.source.id) || this.highlight.has(l.target.id)) {
          lit.add(l.source.id);
          lit.add(l.target.id);
          path.add(k);
        }
      });
      this.lit = lit;
      this.path = path;
      const nodes = [...lit].map((id) => this.byId.get(id)).filter(Boolean);
      if (nodes.length) this.moveTo(this.view(nodes, 1.3));
    }
    this.fadeAt = ui.now();
    this.requestDraw();
  }

  // A selected option: per defining module its best definition status
  // (win > lose > off) and that definition's priority. The camera eases to
  // the defining modules that are drawn; the import paths leading to them
  // stay lit.
  setFocus(map) {
    this.focus = map && map.size ? map : null;
    this.highlight = this.lit = this.path = this.via = null;
    for (const n of this.nodes) n.chipW = 0;
    if (this.focus) {
      const ctx = this.ctx;
      ctx.font = CHIP_FONT;
      const nodes = [];
      for (const [id, f] of this.focus) {
        const n = this.byId.get(id);
        if (!n) continue;
        n.chip = ringText(f);
        n.chipW = Math.ceil(ctx.measureText(n.chip).width) + 16;
        nodes.push(n);
      }
      const path = new Set();
      const todo = nodes.map((n) => n.id);
      const done = new Set(todo);
      while (todo.length) {
        for (const k of this.parents.get(todo.pop()) || []) {
          path.add(k);
          const p = this.links[k].source.id;
          if (!done.has(p)) {
            done.add(p);
            todo.push(p);
          }
        }
      }
      this.path = path;
      this.via = done;
      if (nodes.length) this.moveTo(this.view(nodes, 1.5));
    }
    this.fadeAt = ui.now();
    this.requestDraw();
  }

  width() {
    return this.canvas.clientWidth || 800;
  }
  height() {
    return this.canvas.clientHeight || 600;
  }

  resize() {
    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = Math.round(this.width() * dpr);
    this.canvas.height = Math.round(this.height() * dpr);
    this.requestDraw();
  }

  requestDraw() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => {
      this.pending = false;
      this.draw();
    });
  }

  isLit(n) {
    if (this.focus) return this.focus.has(n.id);
    if (this.lit) return this.lit.has(n.id);
    return true;
  }

  // Camera and fade at the clock's current time; true while either moves.
  step() {
    const now = ui.now();
    let moving = false;
    if (this.cam) {
      const p = Math.max(0, Math.min(1, (now - this.cam.at) / this.cam.dur));
      const e = easeCamera(p);
      const { from, to } = this.cam;
      // Zoom interpolates geometrically; the world point at the centre follows.
      const k = from.k * (to.k / from.k) ** e;
      const cx = this.width() / 2;
      const cy = this.height() / 2;
      const fx = (cx - from.x) / from.k;
      const fy = (cy - from.y) / from.k;
      const wx = fx + ((cx - to.x) / to.k - fx) * e;
      const wy = fy + ((cy - to.y) / to.k - fy) * e;
      this.t = p >= 1 ? to : { k, x: cx - wx * k, y: cy - wy * k };
      if (p >= 1) this.cam = null;
      else moving = true;
    }
    this.fade = ui.reducedMotion() ? 1 : Math.max(0, Math.min(1, (now - this.fadeAt) / FADE_MS));
    return moving || this.fade < 1;
  }

  draw() {
    const moving = this.step();
    const { ctx, t, colors } = this;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, this.width(), this.height());

    // The stage's dot grid moves with the camera.
    let g = 24 * t.k;
    while (g < 14) g *= 2;
    this.stage.style.backgroundSize = `${g}px ${g}px`;
    this.stage.style.backgroundPosition = `${(t.x % g).toFixed(1)}px ${(t.y % g).toFixed(1)}px`;

    const dimming = !!(this.focus || this.lit);
    const dimA = 1 - 0.8 * (dimming ? this.fade : 0);
    const viaA = 1 - 0.45 * (dimming ? this.fade : 0);
    ctx.save();
    ctx.translate(t.x, t.y);
    ctx.scale(t.k, t.k);
    const px = 1 / t.k; // one screen pixel in world units

    // Edges: the rest first, then the import paths to what is selected.
    ctx.lineCap = "round";
    const edge = (l) => {
      const p = l.pts;
      ctx.moveTo(p[0].x, p[0].y);
      for (let i = 1; i < p.length; i++) {
        const a = p[i - 1];
        const b = p[i];
        if (i % 2 === 0) ctx.lineTo(b.x, b.y); // straight through a dummy's column
        else {
          const mx = (a.x + b.x) / 2;
          ctx.bezierCurveTo(mx, a.y, mx, b.y, b.x, b.y);
        }
      }
    };
    for (const dashed of [false, true]) {
      ctx.setLineDash(dashed ? [5 * px, 4 * px] : []);
      ctx.beginPath();
      this.links.forEach((l, k) => l.dashed === dashed && !(this.path && this.path.has(k)) && edge(l));
      ctx.strokeStyle = colors.edge;
      ctx.globalAlpha = this.path ? dimA : 1;
      ctx.lineWidth = Math.max(1.25, px);
      ctx.stroke();
      if (this.path) {
        ctx.beginPath();
        for (const k of this.path) if (this.links[k].dashed === dashed) edge(this.links[k]);
        ctx.globalAlpha = 1;
        ctx.strokeStyle = colors.text3;
        ctx.lineWidth = Math.max(1.6, 1.2 * px);
        ctx.stroke();
      }
    }
    ctx.setLineDash([]);

    // Pills (labels only where readable), lit ones on top.
    const showText = t.k >= 0.42;
    const lit = [];
    for (const n of this.nodes) {
      if (dimming && this.isLit(n)) {
        lit.push(n);
        continue;
      }
      ctx.globalAlpha = !dimming ? 1 : this.via && this.via.has(n.id) ? viaA : dimA;
      this.drawPill(n, showText, px);
    }
    ctx.globalAlpha = 1;
    for (const n of lit) this.drawPill(n, true, px);
    for (const n of lit) this.drawStatus(n, px);
    if (this.selected && !this.focus) this.drawRing(this.selected, colors.accent, 2 * Math.max(px, 0.9), px);
    ctx.restore();
    if (moving) this.requestDraw();
  }

  drawPill(n, showText, px) {
    const { ctx, colors } = this;
    const hover = n === this.hover;
    ctx.beginPath();
    ctx.roundRect(n.x, n.y - n.h / 2, n.w, n.h, n.h / 2);
    ctx.fillStyle = hover ? colors.s3 : colors.s2;
    ctx.fill();
    ctx.lineWidth = px;
    ctx.strokeStyle = hover ? colors.text3 : colors.line2;
    if (n.m.disabled) ctx.setLineDash([4 * px, 3 * px]);
    ctx.stroke();
    ctx.setLineDash([]);
    originMark(ctx, n.cls, n.x + 15, n.y, colors[n.cls], px);
    if (!showText) return;
    ctx.font = PILL_FONT;
    ctx.textBaseline = "middle";
    ctx.fillStyle = n.m.disabled ? colors.text3 : n.cls === "nixpkgs" ? colors.text2 : colors.text;
    ctx.fillText(n.label, n.x + 25, n.y + 0.5);
  }

  drawRing(n, color, width, px, dash) {
    const { ctx } = this;
    const o = 3.5 * px + width / 2;
    ctx.beginPath();
    ctx.roundRect(n.x - o, n.y - n.h / 2 - o, n.w + 2 * o, n.h + 2 * o, n.h / 2 + o);
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.setLineDash(dash || []);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // Ring and trailing priority chip of a module that defines the selected
  // option. Winner: solid ring with a glow and a filled "✓" chip; lost: thin
  // ring, outlined chip; off: dashed ring and chip. Shape and text differ,
  // not only the colour.
  drawStatus(n, px) {
    const f = this.focus && this.focus.get(n.id);
    if (!f) return;
    const { ctx, colors } = this;
    ctx.globalAlpha = this.fade;
    const color = colors[f.status];
    const lw = Math.max(px, 0.9);
    if (f.status === "win") {
      ctx.save();
      ctx.shadowColor = color;
      ctx.shadowBlur = 14 * this.t.k;
      this.drawRing(n, color, 2 * lw, px);
      ctx.restore();
    } else this.drawRing(n, color, 1.5 * lw, px, f.status === "off" ? [4 * px, 3 * px] : null);
    if (n.chipW) {
      const x = n.x + n.w + 8;
      ctx.beginPath();
      ctx.roundRect(x, n.y - CHIP_H / 2, n.chipW, CHIP_H, CHIP_H / 2);
      if (f.status === "win") {
        ctx.fillStyle = color;
        ctx.fill();
      } else {
        ctx.fillStyle = colors.s1;
        ctx.fill();
        ctx.lineWidth = lw;
        ctx.strokeStyle = color;
        if (f.status === "off") ctx.setLineDash([3 * px, 2.5 * px]);
        ctx.stroke();
        ctx.setLineDash([]);
      }
      ctx.font = CHIP_FONT;
      ctx.textBaseline = "middle";
      ctx.fillStyle = f.status === "win" ? colors.onWin : color;
      ctx.fillText(n.chip, x + 8, n.y + 0.5);
    }
    ctx.globalAlpha = 1;
  }

  // Screen rectangle of a module's pill (client coordinates), or null.
  nodeRect(id) {
    const n = this.byId.get(id);
    if (!n) return null;
    const c = this.canvas.getBoundingClientRect();
    const { t } = this;
    return { x: c.left + t.x + n.x * t.k, y: c.top + t.y + (n.y - n.h / 2) * t.k, w: n.w * t.k, h: n.h * t.k, chipW: n.chipW * t.k };
  }

  toWorld(px, py) {
    return [(px - this.t.x) / this.t.k, (py - this.t.y) / this.t.k];
  }

  nodeAt(px, py) {
    const [x, y] = this.toWorld(px, py);
    const slack = 3 / this.t.k;
    for (let i = this.nodes.length - 1; i >= 0; i--) {
      const n = this.nodes[i];
      if (x >= n.x - slack && x <= n.x + n.w + slack && Math.abs(y - n.y) <= n.h / 2 + slack) return n;
    }
    return null;
  }

  select(id) {
    this.selected = id ? this.byId.get(id) || null : null;
    this.requestDraw();
  }

  setHover(n) {
    if (n === this.hover) return;
    this.hover = n;
    this.canvas.classList.toggle("on-node", !!n);
    this.showTip(n);
    this.requestDraw();
  }

  // Tooltip: full path, origin, how many options the module sets.
  showTip(n) {
    const tip = this.tip;
    if (!n) {
      tip.hidden = true;
      return;
    }
    const m = n.m;
    const sets = (this.model.modOptions.get(m.id) || []).length;
    const importers = (this.parents.get(m.id) || []).length;
    tip.replaceChildren(
      h("div", { class: "tip-title" }, originDot(m.origin), h("b", {}, n.label), chip(m.origin)),
      h("div", { class: "tip-path" }, moduleLabel(m)),
      h(
        "div",
        { class: "tip-facts" },
        h("span", {}, h("b", {}, String(sets)), sets === 1 ? " option set" : " options set"),
        h("span", {}, h("b", {}, String(m.imports.length)), " imports"),
        h("span", {}, h("b", {}, String(importers)), importers === 1 ? " importer" : " importers"),
      ),
      m.disabled ? h("div", { class: "tip-note" }, "disabled (disabledModules): not evaluated") : null,
    );
    tip.hidden = false;
    const r = this.nodeRect(n.id);
    const c = this.canvas.getBoundingClientRect();
    let x = r.x - c.left;
    let y = r.y - c.top - tip.offsetHeight - 10;
    if (y < 8) y = r.y - c.top + r.h + 10;
    x = Math.max(8, Math.min(x, this.width() - tip.offsetWidth - 8));
    tip.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`;
  }

  bindPointer() {
    const c = this.canvas;
    const pointers = new Map();
    let drag = null;
    let pinch = null;
    const stopCamera = () => {
      if (!this.cam) return;
      this.step();
      this.cam = null;
    };
    c.addEventListener("pointerdown", (e) => {
      stopCamera();
      pointers.set(e.pointerId, { x: e.offsetX, y: e.offsetY });
      c.setPointerCapture(e.pointerId);
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinch = { d: Math.hypot(a.x - b.x, a.y - b.y), t: { ...this.t }, mx: (a.x + b.x) / 2, my: (a.y + b.y) / 2 };
        drag = null;
      } else drag = { x: e.offsetX, y: e.offsetY, tx: this.t.x, ty: this.t.y, moved: false };
    });
    c.addEventListener("pointermove", (e) => {
      if (pointers.has(e.pointerId)) pointers.set(e.pointerId, { x: e.offsetX, y: e.offsetY });
      if (pinch && pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const k = Math.min(4, Math.max(0.12, (pinch.t.k * Math.hypot(a.x - b.x, a.y - b.y)) / Math.max(1, pinch.d)));
        const wx = (pinch.mx - pinch.t.x) / pinch.t.k;
        const wy = (pinch.my - pinch.t.y) / pinch.t.k;
        this.t = { k, x: (a.x + b.x) / 2 - wx * k, y: (a.y + b.y) / 2 - wy * k };
        this.requestDraw();
        return;
      }
      if (!drag) {
        if (e.pointerType === "mouse") this.setHover(this.nodeAt(e.offsetX, e.offsetY));
        return;
      }
      const dx = e.offsetX - drag.x;
      const dy = e.offsetY - drag.y;
      if (!drag.moved && Math.abs(dx) + Math.abs(dy) > 4) {
        drag.moved = true;
        c.classList.add("panning");
        this.setHover(null);
      }
      if (drag.moved) {
        this.t = { ...this.t, x: drag.tx + dx, y: drag.ty + dy };
        this.requestDraw();
      }
    });
    const end = (e) => {
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinch = null;
      c.classList.remove("panning");
      if (drag && !drag.moved && e.type === "pointerup") {
        const n = this.nodeAt(e.offsetX, e.offsetY);
        if (n) {
          this.select(n.id);
          this.onSelect(n.m);
        }
      }
      drag = null;
    };
    c.addEventListener("pointerup", end);
    c.addEventListener("pointercancel", end);
    c.addEventListener("pointerleave", () => this.setHover(null));
    c.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        stopCamera();
        const k = Math.min(4, Math.max(0.12, this.t.k * Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015))));
        const [wx, wy] = this.toWorld(e.offsetX, e.offsetY);
        this.t = { k, x: e.offsetX - wx * k, y: e.offsetY - wy * k };
        this.setHover(null);
        this.requestDraw();
      },
      { passive: false },
    );
  }
}

// Origin mark at the start of a pill: user a disc, user-inline a ring,
// input a diamond, unknown a square, nixpkgs a small disc. Shape and colour
// both differ.
function originMark(ctx, cls, x, y, color, px) {
  ctx.beginPath();
  if (cls === "input") {
    const d = 4.6;
    ctx.moveTo(x, y - d);
    ctx.lineTo(x + d, y);
    ctx.lineTo(x, y + d);
    ctx.lineTo(x - d, y);
    ctx.closePath();
  } else if (cls === "unknown") ctx.rect(x - 3.6, y - 3.6, 7.2, 7.2);
  else ctx.arc(x, y, cls === "nixpkgs" ? 3 : cls === "user-inline" ? 3.3 : 4, 0, 2 * Math.PI);
  if (cls === "user-inline") {
    ctx.lineWidth = Math.max(1.8, 1.5 * px);
    ctx.strokeStyle = color;
    ctx.stroke();
  } else {
    ctx.fillStyle = color;
    ctx.fill();
  }
}

// The same marks in HTML (legend, tooltip, ladder pills).
function originDot(origin) {
  return h("span", { class: `odot o-${originClass(origin)}`, "aria-hidden": "true" });
}

// Chip text of a defining module: "✓ 50 mkForce", "1000 mkDefault", "mkIf false".
function ringText(f) {
  if (f.status === "off") return f.condition === "mkIf-error" ? "mkIf error" : "mkIf false";
  return `${f.status === "win" ? "✓ " : ""}${priorityLabel(f.priority)}`;
}
