//! The module import graph (centre pane), after viewer/src/graph.js:
//! layered left-to-right layout (optgraph_core::layout), modules as pills,
//! curved edges, pan (drag) and zoom (wheel), hit-testing, a hover tooltip,
//! and an eased camera. Selecting an option rings its defining modules
//! (winner, lost, mkIf false) with priority chips, dims the rest in tiers
//! and fits the camera to them and their import ancestors; defining
//! modules are drawn at least at 12 px labels (pinned), and ancestors that
//! don't fit get chips at the viewport edge.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, BorderStyle, Bounds, BoxShadow, Corners, Font, FontWeight, Hsla, PathBuilder, Pixels, SharedString, TextRun,
    Window, point, px, quad, size,
};
use optgraph_core::layout::{Point as LPoint, Size, layered_layout};
use optgraph_core::{Model, ModuleStatus, OriginClass, Status, module_short_label, ring_text};

use crate::theme::{FONT_MONO, FONT_UI, Theme};

/// Above this many modules only non-nixpkgs ones are drawn.
pub const GRAPH_LIMIT: usize = 300;
const PILL_H: f64 = 28.;
const PILL_PAD: f64 = 12.;
const CHIP_H: f64 = 20.;
const CAMERA_MS: f64 = 380.;
const FADE_MS: f64 = 180.;
/// Pinned modules are drawn at least at READ_K, so their labels stay 12 px.
const READ_K: f64 = 1.;
/// A frame zoomed out below this shows too little.
const PIN_FLOOR: f64 = 0.2;
const MIN_K: f64 = 0.12;
const MAX_K: f64 = 4.;
/// Pixels kept free at the bottom when fitting (the legend).
const RESERVE_BOTTOM: f64 = 44.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam {
    pub x: f64,
    pub y: f64,
    pub k: f64,
}

#[derive(Clone, Debug)]
pub struct Node {
    /// Index in graph.json's modules.
    pub module: usize,
    pub label: SharedString,
    pub class: OriginClass,
    pub disabled: bool,
    /// Left edge, centre line, width (world units).
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub source: usize,
    pub target: usize,
    pub dashed: bool,
    pub pts: Vec<LPoint>,
}

