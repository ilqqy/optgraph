// Module import graph on a <canvas>: d3-force layout, own pan/zoom.
// Canvas instead of SVG keeps a few thousand nodes responsive.

const GRAPH_LIMIT = 1500; // above this many modules only non-nixpkgs ones are drawn

// Rings around the modules that define the selected option.
const RING_RANK = { win: 3, lose: 2, off: 1 };

class ModuleGraph {
  constructor(canvas, banner, onSelect) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.banner = banner;
    this.onSelect = onSelect;
    this.t = { x: 0, y: 0, k: 1 };
    this.nodes = [];
    this.links = [];
    this.highlight = null; // Set of module ids (a selected module), or null
    this.focus = null; // Map module id -> { status, priority } (a selected option), or null
    this.selected = null;
    this.hover = null;
    this.colors = {};
    this.pending = false;
    this.reserveTop = 0; // pixels kept free above the fitted graph (tour captions)
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
      muted: v("--muted"),
      accent: v("--accent"),
      bg: v("--bg"),
      panel: v("--panel"),
      border: v("--border"),
      win: v("--ring-win"),
      lose: v("--ring-lose"),
      off: v("--ring-off"),
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
      const n = { id: m.id, m, label: moduleShortLabel(m), cls: originClass(m.origin), r: isUserish(m) ? 6 : 3.5, deg: 0, labelW: null };
      byId.set(m.id, n);
      return n;
    });
    this.links = [];
    for (const m of shown) {
      for (const t of m.imports) {
        if (byId.has(t)) {
          this.links.push({ source: m.id, target: t, dashed: byId.get(t).m.disabled });
          byId.get(m.id).deg++;
          byId.get(t).deg++;
        }
      }
    }
    this.byId = byId;
    this.highlight = null;
    this.focus = null;
    this.selected = null;
    this.hover = null;
    this.t = { x: this.width() / 2, y: this.height() / 2, k: 1 };
    const big = this.nodes.length > 500;
    this.fitted = false;
    this.userMoved = false;
    this.sim = d3
      .forceSimulation(this.nodes)
      .force("link", d3.forceLink(this.links).id((d) => d.id).distance(big ? 18 : 70).strength(0.5))
      .force("charge", d3.forceManyBody().strength(big ? -12 : -260).theta(0.9).distanceMax(big ? 300 : 220))
      .force("collide", d3.forceCollide((d) => d.r + (big ? 2 : 14)))
      // Pull disconnected roots (e.g. inline modules) in, so they don't drift off.
      .force("x", d3.forceX(0).strength(big ? 0.04 : 0.12))
      .force("y", d3.forceY(0).strength(big ? 0.04 : 0.12))
      .alphaDecay(big ? 0.05 : 0.03)
      .on("tick", () => {
        // Fit the view once the layout has mostly settled, unless the user moved it.
        if (!this.fitted && this.sim.alpha() < 0.12) this.fit();
        this.requestDraw();
      })
      .on("end", () => this.userMoved || this.fit());
    if (!big) {
      // Small graphs: settle synchronously (a few ms) and show the final layout.
      this.sim.stop();
      this.sim.tick(300);
      this.fit();
    }
  }

  // Scale and centre the view on the current node positions.
  fit() {
    this.fitted = true;
    if (!this.nodes.length) return;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const n of this.nodes) {
      x0 = Math.min(x0, n.x);
      y0 = Math.min(y0, n.y);
      x1 = Math.max(x1, n.x + 90); // room for the label
      y1 = Math.max(y1, n.y);
    }
    const pad = 40;
    const w = this.width() - 2 * pad;
    const hgt = this.height() - 2 * pad - 50 - this.reserveTop; // legend
    const k = Math.min(2, w / Math.max(1, x1 - x0), hgt / Math.max(1, y1 - y0));
    this.t = { k, x: pad + (w - (x1 - x0) * k) / 2 - x0 * k, y: pad + this.reserveTop + (hgt - (y1 - y0) * k) / 2 - y0 * k };
    this.requestDraw();
  }

  // A selected module: it is lit, everything else dims.
  setHighlight(ids) {
    this.highlight = ids && ids.size ? ids : null;
    this.focus = null;
    this.requestDraw();
  }

  // A selected option: per defining module its best definition status
  // (win > lose > off) and that definition's priority.
  setFocus(map) {
    this.focus = map && map.size ? map : null;
    this.highlight = null;
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
    if (this.focus) return this.focus.has(n.id) || n === this.selected;
    if (this.highlight) return this.highlight.has(n.id) || n === this.selected;
    return true;
  }

  // Drawn radius in world units: lit nodes grow while something is selected.
  radius(n) {
    return (this.focus || this.highlight) && this.isLit(n) ? n.r + 3 : n.r;
  }

  draw() {
    const { ctx, t, colors } = this;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, this.width(), this.height());
    ctx.save();
    ctx.translate(t.x, t.y);
    ctx.scale(t.k, t.k);

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
      const lit = this.isLit(n);
      ctx.globalAlpha = lit ? 1 : 0.16;
      const r = this.radius(n);
      ctx.beginPath();
      nodePath(ctx, n, r);
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
      const f = this.focus && this.focus.get(n.id);
      if (f) {
        // Status ring; winners also get a second, inner ring, so the status
        // never depends on colour alone (lost: single ring, off: dashed).
        ctx.globalAlpha = 1;
        ctx.strokeStyle = colors[f.status];
        ctx.lineWidth = (f.status === "win" ? 3 : 2) / t.k;
        if (f.status === "off") ctx.setLineDash([3 / t.k, 2.5 / t.k]);
        ctx.beginPath();
        ctx.arc(n.x, n.y, r + 3.5 / t.k, 0, 2 * Math.PI);
        ctx.stroke();
        ctx.setLineDash([]);
        if (f.status === "win") {
          ctx.lineWidth = 1.5 / t.k;
          ctx.strokeStyle = colors.bg;
          ctx.beginPath();
          ctx.arc(n.x, n.y, r + 0.75 / t.k, 0, 2 * Math.PI);
          ctx.stroke();
        }
      } else if (n === this.selected || (this.highlight && lit)) {
        ctx.strokeStyle = colors.accent;
        ctx.lineWidth = 2 / t.k;
        ctx.stroke();
      }
    }
    ctx.globalAlpha = 1;
    ctx.restore();
    this.drawLabels();
  }

  // Labels in screen space. Candidates in priority order (selected, hovered
  // and defining modules first, then by number of import edges); each takes
  // the first free spot right, left, below or above its node, or is hidden.
  // Forced labels are always drawn. The hovered node shows its full label.
  drawLabels() {
    const { ctx, t, colors } = this;
    const W = this.width();
    const H = this.height();
    const dimming = !!(this.focus || this.highlight);
    const font = "11px system-ui, sans-serif";
    const subFont = "600 10.5px system-ui, sans-serif";
    ctx.font = font;
    const screen = (n) => [t.x + n.x * t.k, t.y + n.y * t.k];
    const onScreen = (sx, sy) => sx > -150 && sx < W + 20 && sy > -20 && sy < H + 20;

    // Occupied screen boxes (node discs and placed labels) in a uniform grid,
    // so collision checks stay cheap with many nodes.
    const CELL = 64;
    const grid = new Map();
    const cells = (b, f) => {
      for (let cx = Math.floor(b.x0 / CELL); cx <= Math.floor(b.x1 / CELL); cx++)
        for (let cy = Math.floor(b.y0 / CELL); cy <= Math.floor(b.y1 / CELL); cy++) f(`${cx},${cy}`);
    };
    const occupy = (b) => cells(b, (key) => (grid.get(key) || grid.set(key, []).get(key)).push(b));
    const hits = (b) => {
      let hit = false;
      cells(b, (key) => {
        if (!hit) hit = (grid.get(key) || []).some((o) => o.n !== b.n && b.x0 < o.x1 && b.x1 > o.x0 && b.y0 < o.y1 && b.y1 > o.y0);
      });
      return hit;
    };
    const cands = [];
    for (const n of this.nodes) {
      const [sx, sy] = screen(n);
      if (!onScreen(sx, sy)) continue;
      const R = this.radius(n) * t.k + (this.focus && this.focus.has(n.id) ? 4 : 0);
      occupy({ n, x0: sx - R, y0: sy - R, x1: sx + R, y1: sy + R });
      const forced = n === this.selected || (dimming && this.isLit(n));
      const zoomOk = n.cls === "nixpkgs" ? t.k > 2.5 : t.k > 0.35;
      if (!forced && (dimming || !zoomOk)) continue;
      cands.push({ n, sx, sy, R, forced, rank: forced ? Infinity : n.deg + (n.cls === "nixpkgs" ? 0 : 1000) });
    }
    cands.sort((a, b) => b.rank - a.rank);

    const placed = [];
    for (const c of cands) {
      const { n, sx, sy, R } = c;
      if (n.labelW == null) n.labelW = ctx.measureText(n.label).width;
      const f = this.focus && this.focus.get(n.id);
      const sub = f ? ringText(f) : null;
      let w = n.labelW;
      if (sub) {
        ctx.font = subFont;
        w = Math.max(w, ctx.measureText(sub).width);
        ctx.font = font;
      }
      const hgt = sub ? 26 : 13;
      const spots = [
        [sx + R + 4, sy - (sub ? 7 : 6.5)], // right
        [sx - R - 4 - w, sy - (sub ? 7 : 6.5)], // left
        [sx - w / 2, sy + R + 2], // below
        [sx - w / 2, sy - R - 2 - hgt], // above
      ];
      let box = null;
      for (const [x, y] of spots) {
        const b = { n, x0: x, y0: y, x1: x + w, y1: y + hgt };
        if (!hits(b)) {
          box = b;
          break;
        }
      }
      if (!box && c.forced) box = { n, x0: spots[0][0], y0: spots[0][1], x1: spots[0][0] + w, y1: spots[0][1] + hgt };
      if (!box) continue;
      occupy(box);
      placed.push(box);
      box.sub = sub;
      box.status = f ? f.status : null;
    }
    this.labelBoxes = placed; // read by the tour, to point next to a label

    ctx.textBaseline = "top";
    for (const b of placed) {
      ctx.font = font;
      ctx.fillStyle = colors.fg;
      ctx.fillText(b.n.label, b.x0, b.y0);
      if (b.sub) {
        ctx.font = subFont;
        ctx.fillStyle = colors[b.status];
        ctx.fillText(b.sub, b.x0, b.y0 + 13);
      }
    }

    if (this.hover) {
      const n = this.hover;
      const [sx, sy] = screen(n);
      const text = `${moduleLabel(n.m)}  ·  ${n.m.origin}`;
      ctx.font = font;
      const w = ctx.measureText(text).width + 12;
      const R = this.radius(n) * t.k + 6;
      let x = sx + R;
      if (x + w > W - 4) x = Math.max(4, sx - R - w);
      const y = Math.min(Math.max(4, sy - 10), H - 24);
      ctx.fillStyle = colors.panel;
      ctx.strokeStyle = colors.border;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.roundRect(x, y, w, 20, 5);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = colors.fg;
      ctx.fillText(text, x + 6, y + 4);
    }
    ctx.textBaseline = "alphabetic";
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
      const reach = Math.max(this.radius(n), 6 / this.t.k);
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

  setHover(n) {
    if (n === this.hover) return;
    this.hover = n;
    this.canvas.classList.toggle("on-node", !!n);
    this.requestDraw();
  }

  bindPointer() {
    const c = this.canvas;
    let drag = null;
    c.addEventListener("pointerdown", (e) => {
      this.fitted = this.userMoved = true;
      drag = { x: e.offsetX, y: e.offsetY, tx: this.t.x, ty: this.t.y, moved: false };
      c.setPointerCapture(e.pointerId);
    });
    c.addEventListener("pointermove", (e) => {
      if (!drag) {
        this.setHover(this.nodeAt(e.offsetX, e.offsetY));
        return;
      }
      const dx = e.offsetX - drag.x;
      const dy = e.offsetY - drag.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) {
        drag.moved = true;
        c.classList.add("panning");
        this.setHover(null);
      }
      this.t.x = drag.tx + dx;
      this.t.y = drag.ty + dy;
      this.requestDraw();
    });
    c.addEventListener("pointerleave", () => this.setHover(null));
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
        this.fitted = this.userMoved = true;
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

// Node outline: input modules are diamonds, unknown ones squares, the rest
// circles, so origins never depend on colour alone.
function nodePath(ctx, n, r) {
  if (n.cls === "input") {
    const d = r * 1.3;
    ctx.moveTo(n.x, n.y - d);
    ctx.lineTo(n.x + d, n.y);
    ctx.lineTo(n.x, n.y + d);
    ctx.lineTo(n.x - d, n.y);
    ctx.closePath();
  } else if (n.cls === "unknown") {
    const d = r * 0.9;
    ctx.rect(n.x - d, n.y - d, 2 * d, 2 * d);
  } else {
    ctx.arc(n.x, n.y, r, 0, 2 * Math.PI);
  }
}

// Second label line of a defining module: "✓ 50 mkForce", "1000 mkDefault", "mkIf false".
function ringText(f) {
  if (f.status === "off") return f.condition === "mkIf-error" ? "mkIf error" : "mkIf false";
  return `${f.status === "win" ? "✓ " : ""}${priorityLabel(f.priority)}`;
}
