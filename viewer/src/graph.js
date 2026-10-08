// Module import graph on a <canvas>: d3-force layout, own pan/zoom.
// Canvas instead of SVG keeps a few thousand nodes responsive.

const GRAPH_LIMIT = 1500; // above this many modules only non-nixpkgs ones are drawn

class ModuleGraph {
  constructor(canvas, banner, onSelect) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.banner = banner;
    this.onSelect = onSelect;
    this.t = { x: 0, y: 0, k: 1 };
    this.nodes = [];
    this.links = [];
    this.highlight = null; // Set of module ids, or null
    this.selected = null;
    this.colors = {};
    this.pending = false;
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
      fg: v("--fg"),
      accent: v("--accent"),
      bg: v("--bg"),
    };
  }

  setModel(model) {
    if (this.sim) this.sim.stop();
    const all = model.modules;
    const tooMany = all.length > GRAPH_LIMIT;
    const shown = tooMany ? all.filter(isUserish) : all;
    this.banner.hidden = !tooMany;
    if (tooMany) {
      this.banner.textContent = `Too large to draw ${all.length} modules: showing the ${shown.length} non-nixpkgs modules only. Search and option details cover everything.`;
    }
    const byId = new Map();
    this.nodes = shown.map((m) => {
      const n = { id: m.id, m, label: moduleLabel(m), cls: originClass(m.origin), r: isUserish(m) ? 6 : 3.5 };
      byId.set(m.id, n);
      return n;
    });
    this.links = [];
    for (const m of shown) {
      for (const t of m.imports) {
        if (byId.has(t)) this.links.push({ source: m.id, target: t, dashed: byId.get(t).m.disabled });
      }
    }
    this.byId = byId;
    this.highlight = null;
    this.selected = null;
    this.t = { x: this.width() / 2, y: this.height() / 2, k: 1 };
    const big = this.nodes.length > 500;
    this.sim = d3
      .forceSimulation(this.nodes)
      .force("link", d3.forceLink(this.links).id((d) => d.id).distance(big ? 18 : 40).strength(0.6))
      .force("charge", d3.forceManyBody().strength(big ? -12 : -90).theta(0.9).distanceMax(400))
      .force("x", d3.forceX(0).strength(0.04))
      .force("y", d3.forceY(0).strength(0.04))
      .alphaDecay(big ? 0.05 : 0.03)
      .on("tick", () => this.requestDraw());
  }

  setHighlight(ids) {
    this.highlight = ids && ids.size ? ids : null;
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

  draw() {
    const { ctx, t, colors } = this;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, this.width(), this.height());
    ctx.translate(t.x, t.y);
    ctx.scale(t.k, t.k);
    const hl = this.highlight;

    ctx.lineWidth = 1 / t.k;
    ctx.strokeStyle = colors.edge;
    ctx.beginPath();
    for (const l of this.links) {
      if (l.dashed) continue;
      ctx.moveTo(l.source.x, l.source.y);
      ctx.lineTo(l.target.x, l.target.y);
    }
    ctx.stroke();
    ctx.setLineDash([4 / t.k, 3 / t.k]);
    ctx.beginPath();
    for (const l of this.links) {
      if (!l.dashed) continue;
      ctx.moveTo(l.source.x, l.source.y);
      ctx.lineTo(l.target.x, l.target.y);
    }
    ctx.stroke();
    ctx.setLineDash([]);

    for (const n of this.nodes) {
      const lit = !hl || hl.has(n.id);
      ctx.globalAlpha = lit ? 1 : 0.18;
      ctx.beginPath();
      const r = hl && lit ? n.r + 3 : n.r;
      ctx.arc(n.x, n.y, r, 0, 2 * Math.PI);
      if (n.m.disabled) {
        ctx.setLineDash([2 / t.k, 2 / t.k]);
        ctx.strokeStyle = colors[n.cls];
        ctx.lineWidth = 1.5 / t.k;
        ctx.stroke();
        ctx.setLineDash([]);
      } else {
        ctx.fillStyle = colors[n.cls];
        ctx.fill();
      }
      if (n === this.selected || (hl && lit)) {
        ctx.strokeStyle = colors.accent;
        ctx.lineWidth = 2 / t.k;
        ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;

    // Labels: non-nixpkgs and highlighted modules; the rest only when zoomed in.
    ctx.fillStyle = colors.fg;
    ctx.font = `${11 / t.k}px system-ui, sans-serif`;
    for (const n of this.nodes) {
      const lit = hl && hl.has(n.id);
      const show = lit || n === this.selected || (n.cls !== "nixpkgs" ? t.k > 0.5 : t.k > 2.5);
      if (!show || (hl && !lit && n !== this.selected)) continue;
      ctx.fillText(n.label, n.x + n.r + 3, n.y + 4 / t.k);
    }
  }

  toWorld(px, py) {
    return [(px - this.t.x) / this.t.k, (py - this.t.y) / this.t.k];
  }

  nodeAt(px, py) {
    const [x, y] = this.toWorld(px, py);
    // Nearest node within max(its radius, 6 screen pixels).
    let best = null;
    let bestD = Infinity;
    for (const n of this.nodes) {
      const d = (n.x - x) ** 2 + (n.y - y) ** 2;
      const reach = Math.max(n.r, 6 / this.t.k);
      if (d <= reach * reach && d < bestD) {
        best = n;
        bestD = d;
      }
    }
    return best;
  }

  select(id) {
    this.selected = id ? this.byId.get(id) || null : null;
    this.requestDraw();
  }

  bindPointer() {
    const c = this.canvas;
    let drag = null;
    c.addEventListener("pointerdown", (e) => {
      drag = { x: e.offsetX, y: e.offsetY, tx: this.t.x, ty: this.t.y, moved: false };
      c.setPointerCapture(e.pointerId);
    });
    c.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const dx = e.offsetX - drag.x;
      const dy = e.offsetY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) {
        drag.moved = true;
        c.classList.add("panning");
      }
      this.t.x = drag.tx + dx;
      this.t.y = drag.ty + dy;
      this.requestDraw();
    });
    c.addEventListener("pointerup", (e) => {
      c.classList.remove("panning");
      if (drag && !drag.moved) {
        const n = this.nodeAt(e.offsetX, e.offsetY);
        this.select(n ? n.id : null);
        this.onSelect(n ? n.m : null);
      }
      drag = null;
    });
    c.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const k = Math.min(8, Math.max(0.05, this.t.k * Math.exp(-e.deltaY * 0.0015)));
        const [wx, wy] = this.toWorld(e.offsetX, e.offsetY);
        this.t.k = k;
        this.t.x = e.offsetX - wx * k;
        this.t.y = e.offsetY - wy * k;
        this.requestDraw();
      },
      { passive: false },
    );
  }
}