/// The laid-out graph: fixed for a model.
pub struct GraphData {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub by_module: HashMap<usize, usize>,
    /// Per node: indices of the links into it.
    pub parents: Vec<Vec<usize>>,
    pub banner: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct CamAnim {
    from: Cam,
    to: Cam,
    at: Instant,
}

/// A defining module's ring and chip.
#[derive(Clone, Debug)]
pub struct Ring {
    pub status: Status,
    pub chip: SharedString,
    pub chip_w: f64,
}

#[derive(Clone)]
pub struct EdgeChip {
    pub node: usize,
    pub status: Option<Status>,
    pub x: f64,
    pub y: f64,
    pub text: SharedString,
}

/// What a frame draws: the camera and the selection at that moment.
#[derive(Clone)]
pub struct Frame {
    pub data: Rc<GraphData>,
    pub cam: Cam,
    pub fade: f64,
    pub rings: Rc<HashMap<usize, Ring>>,
    pub lit: Option<Rc<HashSet<usize>>>,
    pub pins: Rc<HashSet<usize>>,
    pub path: Option<Rc<HashSet<usize>>>,
    pub via: Option<Rc<HashSet<usize>>>,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
}

pub struct Drag {
    pub start: (f64, f64),
    pub cam: Cam,
    pub moved: bool,
}

pub struct GraphState {
    pub data: Option<Rc<GraphData>>,
    pub cam: Cam,
    anim: Option<CamAnim>,
    /// Defining modules of the selected option (node -> ring).
    pub rings: Rc<HashMap<usize, Ring>>,
    /// A selected module and its neighbours.
    pub lit: Option<Rc<HashSet<usize>>>,
    pub pins: Rc<HashSet<usize>>,
    /// Links on the import paths to what is selected, and their modules.
    pub path: Option<Rc<HashSet<usize>>>,
    pub via: Option<Rc<HashSet<usize>>>,
    fade_at: Instant,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub drag: Option<Drag>,
    /// Stage size in pixels (from the window layout).
    pub width: f64,
    pub height: f64,
    /// A camera move waiting for the stage size.
    pending: Option<Goal>,
    pub reduced_motion: bool,
}

#[derive(Clone)]
enum Goal {
    Fit,
    Frame {
        core: Vec<usize>,
        context: Vec<usize>,
        max_k: f64,
    },
}

impl Default for GraphState {
    fn default() -> Self {
        GraphState {
            data: None,
            cam: Cam { x: 0., y: 0., k: 1. },
            anim: None,
            rings: Rc::default(),
            lit: None,
            pins: Rc::default(),
            path: None,
            via: None,
            fade_at: Instant::now() - Duration::from_secs(10),
            hover: None,
            selected: None,
            drag: None,
            width: 0.,
            height: 0.,
            pending: None,
            reduced_motion: false,
        }
    }
}

fn ui_font(weight: FontWeight) -> Font {
    Font {
        weight,
        ..gpui::font(FONT_UI)
    }
}

fn mono_font() -> Font {
    Font {
        weight: FontWeight::SEMIBOLD,
        ..gpui::font(FONT_MONO)
    }
}

fn text_width(window: &Window, text: &str, font: Font, size: f64) -> f64 {
    let run = TextRun {
        len: text.len(),
        font,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .shape_line(SharedString::from(text.to_string()), px(size as f32), &[run], None)
            .width,
    ) as f64
}

const fn ease(p: f64) -> f64 {
    if p < 0.5 {
        4. * p * p * p
    } else {
        1. - (-2. * p + 2.) * (-2. * p + 2.) * (-2. * p + 2.) / 2.
    }
}

impl GraphState {
    /// Lays out the model's modules (all, or the non-nixpkgs ones above
    /// GRAPH_LIMIT).
    pub fn set_model(&mut self, model: &Model, window: &Window) {
        let all = model.modules();
        let too_many = all.len() > GRAPH_LIMIT;
        let shown: Vec<usize> = (0..all.len())
            .filter(|&i| !too_many || all[i].origin.is_userish())
            .collect();
        let banner = too_many.then(|| {
            format!(
                "{} modules are too many to draw: showing the {} non-nixpkgs modules only. Search and option details cover everything.",
                optgraph_core::text::thousands(all.len() as u64),
                optgraph_core::text::thousands(shown.len() as u64)
            )
        });
        let mut by_module = HashMap::new();
        let mut nodes: Vec<Node> = shown
            .iter()
            .enumerate()
            .map(|(i, &mi)| {
                let m = &all[mi];
                by_module.insert(mi, i);
                let label = module_short_label(m);
                let w = text_width(window, &label, ui_font(FontWeight::MEDIUM), 12.).ceil() + 2. * PILL_PAD + 14.;
                Node {
                    module: mi,
                    label: label.into(),
                    class: m.origin.class(),
                    disabled: m.disabled,
                    x: 0.,
                    y: 0.,
                    w,
                }
            })
            .collect();
        let mut links: Vec<Link> = Vec::new();
        for (i, &mi) in shown.iter().enumerate() {
            for t in &all[mi].imports {
                if let Some(&tn) = model.mod_index.get(t).and_then(|ti| by_module.get(ti)) {
                    links.push(Link {
                        source: i,
                        target: tn,
                        dashed: all[nodes[tn].module].disabled,
                        pts: vec![],
                    });
                }
            }
            // A module inside a submodule instance hangs off the module whose
            // definition it is (the module system imports it into the instance).
            if let Some(on) = all[mi]
                .owner
                .as_ref()
                .and_then(|o| model.mod_index.get(o))
                .and_then(|oi| by_module.get(oi))
            {
                links.push(Link {
                    source: *on,
                    target: i,
                    dashed: false,
                    pts: vec![],
                });
            }
        }
        let lay = layered_layout(
            &nodes.iter().map(|n| Size { w: n.w, h: PILL_H }).collect::<Vec<_>>(),
            &links.iter().map(|l| (l.source, l.target)).collect::<Vec<_>>(),
        );
        for (i, n) in nodes.iter_mut().enumerate() {
            n.x = lay.x[i] - n.w / 2.;
            n.y = lay.y[i];
        }
        for (k, l) in links.iter_mut().enumerate() {
            l.pts = lay.paths[k].clone().unwrap_or_default();
        }
        let mut parents = vec![Vec::new(); nodes.len()];
        for (k, l) in links.iter().enumerate() {
            parents[l.target].push(k);
        }
        self.data = Some(Rc::new(GraphData {
            nodes,
            links,
            by_module,
            parents,
            banner,
        }));
        self.clear();
        self.goal(Goal::Fit, false);
    }

    fn clear(&mut self) {
        self.rings = Rc::default();
        self.lit = None;
        self.pins = Rc::default();
        self.path = None;
        self.via = None;
        self.selected = None;
        self.hover = None;
    }

    pub fn dimming(&self) -> bool {
        !self.rings.is_empty() || self.lit.is_some()
    }

    fn start_fade(&mut self, was_dim: bool) {
        if !was_dim {
            self.fade_at = Instant::now();
        }
    }

    /// Nothing selected: everything lit, the camera fits the graph.
    pub fn select_none(&mut self) {
        self.clear();
        self.goal(Goal::Fit, true);
    }

    /// A selected option: rings on its defining modules, their import
    /// paths lit, the camera on them.
    pub fn select_option(&mut self, defining: &[(usize, ModuleStatus)], window: &Window) {
        let Some(data) = self.data.clone() else { return };
        let was_dim = self.dimming();
        self.clear();
        let mut rings = HashMap::new();
        let mut core = Vec::new();
        for (mi, s) in defining {
            let Some(&n) = data.by_module.get(mi) else { continue };
            let chip: SharedString = ring_text(s).into();
            let chip_w = text_width(window, &chip, mono_font(), 11.).ceil() + 16.;
            rings.insert(
                n,
                Ring {
                    status: s.status,
                    chip,
                    chip_w,
                },
            );
            core.push(n);
        }
        if rings.is_empty() {
            self.goal(Goal::Fit, true);
            return;
        }
        // Importers back to the roots.
        let mut path = HashSet::new();
        let mut todo: Vec<usize> = core.clone();
        let mut done: HashSet<usize> = core.iter().copied().collect();
        while let Some(n) = todo.pop() {
            for &k in &data.parents[n] {
                path.insert(k);
                let p = data.links[k].source;
                if done.insert(p) {
                    todo.push(p);
                }
            }
        }
        self.pins = Rc::new(core.iter().copied().collect());
        self.rings = Rc::new(rings);
        self.path = Some(Rc::new(path));
        let mut context: Vec<usize> = done.iter().copied().collect();
        context.sort();
        self.via = Some(Rc::new(done));
        let max_k = if self.width < 600. { 1.1 } else { 1.5 };
        self.start_fade(was_dim);
        self.goal(Goal::Frame { core, context, max_k }, true);
    }

