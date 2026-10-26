//! Layered left-to-right layout of the import graph, a port of the web
//! viewer's viewer/src/layout.js (Sugiyama style, by hand):
//!   1. cycles: DFS back edges are reversed while layering;
//!   2. layers: longest path from the roots (a module sits one column right
//!      of its deepest importer);
//!   3. an edge spanning several columns gets a dummy node in each, so it is
//!      routed between the modules instead of through them;
//!   4. order within a column: barycenter sweeps, down and up, keeping the
//!      order with the fewest edge crossings;
//!   5. y: each node pulled to the mean y of its neighbours, without overlaps
//!      and keeping the order (isotonic regression, pool adjacent violators);
//!      x: columns as wide as their widest node, plus a gap.
//!
//! Nodes without any edge are packed into a grid under the rest. The steps
//! and their arithmetic follow the JS line by line, so both give the same
//! coordinates (tests/layout.rs compares them).

use std::collections::HashSet;

pub const GAP_X: f64 = 120.; // between columns: room for curves and priority chips
pub const GAP_Y: f64 = 12.; // between nodes in a column
const DUMMY_H: f64 = 4.;
const SWEEPS: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Node centres.
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    /// Column of each node (longest path from the roots).
    pub layer: Vec<usize>,
    /// Per edge: the points of its route, left to right (a node's right
    /// side, then a pair per dummy column, then the target's left side);
    /// `None` for self loops.
    pub paths: Vec<Option<Vec<Point>>>,
    pub crossings: usize,
}

