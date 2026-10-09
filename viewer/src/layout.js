// Layered left-to-right layout of the import graph (Sugiyama style, by hand):
//   1. cycles: DFS back edges are reversed while layering;
//   2. layers: longest path from the roots (a module sits one column right
//      of its deepest importer);
//   3. an edge spanning several columns gets a dummy node in each, so it is
//      routed between the modules instead of through them;
//   4. order within a column: barycenter sweeps, down and up, keeping the
//      order with the fewest edge crossings;
//   5. y: each node pulled to the mean y of its neighbours, without overlaps
//      and keeping the order (isotonic regression, pool adjacent violators);
//      x: columns as wide as their widest node, plus a gap.
// Nodes without any edge are packed into a grid under the rest.
//
// layeredLayout(sizes: [{ w, h }], edges: [[from, to]]) ->
//   { x[], y[] (node centre), layer[], paths: [[{x, y}...] per edge] }

const LAYOUT_GAP_X = 120; // between columns: room for curves and priority chips
const LAYOUT_GAP_Y = 12; // between nodes in a column
const LAYOUT_DUMMY_H = 4;
const LAYOUT_SWEEPS = 24;

function layeredLayout(sizes, edges) {
  const n = sizes.length;
  const out = Array.from({ length: n }, () => []);
  for (const [a, b] of edges) if (a !== b) out[a].push(b);

  // 1. Back edges (to a node on the DFS stack) point backwards.
  const state = new Uint8Array(n); // 0 new, 1 on stack, 2 done
  const back = new Set();
  for (let r = 0; r < n; r++) {
    if (state[r]) continue;
    const stack = [[r, 0]];
    state[r] = 1;
    while (stack.length) {
      const top = stack[stack.length - 1];
      const [u, i] = top;
      if (i < out[u].length) {
        top[1]++;
        const v = out[u][i];
        if (state[v] === 1) back.add(`${u}>${v}`);
        else if (state[v] === 0) {
          state[v] = 1;
          stack.push([v, 0]);
        }
      } else {
        state[u] = 2;
        stack.pop();
      }
    }
  }
  const dag = []; // [from, to, edge index]
  edges.forEach(([a, b], k) => {
    if (a === b) return;
    dag.push(back.has(`${a}>${b}`) ? [b, a, k] : [a, b, k]);
  });

  // 2. Longest path from the roots (Kahn order).
  const succ = Array.from({ length: n }, () => []);
  const indeg = new Int32Array(n);
  for (const [a, b] of dag) {
    succ[a].push(b);
    indeg[b]++;
  }
  const layer = new Int32Array(n);
  const queue = [];
  for (let v = 0; v < n; v++) if (!indeg[v]) queue.push(v);
  for (let qi = 0; qi < queue.length; qi++) {
    const u = queue[qi];
    for (const v of succ[u]) {
      layer[v] = Math.max(layer[v], layer[u] + 1);
      if (--indeg[v] === 0) queue.push(v);
    }
  }

  const connected = new Uint8Array(n);
  for (const [a, b] of dag) connected[a] = connected[b] = 1;
  const isolated = [];
  for (let v = 0; v < n; v++) if (!connected[v] && n > 1) isolated.push(v);
  const packIsolated = isolated.length > 6 && isolated.length < n;
  const inLayout = (v) => !packIsolated || connected[v];

  // 3. Dummy nodes; chains[k]: node ids along edge k, left to right.
  const L = Math.max(0, ...Array.from(layer)) + 1;
  const h = sizes.map((s) => s.h);
  const w = sizes.map((s) => s.w);
  const lay = Array.from(layer);
  const up = Array.from({ length: n }, () => []);
  const down = Array.from({ length: n }, () => []);
  const chains = new Array(edges.length).fill(null);
  const link = (a, b) => {
    down[a].push(b);
    up[b].push(a);
  };
  for (const [a, b, k] of dag) {
    const chain = [a];
    for (let l = lay[a] + 1; l < lay[b]; l++) {
      const d = lay.length;
      lay.push(l);
      h.push(LAYOUT_DUMMY_H);
      w.push(0);
      up.push([]);
      down.push([]);
      link(chain[chain.length - 1], d);
      chain.push(d);
    }
    link(chain[chain.length - 1], b);
    chain.push(b);
    chains[k] = chain;
  }
  const N = lay.length;

  // Initial order: depth-first from the roots, in input order.
  const layers = Array.from({ length: L }, () => []);
  const seen = new Uint8Array(N);
  const visit = (r) => {
    const stack = [r];
    while (stack.length) {
      const u = stack.pop();
      if (seen[u]) continue;
      seen[u] = 1;
      layers[lay[u]].push(u);
      for (let i = down[u].length - 1; i >= 0; i--) if (!seen[down[u][i]]) stack.push(down[u][i]);
    }
  };
  for (let v = 0; v < n; v++) if (inLayout(v) && !up[v].length) visit(v);
  for (let v = 0; v < N; v++) if (!seen[v] && (v >= n || inLayout(v))) visit(v);

  // 4. Barycenter sweeps.
  const pos = new Float64Array(N);
  const index = () => layers.forEach((col) => col.forEach((v, i) => (pos[v] = i)));
  index();
  const crossings = () => {
    let total = 0;
    for (let l = 0; l + 1 < L; l++) {
      const segs = [];
      for (const u of layers[l]) for (const v of down[u]) segs.push([pos[u], pos[v]]);
      segs.sort((p, q) => p[0] - q[0] || p[1] - q[1]);
      // Inversions in the lower ends (Fenwick tree).
      const size = layers[l + 1].length + 1;
      const tree = new Int32Array(size + 1);
      let seenSoFar = 0;
      for (const [, b] of segs) {
        let s = 0;
        for (let i = b + 1; i > 0; i -= i & -i) s += tree[i];
        total += seenSoFar - s;
        for (let i = b + 1; i <= size; i += i & -i) tree[i]++;
        seenSoFar++;
      }
    }
    return total;
  };
  let best = layers.map((c) => c.slice());
  let bestX = crossings();
  for (let it = 0; it < LAYOUT_SWEEPS && bestX > 0; it++) {
    const downward = it % 2 === 0;
    for (let s = 1; s < L; s++) {
      const l = downward ? s : L - 1 - s;
      const nb = downward ? up : down;
      const col = layers[l];
      const key = new Map();
      for (const v of col) {
        const adj = nb[v];
        key.set(v, adj.length ? adj.reduce((acc, u) => acc + pos[u], 0) / adj.length : pos[v]);
      }
      col.sort((a, b) => key.get(a) - key.get(b) || pos[a] - pos[b]);
      col.forEach((v, i) => (pos[v] = i));
    }
    const x = crossings();
    if (x < bestX) {
      bestX = x;
      best = layers.map((c) => c.slice());
    }
  }
  best.forEach((c, l) => (layers[l] = c));
  index();

  // 5. Coordinates.
  const colW = layers.map((col) => Math.max(0, ...col.map((v) => w[v])));
  const colX = [];
  let acc = 0;
  for (let l = 0; l < L; l++) {
    colX.push(acc);
    acc += colW[l] + LAYOUT_GAP_X;
  }
  const sep = (a, b) => (h[a] + h[b]) / 2 + (a >= n || b >= n ? LAYOUT_GAP_Y / 2 : LAYOUT_GAP_Y);
  const y = new Float64Array(N);
  for (const col of layers) for (let i = 1; i < col.length; i++) y[col[i]] = y[col[i - 1]] + sep(col[i - 1], col[i]);

  // Isotonic regression: minimise sum wt*(y - want)^2 with y[i+1] - y[i] >= sep.
  const place = (col, want, wt) => {
    const off = [0];
    for (let i = 1; i < col.length; i++) off.push(off[i - 1] + sep(col[i - 1], col[i]));
    const blocks = []; // [mean, weight, count]
    for (let i = 0; i < col.length; i++) {
      blocks.push([want[i] - off[i], wt[i], 1]);
      while (blocks.length > 1 && blocks[blocks.length - 2][0] > blocks[blocks.length - 1][0]) {
        const b2 = blocks.pop();
        const b1 = blocks[blocks.length - 1];
        const W = b1[1] + b2[1];
        b1[0] = (b1[0] * b1[1] + b2[0] * b2[1]) / W;
        b1[1] = W;
        b1[2] += b2[2];
      }
    }
    let i = 0;
    for (const [m, , c] of blocks) for (let j = 0; j < c; j++, i++) y[col[i]] = m + off[i];
  };
  const relax = (dir) => {
    const order = dir === "up" ? [...layers.keys()].reverse() : [...layers.keys()];
    for (const l of order) {
      const col = layers[l];
      if (!col.length) continue;
      const want = [];
      const wt = [];
      for (const v of col) {
        const adj = dir === "down" ? up[v] : dir === "up" ? down[v] : up[v].concat(down[v]);
        want.push(adj.length ? adj.reduce((s, u) => s + y[u], 0) / adj.length : y[v]);
        wt.push(adj.length ? (v >= n ? 0.5 : 1) : 0.05);
      }
      place(col, want, wt);
    }
  };
  for (let r = 0; r < 4; r++) {
    relax("down");
    relax("up");
  }
  relax("both");

  // Normalise: top at 0.
  let top = Infinity;
  let bottom = -Infinity;
  for (let v = 0; v < N; v++) {
    if (v < n && !inLayout(v)) continue;
    top = Math.min(top, y[v] - h[v] / 2);
    bottom = Math.max(bottom, y[v] + h[v] / 2);
  }
  if (!Number.isFinite(top)) top = bottom = 0;
  for (let v = 0; v < N; v++) y[v] -= top;
  bottom -= top;

  const x = new Float64Array(n);
  for (let v = 0; v < n; v++) x[v] = colX[lay[v]] + w[v] / 2;

  // Isolated nodes: a grid under the graph, in input order.
  if (packIsolated) {
    const cellW = Math.max(...isolated.map((v) => w[v])) + LAYOUT_GAP_X / 2;
    const width = Math.max(acc - LAYOUT_GAP_X, cellW);
    const cols = Math.max(1, Math.min(isolated.length, Math.floor((width + LAYOUT_GAP_X / 2) / cellW)));
    const rowH = Math.max(...isolated.map((v) => h[v])) + LAYOUT_GAP_Y;
    const y0 = (bottom > 0 ? bottom + 3 * LAYOUT_GAP_Y : 0) + rowH / 2;
    isolated.forEach((v, i) => {
      x[v] = (i % cols) * cellW + w[v] / 2;
      y[v] = y0 + Math.floor(i / cols) * rowH;
    });
  }

  const paths = chains.map((chain) => {
    if (!chain) return null;
    const pts = [];
    chain.forEach((v, i) => {
      if (v < n) {
        // Leave a module on its right side, enter on its left.
        pts.push({ x: x[v] + (i === 0 ? w[v] / 2 : -w[v] / 2), y: y[v] });
      } else {
        pts.push({ x: colX[lay[v]], y: y[v] }, { x: colX[lay[v]] + colW[lay[v]], y: y[v] });
      }
    });
    return pts;
  });
  return { x: Array.from(x), y: Array.from(y.subarray(0, n)), layer: Array.from(layer), paths, crossings: bestX };
}