    /// A selected module: it and its direct imports and importers are lit.
    pub fn select_module(&mut self, mi: usize) {
        let Some(data) = self.data.clone() else { return };
        let was_dim = self.dimming();
        self.clear();
        let Some(&n) = data.by_module.get(&mi) else {
            self.goal(Goal::Fit, true);
            return;
        };
        let mut lit = HashSet::from([n]);
        let mut path = HashSet::new();
        for (k, l) in data.links.iter().enumerate() {
            if l.source == n || l.target == n {
                lit.insert(l.source);
                lit.insert(l.target);
                path.insert(k);
            }
        }
        let mut context: Vec<usize> = lit.iter().copied().collect();
        context.sort();
        self.selected = Some(n);
        self.pins = Rc::new(HashSet::from([n]));
        self.lit = Some(Rc::new(lit));
        self.path = Some(Rc::new(path));
        self.start_fade(was_dim);
        self.goal(
            Goal::Frame {
                core: vec![n],
                context,
                max_k: 1.3,
            },
            true,
        );
    }

    fn goal(&mut self, g: Goal, animate: bool) {
        if self.width <= 0. {
            self.pending = Some(g);
            return;
        }
        let target = self.resolve(&g);
        self.move_to(target, animate);
    }

    fn resolve(&self, g: &Goal) -> Cam {
        let Some(data) = &self.data else { return self.cam };
        match g {
            Goal::Fit => self.view(&(0..data.nodes.len()).collect::<Vec<_>>(), 1.3, &[]),
            Goal::Frame { core, context, max_k } => self.frame(core, context, *max_k),
        }
    }

    /// The stage size; runs a camera move that waited for it.
    pub fn set_size(&mut self, w: f64, h: f64) {
        let changed = (w - self.width).abs() > 0.5 || (h - self.height).abs() > 0.5;
        self.width = w;
        self.height = h;
        if let Some(g) = self.pending.take() {
            self.cam = self.resolve(&g);
            self.anim = None;
        } else if changed && self.anim.is_none() && !self.dimming() && self.drag.is_none() {
            // Nothing selected: keep the whole graph fitted while the window resizes.
            self.cam = self.resolve(&Goal::Fit);
        }
    }

    pub fn move_to(&mut self, target: Cam, animate: bool) {
        self.step(Instant::now());
        if animate && !self.reduced_motion {
            self.anim = Some(CamAnim {
                from: self.cam,
                to: target,
                at: Instant::now(),
            });
        } else {
            self.anim = None;
            self.cam = target;
        }
    }

    pub fn stop_camera(&mut self) {
        self.step(Instant::now());
        self.anim = None;
    }

    /// Camera and fade at `now`; true while either moves.
    pub fn step(&mut self, now: Instant) -> bool {
        let mut moving = false;
        if let Some(a) = self.anim {
            let p = ((now - a.at).as_secs_f64() * 1000. / CAMERA_MS).clamp(0., 1.);
            let e = ease(p);
            let (from, to) = (a.from, a.to);
            // Zoom interpolates geometrically; the world point at the centre follows.
            let k = from.k * (to.k / from.k).powf(e);
            let (cx, cy) = (self.width / 2., self.height / 2.);
            let (fx, fy) = ((cx - from.x) / from.k, (cy - from.y) / from.k);
            let wx = fx + ((cx - to.x) / to.k - fx) * e;
            let wy = fy + ((cy - to.y) / to.k - fy) * e;
            if p >= 1. {
                self.cam = to;
                self.anim = None;
            } else {
                self.cam = Cam {
                    k,
                    x: cx - wx * k,
                    y: cy - wy * k,
                };
                moving = true;
            }
        }
        moving || self.fade(now) < 1.
    }

    fn fade(&self, now: Instant) -> f64 {
        if self.reduced_motion {
            1.
        } else {
            ((now - self.fade_at).as_secs_f64() * 1000. / FADE_MS).clamp(0., 1.)
        }
    }

    pub fn frame_at(&mut self, now: Instant) -> Option<(Frame, bool)> {
        let moving = self.step(now);
        let data = self.data.clone()?;
        Some((
            Frame {
                data,
                cam: self.cam,
                fade: if self.dimming() { self.fade(now) } else { 0. },
                rings: self.rings.clone(),
                lit: self.lit.clone(),
                pins: self.pins.clone(),
                path: self.path.clone(),
                via: self.via.clone(),
                hover: self.hover,
                selected: self.selected,
            },
            moving,
        ))
    }

    fn chip_w(&self, n: usize) -> f64 {
        self.rings.get(&n).map(|r| r.chip_w + 8.).unwrap_or(0.)
    }