pub fn layered_layout(sizes: &[Size], edges: &[(usize, usize)]) -> Layout {
    let n = sizes.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(a, b) in edges {
        if a != b {
            out[a].push(b);
        }
    }

    // 1. Back edges (to a node on the DFS stack) point backwards.
    let mut state = vec![0u8; n]; // 0 new, 1 on stack, 2 done
    let mut back: HashSet<(usize, usize)> = HashSet::new();
    for r in 0..n {
        if state[r] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(r, 0)];
        state[r] = 1;
        while let Some(top) = stack.last_mut() {
            let (u, i) = *top;
            if i < out[u].len() {
                top.1 += 1;
                let v = out[u][i];
                if state[v] == 1 {
                    back.insert((u, v));
                } else if state[v] == 0 {
                    state[v] = 1;
                    stack.push((v, 0));
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }
    let mut dag: Vec<(usize, usize, usize)> = Vec::new(); // from, to, edge index
    for (k, &(a, b)) in edges.iter().enumerate() {
        if a == b {
            continue;
        }
        dag.push(if back.contains(&(a, b)) { (b, a, k) } else { (a, b, k) });
    }

    // 2. Longest path from the roots (Kahn order).
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for &(a, b, _) in &dag {
        succ[a].push(b);
        indeg[b] += 1;
    }
    let mut layer = vec![0usize; n];
    let mut queue: Vec<usize> = (0..n).filter(|&v| indeg[v] == 0).collect();
    let mut qi = 0;
    while qi < queue.len() {
        let u = queue[qi];
        qi += 1;
        for &v in &succ[u] {
            layer[v] = layer[v].max(layer[u] + 1);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push(v);
            }
        }
    }

    let mut connected = vec![false; n];
    for &(a, b, _) in &dag {
        connected[a] = true;
        connected[b] = true;
    }
    let isolated: Vec<usize> = (0..n).filter(|&v| !connected[v] && n > 1).collect();
    let pack_isolated = isolated.len() > 6 && isolated.len() < n;
    let in_layout = |v: usize| !pack_isolated || connected[v];

    // 3. Dummy nodes; chains[k]: node ids along edge k, left to right.
    let nl = layer.iter().copied().max().unwrap_or(0) + 1;
    let mut h: Vec<f64> = sizes.iter().map(|s| s.h).collect();
    let mut w: Vec<f64> = sizes.iter().map(|s| s.w).collect();
    let mut lay: Vec<usize> = layer.clone();
    let mut up: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut down: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut chains: Vec<Option<Vec<usize>>> = vec![None; edges.len()];
    for &(a, b, k) in &dag {
        let mut chain = vec![a];
        for l in lay[a] + 1..lay[b] {
            let d = lay.len();
            lay.push(l);
            h.push(DUMMY_H);
            w.push(0.);
            up.push(Vec::new());
            down.push(Vec::new());
            let last = *chain.last().unwrap();
            down[last].push(d);
            up[d].push(last);
            chain.push(d);
        }
        let last = *chain.last().unwrap();
        down[last].push(b);
        up[b].push(last);
        chain.push(b);
        chains[k] = Some(chain);
    }
    let nn = lay.len();

    // Initial order: depth-first from the roots, in input order.
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); nl];
    let mut seen = vec![false; nn];
    let visit = |r: usize, seen: &mut Vec<bool>, layers: &mut Vec<Vec<usize>>| {
        let mut stack = vec![r];
        while let Some(u) = stack.pop() {
            if seen[u] {
                continue;
            }
            seen[u] = true;
            layers[lay[u]].push(u);
            for &c in down[u].iter().rev() {
                if !seen[c] {
                    stack.push(c);
                }
            }
        }
    };
    for (v, ups) in up.iter().enumerate().take(n) {
        if in_layout(v) && ups.is_empty() {
            visit(v, &mut seen, &mut layers);
        }
    }
    for v in 0..nn {
        if !seen[v] && (v >= n || in_layout(v)) {
            visit(v, &mut seen, &mut layers);
        }
    }

    // 4. Barycenter sweeps.
    let mut pos = vec![0f64; nn];
    let index = |layers: &Vec<Vec<usize>>, pos: &mut Vec<f64>| {
        for col in layers {
            for (i, &v) in col.iter().enumerate() {
                pos[v] = i as f64;
            }
        }
    };
    index(&layers, &mut pos);
    let crossings = |layers: &Vec<Vec<usize>>, pos: &Vec<f64>| -> usize {
        let mut total = 0usize;
        for l in 0..nl.saturating_sub(1) {
            let mut segs: Vec<(f64, f64)> = Vec::new();
            for &u in &layers[l] {
                for &v in &down[u] {
                    segs.push((pos[u], pos[v]));
                }
            }
            segs.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.total_cmp(&q.1)));
            // Inversions in the lower ends (Fenwick tree).
            let size = layers[l + 1].len() + 1;
            let mut tree = vec![0usize; size + 1];
            for (seen_so_far, &(_, b)) in segs.iter().enumerate() {
                let b = b as usize;
                let mut s = 0;
                let mut i = b + 1;
                while i > 0 {
                    s += tree[i];
                    i -= i.isolate_lowest_one();
                }
                total += seen_so_far - s;
                let mut i = b + 1;
                while i <= size {
                    tree[i] += 1;
                    i += i.isolate_lowest_one();
                }
            }
        }
        total
    };
    let mut best = layers.clone();
    let mut best_x = crossings(&layers, &pos);
    let mut key = vec![0f64; nn];
    let mut it = 0;
    while it < SWEEPS && best_x > 0 {
        let downward = it % 2 == 0;
        for s in 1..nl {
            let l = if downward { s } else { nl - 1 - s };
            let nb = if downward { &up } else { &down };
            let col = &mut layers[l];
            for &v in col.iter() {
                let adj = &nb[v];
                key[v] = if adj.is_empty() {
                    pos[v]
                } else {
                    adj.iter().fold(0., |acc, &u| acc + pos[u]) / adj.len() as f64
                };
            }
            let key_of = |v: usize| key[v];
            col.sort_by(|&a, &b| {
                let d = key_of(a) - key_of(b);
                if d != 0. {
                    d.total_cmp(&0.)
                } else {
                    (pos[a] - pos[b]).total_cmp(&0.)
                }
            });
            for (i, &v) in col.iter().enumerate() {
                pos[v] = i as f64;
            }
        }
        let x = crossings(&layers, &pos);
        if x < best_x {
            best_x = x;
            best = layers.clone();
        }
        it += 1;
    }
    layers = best;
    index(&layers, &mut pos);

    // 5. Coordinates.
    let col_w: Vec<f64> = layers.iter().map(|col| col.iter().fold(0f64, |m, &v| m.max(w[v]))).collect();
    let mut col_x = Vec::with_capacity(nl);
    let mut acc = 0.;
    for cw in &col_w {
        col_x.push(acc);
        acc += cw + GAP_X;
    }
    let sep = |a: usize, b: usize| (h[a] + h[b]) / 2. + if a >= n || b >= n { GAP_Y / 2. } else { GAP_Y };
    let mut y = vec![0f64; nn];
    for col in &layers {
        for i in 1..col.len() {
            y[col[i]] = y[col[i - 1]] + sep(col[i - 1], col[i]);
        }
    }

    // Isotonic regression: minimise sum wt*(y - want)^2 with y[i+1] - y[i] >= sep.
    let place = |col: &[usize], want: &[f64], wt: &[f64], y: &mut Vec<f64>| {
        let mut off = vec![0.];
        for i in 1..col.len() {
            off.push(off[i - 1] + sep(col[i - 1], col[i]));
        }
        let mut blocks: Vec<(f64, f64, usize)> = Vec::new(); // mean, weight, count
        for i in 0..col.len() {
            blocks.push((want[i] - off[i], wt[i], 1));
            while blocks.len() > 1 && blocks[blocks.len() - 2].0 > blocks[blocks.len() - 1].0 {
                let b2 = blocks.pop().unwrap();
                let b1 = blocks.last_mut().unwrap();
                let ww = b1.1 + b2.1;
                b1.0 = (b1.0 * b1.1 + b2.0 * b2.1) / ww;
                b1.1 = ww;
                b1.2 += b2.2;
            }
        }
        let mut i = 0;
        for (m, _, c) in blocks {
            for _ in 0..c {
                y[col[i]] = m + off[i];
                i += 1;
            }
        }
    };
    #[derive(Clone, Copy, PartialEq)]
    enum Dir {
        Down,
        Up,
        Both,
    }
    let relax = |dir: Dir, y: &mut Vec<f64>| {
        let order: Vec<usize> = if dir == Dir::Up { (0..nl).rev().collect() } else { (0..nl).collect() };
        for l in order {
            let col = &layers[l];
            if col.is_empty() {
                continue;
            }
            let mut want = Vec::with_capacity(col.len());
            let mut wt = Vec::with_capacity(col.len());
            for &v in col {
                let adj: Vec<usize> = match dir {
                    Dir::Down => up[v].clone(),
                    Dir::Up => down[v].clone(),
                    Dir::Both => up[v].iter().chain(down[v].iter()).copied().collect(),
                };
                want.push(if adj.is_empty() {
                    y[v]
                } else {
                    adj.iter().fold(0., |s, &u| s + y[u]) / adj.len() as f64
                });
                wt.push(if adj.is_empty() {
                    0.05
                } else if v >= n {
                    0.5
                } else {
                    1.
                });
            }
            place(col, &want, &wt, y);
        }
    };
    for _ in 0..4 {
        relax(Dir::Down, &mut y);
        relax(Dir::Up, &mut y);
    }
    relax(Dir::Both, &mut y);

    // Normalise: top at 0.
    let mut top = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for v in 0..nn {
        if v < n && !in_layout(v) {
            continue;
        }
        top = top.min(y[v] - h[v] / 2.);
        bottom = bottom.max(y[v] + h[v] / 2.);
    }
    if !top.is_finite() {
        top = 0.;
        bottom = 0.;
    }
    for v in y.iter_mut() {
        *v -= top;
    }
    bottom -= top;

    let mut x: Vec<f64> = (0..n).map(|v| col_x[lay[v]] + w[v] / 2.).collect();

    // Isolated nodes: a grid under the graph, in input order.
    if pack_isolated {
        let cell_w = isolated.iter().fold(f64::NEG_INFINITY, |m, &v| m.max(w[v])) + GAP_X / 2.;
        let width = (acc - GAP_X).max(cell_w);
        let cols = ((width + GAP_X / 2.) / cell_w).floor().min(isolated.len() as f64).max(1.) as usize;
        let row_h = isolated.iter().fold(f64::NEG_INFINITY, |m, &v| m.max(h[v])) + GAP_Y;
        let y0 = (if bottom > 0. { bottom + 3. * GAP_Y } else { 0. }) + row_h / 2.;
        for (i, &v) in isolated.iter().enumerate() {
            x[v] = (i % cols) as f64 * cell_w + w[v] / 2.;
            y[v] = y0 + (i / cols) as f64 * row_h;
        }
    }

    let paths = chains
        .iter()
        .map(|chain| {
            let chain = chain.as_ref()?;
            let mut pts = Vec::new();
            for (i, &v) in chain.iter().enumerate() {
                if v < n {
                    // Leave a module on its right side, enter on its left.
                    pts.push(Point { x: x[v] + if i == 0 { w[v] / 2. } else { -w[v] / 2. }, y: y[v] });
                } else {
                    pts.push(Point { x: col_x[lay[v]], y: y[v] });
                    pts.push(Point { x: col_x[lay[v]] + col_w[lay[v]], y: y[v] });
                }
            }
            Some(pts)
        })
        .collect();
    y.truncate(n);
    Layout { x, y, layer, paths, crossings: best_x }
}