    fn banner_h(&self) -> f64 {
        if self.data.as_ref().is_some_and(|d| d.banner.is_some()) {
            56.
        } else {
            0.
        }
    }

    /// World bounds of `nodes`, those in `pinned` scaled by s about their centre.
    fn bounds(&self, nodes: &[usize], pinned: &HashSet<usize>, s: f64) -> (f64, f64, f64, f64) {
        let data = self.data.as_ref().unwrap();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &i in nodes {
            let n = &data.nodes[i];
            let f = if pinned.contains(&i) { s } else { 1. };
            let cx = n.x + n.w / 2.;
            x0 = x0.min(cx - n.w / 2. * f);
            y0 = y0.min(n.y - PILL_H / 2. * f);
            x1 = x1.max(cx + (n.w / 2. + self.chip_w(i)) * f);
            y1 = y1.max(n.y + PILL_H / 2. * f);
        }
        (x0, y0, x1, y1)
    }

    /// Camera that shows `nodes`, zoomed in at most to max_k. Below READ_K
    /// the `pinned` ones are drawn larger, so the frame makes room for them.
    fn view(&self, nodes: &[usize], max_k: f64, pinned: &[usize]) -> Cam {
        if nodes.is_empty() {
            return self.cam;
        }
        let pad = (self.width * 0.07).clamp(16., 56.);
        let ww = self.width - 2. * pad;
        let top = self.banner_h();
        let hh = self.height - 2. * pad - top - RESERVE_BOTTOM;
        let fit_k = |b: (f64, f64, f64, f64)| {
            (ww / (b.2 - b.0).max(1.))
                .min(hh / (b.3 - b.1).max(1.))
                .min(max_k)
                .max(MIN_K)
        };
        let pin_set: HashSet<usize> = pinned.iter().copied().collect();
        let mut b = self.bounds(nodes, &HashSet::new(), 1.);
        let mut k = fit_k(b);
        for _ in 0..3 {
            if pin_set.is_empty() || k >= READ_K {
                break;
            }
            b = self.bounds(nodes, &pin_set, READ_K / k);
            k = fit_k(b);
        }
        Cam {
            k,
            x: pad + (ww - (b.2 - b.0) * k) / 2. - b.0 * k,
            y: pad + top + (hh - (b.3 - b.1) * k) / 2. - b.1 * k,
        }
    }

    /// Do the pins of `nodes` that are in view overlap at camera v?
    fn pins_apart(&self, nodes: &[usize], v: Cam) -> bool {
        let data = self.data.as_ref().unwrap();
        let s = v.k.max(READ_K);
        let h = PILL_H * s;
        let mut r: Vec<(f64, f64, f64)> = nodes
            .iter()
            .map(|&i| {
                let n = &data.nodes[i];
                let cx = v.x + (n.x + n.w / 2.) * v.k;
                (cx - n.w / 2. * s, cx + (n.w / 2. + self.chip_w(i)) * s, v.y + n.y * v.k)
            })
            .filter(|p| p.1 > 0. && p.0 < self.width && p.2 > -h && p.2 < self.height + h)
            .collect();
        r.sort_by(|a, b| a.2.total_cmp(&b.2));
        for i in 0..r.len() {
            let mut j = i + 1;
            while j < r.len() && r[j].2 - r[i].2 < h {
                if r[j].0 < r[i].1 && r[i].0 < r[j].1 {
                    return false;
                }
                j += 1;
            }
        }
        true
    }

    /// Camera for a selection. `core` (pinned, strongest first) must stay
    /// readable: at READ_K or closer, or at PIN_FLOOR or closer with pins
    /// that do not overlap. Tries `context` (core plus its import paths),
    /// then the core alone, then as many of the core, in order, as fit.
    fn frame(&self, core: &[usize], context: &[usize], max_k: f64) -> Cam {
        let ok = |v: Cam| v.k >= READ_K || (v.k >= PIN_FLOOR && self.pins_apart(core, v));
        for set in [context, core] {
            let v = self.view(set, max_k, core);
            if ok(v) {
                return v;
            }
        }
        let mut shown = vec![core[0]];
        let mut best = self.view(&shown, max_k, &shown);
        for &n in &core[1..] {
            let mut next = shown.clone();
            next.push(n);
            let v = self.view(&next, max_k, &next);
            if ok(v) {
                shown = next;
                best = v;
            }
        }
        best
    }

    pub fn fit(&mut self) {
        self.goal(Goal::Fit, true);
    }

    /// Zoom by a factor around the centre of the stage.
    pub fn zoom_by(&mut self, f: f64) {
        let t = self.anim.map(|a| a.to).unwrap_or(self.cam);
        let k = (t.k * f).clamp(MIN_K, MAX_K);
        let (cx, cy) = (self.width / 2., self.height / 2.);
        self.move_to(
            Cam {
                k,
                x: cx - (cx - t.x) / t.k * k,
                y: cy - (cy - t.y) / t.k * k,
            },
            true,
        );
    }

    /// Wheel zoom about the pointer.
    pub fn wheel(&mut self, px_: f64, py: f64, dy: f64, fine: bool) {
        self.stop_camera();
        let k = (self.cam.k * (-dy * if fine { 0.01 } else { 0.0015 }).exp()).clamp(MIN_K, MAX_K);
        let (wx, wy) = ((px_ - self.cam.x) / self.cam.k, (py - self.cam.y) / self.cam.k);
        self.cam = Cam {
            k,
            x: px_ - wx * k,
            y: py - wy * k,
        };
    }

    /// Centre a node, keeping the zoom.
    pub fn reveal(&mut self, n: usize) {
        let Some(data) = &self.data else { return };
        let node = &data.nodes[n];
        let k = self.cam.k;
        let target = Cam {
            k,
            x: self.width / 2. - (node.x + node.w / 2.) * k,
            y: self.height / 2. - node.y * k,
        };
        self.move_to(target, true);
    }

    pub fn pin_scale(&self, n: usize) -> f64 {
        if self.pins.contains(&n) {
            (READ_K / self.cam.k).max(1.)
        } else {
            1.
        }
    }

    /// The node under a stage point; pins first, as they are drawn on top.
    pub fn node_at(&self, px_: f64, py: f64) -> Option<usize> {
        let data = self.data.as_ref()?;
        let (x, y) = ((px_ - self.cam.x) / self.cam.k, (py - self.cam.y) / self.cam.k);
        let slack = 3. / self.cam.k;
        let hit = |i: usize, s: f64| {
            let n = &data.nodes[i];
            (x - (n.x + n.w / 2.)).abs() <= n.w / 2. * s + slack && (y - n.y).abs() <= PILL_H / 2. * s + slack
        };
        let mut pins: Vec<usize> = self.pins.iter().copied().collect();
        pins.sort();
        for i in pins {
            if hit(i, self.pin_scale(i)) {
                return Some(i);
            }
        }
        (0..data.nodes.len()).rev().find(|&i| hit(i, 1.))
    }

    /// Screen rectangle (stage coordinates) of a node's pill.
    pub fn node_rect(&self, n: usize) -> (f64, f64, f64, f64) {
        let data = self.data.as_ref().unwrap();
        let node = &data.nodes[n];
        let k = self.cam.k * self.pin_scale(n);
        let cx = self.cam.x + (node.x + node.w / 2.) * self.cam.k;
        let cy = self.cam.y + node.y * self.cam.k;
        (cx - node.w / 2. * k, cy - PILL_H / 2. * k, node.w * k, PILL_H * k)
    }

    /// Chips at the viewport edge for the modules of the selected option's
    /// import paths (its defining modules and their ancestors) that are out
    /// of view: an arrow towards each and its name, a defining module's
    /// chip in its status colour with its ring text; a click brings it into
    /// view.
    pub fn edge_chips(&self) -> Vec<EdgeChip> {
        let (Some(data), Some(via)) = (&self.data, &self.via) else {
            return vec![];
        };
        if self.rings.is_empty() || self.anim.is_some() {
            return vec![];
        }
        let margin = 10.;
        // Bottom chips stay above the legend.
        let (w, h) = (self.width, self.height - RESERVE_BOTTOM - 36.);
        let mut out: Vec<EdgeChip> = Vec::new();
        let pins: Vec<(f64, f64, f64, f64)> = self
            .pins
            .iter()
            .map(|&p| {
                let (x, y, rw, rh) = self.node_rect(p);
                (
                    x - 6.,
                    y - 6.,
                    x + rw + (self.chip_w(p) + 6.) * self.cam.k * self.pin_scale(p),
                    y + rh + 6.,
                )
            })
            .collect();
        let mut ids: Vec<usize> = via.iter().copied().collect();

        ids.sort_by(|a, b| data.nodes[*a].y.total_cmp(&data.nodes[*b].y));
        for n in ids {
            let (x, y, rw, rh) = self.node_rect(n);
            let (cx, cy) = (x + rw / 2., y + rh / 2.);
            let off_x = if x + rw < 0. {
                -1
            } else if x > w {
                1
            } else {
                0
            };
            let off_y = if y + rh < self.banner_h() {
                -1
            } else if y > h {
                1
            } else {
                0
            };
            if off_x == 0 && off_y == 0 {
                continue;
            }
            let arrow = match (off_x, off_y) {
                (-1, 0) => "←",
                (1, 0) => "→",
                (0, -1) => "↑",
                (0, 1) => "↓",
                (-1, -1) => "↖",
                (1, -1) => "↗",
                (-1, 1) => "↙",
                _ => "↘",
            };
            let ring = self.rings.get(&n);
            let text: SharedString = match ring {
                Some(r) => format!("{arrow} {} · {}", data.nodes[n].label, r.chip),
                None => format!("{arrow} {}", data.nodes[n].label),
            }
            .into();
            let est_w = 14. + 7. * text.chars().count() as f64;
            let ex = match off_x {
                -1 => margin,
                1 => w - margin - est_w,
                _ => (cx - est_w / 2.).clamp(margin, w - margin - est_w),
            };
            let mut ey = match off_y {
                -1 => margin + self.banner_h(),
                1 => h - margin - 24.,
                _ => (cy - 12.).clamp(margin + self.banner_h(), h - margin - 24.),
            };
            // Stack chips that would overlap each other or a pinned module
            // (bottom ones upwards).
            let step = if off_y == 1 { -26. } else { 26. };
            let hits_pin = |ey: f64| {
                pins.iter()
                    .any(|&(px0, py0, px1, py1)| ex < px1 && ex + est_w > px0 && ey < py1 && ey + 24. > py0)
            };
            let mut tries = 0;
            while tries < 40
                && (out
                    .iter()
                    .any(|c| (c.y - ey).abs() < 26. && (c.x - ex).abs() < est_w.max(60.))
                    || hits_pin(ey))
            {
                ey += step;
                tries += 1;
            }
            out.push(EdgeChip {
                node: n,
                x: ex,
                y: ey,
                text,
                status: ring.map(|r| r.status),
            });
        }
        out
    }
}

/// Paints a frame of the graph into `bounds` (the stage).
pub fn paint(frame: &Frame, t: &Theme, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let data = &frame.data;
    let cam = frame.cam;
    let ox = f64::from(f32::from(bounds.origin.x));
    let oy = f64::from(f32::from(bounds.origin.y));
    let (bw, bh) = (
        f64::from(f32::from(bounds.size.width)),
        f64::from(f32::from(bounds.size.height)),
    );
    let sp = |x: f64, y: f64| point(px((ox + cam.x + x * cam.k) as f32), px((oy + cam.y + y * cam.k) as f32));
    let dimming = !frame.rings.is_empty() || frame.lit.is_some();
    let f = frame.fade;
    let dim_a = 1. - 0.7 * f;

    // Dot grid: a texture of the stage, fixed while the graph moves.
    let mut gy = 12.;
    while gy < bh {
        let mut gx = 12.;
        while gx < bw {
            window.paint_quad(quad(
                Bounds::new(
                    point(px((ox + gx) as f32), px((oy + gy) as f32)),
                    size(px(1.5), px(1.5)),
                ),
                px(0.75),
                t.grid,
                px(0.),
                t.grid,
                BorderStyle::Solid,
            ));
            gx += 24.;
        }
        gy += 24.;
    }

    // Edges: the rest first, then the import paths to what is selected.
    let visible = |l: &Link| {
        let (mut x0, mut x1, mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &l.pts {
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
        let (sx0, sy0) = (cam.x + x0 * cam.k, cam.y + y0 * cam.k);
        let (sx1, sy1) = (cam.x + x1 * cam.k, cam.y + y1 * cam.k);
        sx1 >= 0. && sx0 <= bw && sy1 >= 0. && sy0 <= bh
    };
    let edge_path = |links: &mut dyn Iterator<Item = &Link>, width: f64, dashed: bool| -> Option<gpui::Path<Pixels>> {
        let mut b = PathBuilder::stroke(px(width as f32));
        if dashed {
            b = b.dash_array(&[px((5. * cam.k).max(2.) as f32), px((4. * cam.k).max(2.) as f32)]);
        }
        let mut any = false;
        for l in links {
            if l.pts.len() < 2 || !visible(l) {
                continue;
            }
            any = true;
            b.move_to(sp(l.pts[0].x, l.pts[0].y));
            for i in 1..l.pts.len() {
                let (a, c) = (l.pts[i - 1], l.pts[i]);
                if i % 2 == 0 {
                    b.line_to(sp(c.x, c.y)); // straight through a dummy's column
                } else {
                    let mx = (a.x + c.x) / 2.;
                    b.cubic_bezier_to(sp(c.x, c.y), sp(mx, a.y), sp(mx, c.y));
                }
            }
        }
        if any { b.build().ok() } else { None }
    };
    let on_path = |k: usize| frame.path.as_ref().is_some_and(|p| p.contains(&k));
    for dashed in [false, true] {
        let rest = data
            .links
            .iter()
            .enumerate()
            .filter(|(k, l)| l.dashed == dashed && !on_path(*k))
            .map(|(_, l)| l);
        let alpha = if frame.path.is_some() { dim_a } else { 1. };
        if let Some(p) = edge_path(&mut rest.into_iter(), (1.25 * cam.k).max(1.), dashed) {
            window.paint_path(p, t.edge.opacity(alpha as f32));
        }
        if frame.path.is_some() {
            let lit = data
                .links
                .iter()
                .enumerate()
                .filter(|(k, l)| l.dashed == dashed && on_path(*k))
                .map(|(_, l)| l);
            if let Some(p) = edge_path(&mut lit.into_iter(), (1.6 * cam.k).max(1.2), dashed) {
                window.paint_path(p, t.text3);
            }
        }
    }

    // Pills (labels only where readable), lit ones on top.
    let is_lit = |i: usize| {
        if !frame.rings.is_empty() {
            frame.rings.contains_key(&i)
        } else if let Some(l) = &frame.lit {
            l.contains(&i)
        } else {
            true
        }
    };
    let show_text = cam.k >= 0.42;
    let pin_scale = |i: usize| {
        if frame.pins.contains(&i) {
            (READ_K / cam.k).max(1.)
        } else {
            1.
        }
    };
    let mut lit = Vec::new();
    for (i, n) in data.nodes.iter().enumerate() {
        if dimming && is_lit(i) {
            lit.push(i);
            continue;
        }
        let tier = if !dimming {
            Tier::Own
        } else if frame.via.as_ref().is_some_and(|v| v.contains(&i)) {
            Tier::Via
        } else {
            Tier::Dim
        };
        paint_pill(frame, t, n, i, 1., show_text, tier, f, (ox, oy, bw, bh), window, cx);
    }
    // The winners' glow goes under their pills: it lights the ring, not the fill.
    for &i in &lit {
        if frame.rings.get(&i).is_some_and(|r| r.status == Status::Win) {
            paint_glow(frame, t, &data.nodes[i], pin_scale(i), (ox, oy), window);
        }
    }
    for &i in &lit {
        paint_pill(
            frame,
            t,
            &data.nodes[i],
            i,
            pin_scale(i),
            true,
            Tier::Own,
            1.,
            (ox, oy, bw, bh),
            window,
            cx,
        );
    }
    for &i in &lit {
        if let Some(r) = frame.rings.get(&i) {
            paint_status(frame, t, &data.nodes[i], r, pin_scale(i), (ox, oy), window, cx);
        }
    }
    if let Some(s) = frame.selected
        && frame.rings.is_empty()
    {
        let sc = pin_scale(s);
        paint_ring(frame, &data.nodes[s], sc, (ox, oy), t.accent, 2., false, window);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tier {
    Own,
    Via,
    Dim,
}

/// Screen geometry of a node drawn at scale `s` about its centre.
fn node_screen(frame: &Frame, n: &Node, s: f64, o: (f64, f64)) -> (f64, f64, f64, f64, f64) {
    let cam = frame.cam;
    let k = cam.k * s;
    let cx = o.0 + cam.x + (n.x + n.w / 2.) * cam.k;
    let cy = o.1 + cam.y + n.y * cam.k;
    (cx - n.w / 2. * k, cy, n.w * k, PILL_H * k, k)
}

/// A pill. While something is selected the rest recede: "dim" pills fade
/// their shape to 40 % and turn their label to dim_text; "via" pills
/// (importers on the lit paths) keep 75 % and a text2 label.
#[allow(clippy::too_many_arguments)]
fn paint_pill(
    frame: &Frame,
    t: &Theme,
    n: &Node,
    i: usize,
    s: f64,
    show_text: bool,
    tier: Tier,
    f: f64,
    view: (f64, f64, f64, f64),
    window: &mut Window,
    cx: &mut App,
) {
    let (x, cy, w, h, k) = node_screen(frame, n, s, (view.0, view.1));
    if x + w < view.0 - 20. || x > view.0 + view.2 + 20. || cy + h < view.1 - 20. || cy - h > view.1 + view.3 + 20. {
        return;
    }
    let alpha = match tier {
        Tier::Dim => 1. - 0.6 * f,
        Tier::Via => 1. - 0.25 * f,
        Tier::Own => 1.,
    } as f32;
    let hover = frame.hover == Some(i);
    let bounds = Bounds::new(
        point(px(x as f32), px((cy - h / 2.) as f32)),
        size(px(w as f32), px(h as f32)),
    );
    window.paint_quad(quad(
        bounds,
        px((h / 2.) as f32),
        (if hover { t.s3 } else { t.s2 }).opacity(alpha),
        px(1.),
        (if hover { t.text3 } else { t.line2 }).opacity(alpha),
        if n.disabled {
            BorderStyle::Dashed
        } else {
            BorderStyle::Solid
        },
    ));
    origin_mark(t, n.class, x + 15. * k, cy, k, alpha, window);
    if show_text {
        let own = if n.disabled {
            t.text3
        } else if n.class == OriginClass::Nixpkgs {
            t.text2
        } else {
            t.text
        };
        let size = 12. * k;
        let paint_label = |color: Hsla, a: f32, window: &mut Window, cx: &mut App| {
            let run = TextRun {
                len: n.label.len(),
                font: ui_font(FontWeight::MEDIUM),
                color: color.opacity(a),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = window
                .text_system()
                .shape_line(n.label.clone(), px(size as f32), &[run], None);
            let lh = size * 1.25;
            line.paint(
                point(px((x + 25. * k) as f32), px((cy - lh / 2. + 0.5 * k) as f32)),
                px(lh as f32),
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            )
            .ok();
        };
        match tier {
            Tier::Own => paint_label(own, 1., window, cx),
            _ => {
                // Cross-fade from the label's own colour to the receded one.
                let to = if tier == Tier::Dim { t.dim_text } else { t.text2 };
                if f < 1. {
                    paint_label(own, (1. - f) as f32, window, cx);
                }
                paint_label(to, f as f32, window, cx);
            }
        }
    }
}

/// Origin mark at the start of a pill: user a disc, user-inline a ring,
/// input a diamond, unknown a square, nixpkgs a small disc.
fn origin_mark(t: &Theme, class: OriginClass, x: f64, y: f64, k: f64, alpha: f32, window: &mut Window) {
    let color = t.origin(class).opacity(alpha);
    let disc = |r: f64, fill: bool, window: &mut Window| {
        let b = Bounds::new(
            point(px((x - r * k) as f32), px((y - r * k) as f32)),
            size(px((2. * r * k) as f32), px((2. * r * k) as f32)),
        );
        if fill {
            window.paint_quad(quad(b, px((r * k) as f32), color, px(0.), color, BorderStyle::Solid));
        } else {
            window.paint_quad(quad(
                b,
                px((r * k) as f32),
                gpui::transparent_black(),
                px((1.8 * k).max(1.) as f32),
                color,
                BorderStyle::Solid,
            ));
        }
    };
    match class {
        OriginClass::User => disc(4., true, window),
        OriginClass::Nixpkgs => disc(3., true, window),
        OriginClass::UserInline => disc(4.2, false, window),
        OriginClass::Unknown => {
            let d = 3.6 * k;
            window.paint_quad(quad(
                Bounds::new(
                    point(px((x - d) as f32), px((y - d) as f32)),
                    size(px((2. * d) as f32), px((2. * d) as f32)),
                ),
                px(0.),
                color,
                px(0.),
                color,
                BorderStyle::Solid,
            ));
        }
        OriginClass::Input => {
            let d = 4.6 * k;
            let mut b = PathBuilder::fill();
            b.move_to(point(px(x as f32), px((y - d) as f32)));
            b.line_to(point(px((x + d) as f32), px(y as f32)));
            b.line_to(point(px(x as f32), px((y + d) as f32)));
            b.line_to(point(px((x - d) as f32), px(y as f32)));
            b.close();
            if let Ok(p) = b.build() {
                window.paint_path(p, color);
            }
        }
    }
}

fn paint_glow(frame: &Frame, t: &Theme, n: &Node, s: f64, o: (f64, f64), window: &mut Window) {
    let (x, cy, w, h, k) = node_screen(frame, n, s, o);
    let off = 3.5 + 1.;
    let b = Bounds::new(
        point(px((x - off) as f32), px((cy - h / 2. - off) as f32)),
        size(px((w + 2. * off) as f32), px((h + 2. * off) as f32)),
    );
    window.paint_drop_shadows(
        b,
        Corners::all(px((h / 2. + off) as f32)),
        &[BoxShadow {
            color: t.win.opacity(0.55 * frame.fade as f32),
            offset: point(px(0.), px(0.)),
            blur_radius: px((14. * k).max(6.) as f32),
            spread_radius: px(0.),
            inset: false,
        }],
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_ring(
    frame: &Frame,
    n: &Node,
    s: f64,
    o: (f64, f64),
    color: Hsla,
    width: f64,
    dashed: bool,
    window: &mut Window,
) {
    let (x, cy, w, h, _) = node_screen(frame, n, s, o);
    let off = 3.5 + width / 2.;
    let b = Bounds::new(
        point(px((x - off) as f32), px((cy - h / 2. - off) as f32)),
        size(px((w + 2. * off) as f32), px((h + 2. * off) as f32)),
    );
    window.paint_quad(quad(
        b,
        px((h / 2. + off) as f32),
        gpui::transparent_black(),
        px(width as f32),
        color,
        if dashed {
            BorderStyle::Dashed
        } else {
            BorderStyle::Solid
        },
    ));
}

/// Ring and trailing priority chip of a module that defines the selected
/// option. Winner: solid ring with a glow and a filled "✓" chip; lost: thin
/// ring, outlined chip; off: dashed ring and chip. Shape and text differ,
/// not only the colour.
#[allow(clippy::too_many_arguments)]
fn paint_status(
    frame: &Frame,
    t: &Theme,
    n: &Node,
    r: &Ring,
    s: f64,
    o: (f64, f64),
    window: &mut Window,
    cx: &mut App,
) {
    let a = frame.fade as f32;
    let color = match r.status {
        Status::Win => t.win,
        Status::Lose => t.lose,
        Status::Off => t.off,
        Status::Unknown => t.err,
    }
    .opacity(a);
    let (x, cy, w, _, k) = node_screen(frame, n, s, o);
    if r.status == Status::Win {
        paint_ring(frame, n, s, o, color, 2., false, window);
    } else {
        paint_ring(frame, n, s, o, color, 1.5, r.status == Status::Off, window);
    }
    // The chip, after the pill.
    let cw = r.chip_w * k;
    let ch = CHIP_H * k;
    let chx = x + w + 8. * k;
    let b = Bounds::new(
        point(px(chx as f32), px((cy - ch / 2.) as f32)),
        size(px(cw as f32), px(ch as f32)),
    );
    if r.status == Status::Win {
        window.paint_quad(quad(b, px((ch / 2.) as f32), color, px(0.), color, BorderStyle::Solid));
    } else {
        window.paint_quad(quad(
            b,
            px((ch / 2.) as f32),
            t.s1.opacity(a),
            px(1.),
            color,
            if r.status == Status::Off {
                BorderStyle::Dashed
            } else {
                BorderStyle::Solid
            },
        ));
    }
    let size = 11. * k;
    let run = TextRun {
        len: r.chip.len(),
        font: mono_font(),
        color: if r.status == Status::Win {
            t.on_win.opacity(a)
        } else {
            color
        },
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(r.chip.clone(), px(size as f32), &[run], None);
    let lh = size * 1.3;
    line.paint(
        point(px((chx + 8. * k) as f32), px((cy - lh / 2. + 0.5 * k) as f32)),
        px(lh as f32),
        gpui::TextAlign::Left,
        None,
        window,
        cx,
    )
    .ok();
}
