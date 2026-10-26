//! The centre pane: the graph canvas with pan (drag), zoom (wheel),
//! hover tooltip and click to select, and its overlays (banner, legend,
//! zoom buttons, chips for off-screen ancestors). Also the pan benchmark
//! (OPTGRAPH_BENCH_PAN=<frames>: pans by itself, prints frame times, quits).

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, CursorStyle, FontWeight, IntoElement, MouseButton, Pixels, ScrollEvent, Window, canvas, div,
    prelude::*, px,
};
use optgraph_core::{Model, module_label};

use crate::graph::{self, GraphState};
use crate::theme::{FONT_MONO, FS_SM, FS_XS, R_MD, SIDE_W, TOP_H, Theme};
use crate::ui::{icon, origin_chip, origin_dot, shadow_1, shadow_3};
use crate::view::{OptgraphView, Sel};

pub struct Bench {
    frames: usize,
    last: Option<Instant>,
    intervals: Vec<f64>,
    cpu: Vec<f64>,
    dir: f64,
}

impl Bench {
    pub fn from_env() -> Option<Bench> {
        let n: usize = std::env::var("OPTGRAPH_BENCH_PAN").ok()?.parse().ok()?;
        Some(Bench { frames: n, last: None, intervals: Vec::new(), cpu: Vec::new(), dir: 1. })
    }

    fn report(&self) {
        let stats = |v: &[f64]| {
            let mut s = v.to_vec();
            s.sort_by(|a, b| a.total_cmp(b));
            let mean = s.iter().sum::<f64>() / s.len().max(1) as f64;
            let q = |p: f64| s[((s.len() as f64 - 1.) * p).round() as usize];
            format!("mean {mean:.2} ms, p50 {:.2}, p95 {:.2}, max {:.2}", q(0.5), q(0.95), q(1.))
        };
        // The first frames warm caches (glyphs, paths); skip them.
        let skip = 10.min(self.intervals.len() / 4);
        eprintln!("optgraph-app bench: {} frames panning", self.intervals.len() - skip);
        eprintln!("  frame interval: {}", stats(&self.intervals[skip..]));
        eprintln!("  cpu (render + paint): {}", stats(&self.cpu[skip..]));
    }
}

impl OptgraphView {
    pub fn render_stage(
        &mut self,
        t: &Theme,
        model: &Model,
        insp_w: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let started = Instant::now();
        let vp = window.viewport_size();
        let (w, h) = (
            f64::from(f32::from(vp.width)) - SIDE_W as f64 - f64::from(f32::from(insp_w)),
            f64::from(f32::from(vp.height)) - TOP_H as f64,
        );
        if self.graph.data.is_none() {
            self.graph.set_model(model, window);
        }
        self.graph.set_size(w, h);
        if let Some(sel) = self.graph_sel.take() {
            match sel {
                Sel::None => self.graph.select_none(),
                Sel::Option(oi) => self.graph.select_option(&model.defining_modules(oi), window),
                Sel::Module(mi) => self.graph.select_module(mi),
            }
        }
        if let Some(b) = &mut self.bench {
            // Pan back and forth across the graph.
            self.graph.cam.x += 6. * b.dir;
            if self.graph.cam.x.abs() > 600. {
                b.dir = -b.dir;
            }
        }
        let Some((frame, moving)) = self.graph.frame_at(Instant::now()) else {
            return div().flex_1().into_any_element();
        };
        if moving || self.bench.is_some() {
            window.request_animation_frame();
        }
        let theme = t.clone();
        let entity = cx.entity().downgrade();
        let painter = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    graph::paint(&frame, &theme, bounds, window, cx);
                });
                // Bench: CPU time of this frame, from render to the end of painting.
                let cpu = started.elapsed().as_secs_f64() * 1000.;
                entity.update(cx, |v, cx| v.bench_frame(cpu, cx)).ok();
            },
        )
        .absolute()
        .inset_0();

        let local = |p: gpui::Point<Pixels>| (f64::from(f32::from(p.x)) - SIDE_W as f64, f64::from(f32::from(p.y)) - TOP_H as f64);
        let dragging = self.graph.drag.as_ref().is_some_and(|d| d.moved);
        let cursor = if dragging {
            CursorStyle::ClosedHand
        } else if self.graph.hover.is_some() {
            CursorStyle::PointingHand
        } else {
            CursorStyle::OpenHand
        };
        let mut stage = div()
            .id("stage")
            .flex_1()
            .min_w_0()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(t.bg)
            .cursor(cursor)
            .child(painter)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                    this.graph.stop_camera();
                    this.graph.drag = Some(graph::Drag { start: local(ev.position), cam: this.graph.cam, moved: false });
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _, cx| {
                let (x, y) = local(ev.position);
                if let Some(d) = &mut this.graph.drag {
                    if !ev.dragging() {
                        this.graph.drag = None;
                        return;
                    }
                    let (dx, dy) = (x - d.start.0, y - d.start.1);
                    if !d.moved && dx.abs() + dy.abs() > 4. {
                        d.moved = true;
                    }
                    if d.moved {
                        this.graph.cam = graph::Cam { x: d.cam.x + dx, y: d.cam.y + dy, k: d.cam.k };
                        this.graph.hover = None;
                        cx.notify();
                    }
                    return;
                }
                let hover = this.graph.node_at(x, y);
                if hover != this.graph.hover {
                    this.graph.hover = hover;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, ev: &gpui::MouseUpEvent, _, cx| {
                    let Some(d) = this.graph.drag.take() else { return };
                    if !d.moved {
                        let (x, y) = local(ev.position);
                        if let Some(n) = this.graph.node_at(x, y) {
                            let mi = this.graph.data.as_ref().unwrap().nodes[n].module;
                            this.select(Sel::Module(mi), cx);
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.graph.drag = None;
                    cx.notify();
                }),
            )
            .on_hover(cx.listener(|this, inside: &bool, _, cx| {

                if !inside && this.graph.hover.is_some() {
                    this.graph.hover = None;
                    cx.notify();
                }
            }))
            .on_scroll(cx.listener(move |this, ev: &ScrollEvent, window, cx| {
                let d = ev.delta.pixel_delta(window.line_height());
                let (x, y) = local(ev.position);
                this.graph.wheel(x, y, -f64::from(f32::from(d.y)), ev.modifiers.control);
                this.graph.hover = None;
                cx.notify();
            }));

        let data = frame_data(&self.graph);
        if let Some(banner) = data.and_then(|d| d.banner.clone()) {
            stage = stage.child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left(px(12.))
                    .right(px(12.))
                    .px(px(12.))
                    .py(px(8.))
                    .bg(t.s1)
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(R_MD))
                    .text_color(t.text2)
                    .shadow(shadow_1(t))
                    .child(banner),
            );
        }
        for chip in self.graph.edge_chips() {
            let n = chip.node;
            let border = match chip.status {
                Some(optgraph_core::Status::Win) => t.win,
                Some(optgraph_core::Status::Lose) => t.lose,
                Some(optgraph_core::Status::Off) => t.off,
                Some(optgraph_core::Status::Unknown) => t.err,
                None => t.line2,
            };
            stage = stage.child(
                div()
                    .id(("edge-chip", n))
                    .absolute()
                    .left(px(chip.x as f32))
                    .top(px(chip.y as f32))
                    .h(px(24.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .border_1()
                    .border_color(border)
                    .when(chip.status == Some(optgraph_core::Status::Off), |d| d.border_dashed())
                    .bg(t.s1)
                    .text_color(t.text2)
                    .text_size(px(FS_XS))
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .shadow(shadow_1(t))
                    .cursor_pointer()
                    .hover(|s| s.border_color(t.accent).text_color(t.text))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.graph.reveal(n);
                        cx.notify();
                    }))
                    .child(chip.text),
            );
        }
        if let (Some(n), None) = (self.graph.hover, &self.graph.drag) {
            stage = stage.child(self.tooltip(t, model, n));
        }
        stage = stage.child(self.legend(t)).child(self.zoom_buttons(t, cx));
        stage.into_any_element()
    }

    fn tooltip(&self, t: &Theme, model: &Model, n: usize) -> AnyElement {
        let data = self.graph.data.as_ref().unwrap();
        let mi = data.nodes[n].module;
        let m = &model.modules()[mi];
        let sets = model.mod_options[mi].len();
        let importers = model.importers[mi].len();
        let (x, y, _, rh) = self.graph.node_rect(n);
        let tip_h = if m.disabled { 104. } else { 84. };
        let tip_w = 420f64.min(self.graph.width - 16.);
        let mut ty = y - tip_h - 10.;
        if ty < 8. {
            ty = y + rh + 10.;
        }
        let tx = x.min(self.graph.width - tip_w - 8.).max(8.);
        let fact = |n: usize, one: &str, many: &str| {
            div()
                .flex()
                .gap(px(4.))
                .child(div().text_color(t.text).font_weight(FontWeight::SEMIBOLD).child(n.to_string()))
                .child(if n == 1 { one.to_string() } else { many.to_string() })
        };
        div()
            .absolute()
            .left(px(tx as f32))
            .top(px(ty as f32))
            .max_w(px(tip_w as f32))
            .px(px(12.))
            .py(px(9.))
            .border_1()
            .border_color(t.line2)
            .rounded(px(R_MD))
            .bg(t.s1)
            .shadow(shadow_3(t))
            .text_size(px(FS_SM))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .text_size(px(13.))
                    .child(origin_dot(t, &m.origin))
                    .child(div().font_weight(FontWeight::BOLD).child(data.nodes[n].label.clone()))
                    .child(div().ml_auto().pl(px(12.)).child(origin_chip(t, &m.origin))),
            )
            .child(
                div()
                    .mt(px(4.))
                    .mb(px(6.))
                    .text_color(t.text2)
                    .font_family(FONT_MONO)
                    .text_size(px(FS_XS))
                    .child(module_label(m)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .text_color(t.text3)
                    .child(fact(sets, "option set", "options set"))
                    .child(fact(m.imports.len(), "imports", "imports"))
                    .child(fact(importers, "importer", "importers")),
            )
            .when(m.disabled, |d| {
                d.child(div().mt(px(4.)).text_color(t.text3).italic().child("disabled (disabledModules): not evaluated"))
            })
            .into_any_element()
    }

    /// Origins (mark shape and colour), and while an option is selected the
    /// statuses of its defining modules.
    fn legend(&self, t: &Theme) -> AnyElement {
        let item = |mark: AnyElement, label: &str| div().flex().items_center().gap(px(6.)).child(mark).child(label.to_string());
        let o = |s: &str| origin_dot(t, &optgraph_core::Origin::parse(s));
        let ring = |c: gpui::Hsla, dashed: bool, glow: bool| {
            div()
                .size(px(11.))
                .rounded_full()
                .border(px(if glow { 2. } else { 1.5 }))
                .border_color(c)
                .when(dashed, |d| d.border_dashed())
                .into_any_element()
        };
        let focused = !self.graph.rings.is_empty();
        div()
            .absolute()
            .bottom(px(12.))
            .left(px(12.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .px(px(12.))
            .py(px(7.))
            .border_1()
            .border_color(t.line)
            .rounded(px(R_MD))
            .bg(t.s1.opacity(0.92))
            .shadow(shadow_1(t))
            .text_size(px(FS_XS))
            .text_color(t.text2)
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .items_center()
                    .child(item(o("user"), "user"))
                    .child(item(o("user-inline"), "inline"))
                    .child(item(o("input:x"), "input"))
                    .child(item(o("nixpkgs"), "nixpkgs"))
                    .child(item(o("unknown"), "unknown"))
                    .child(item(
                        div().w(px(18.)).h(px(10.)).rounded(px(5.)).border_1().border_dashed().border_color(t.text3).into_any_element(),
                        "disabled",
                    )),
            )
            .when(focused, |d| {
                d.child(
                    div()
                        .flex()
                        .gap(px(12.))
                        .items_center()
                        .child(item(ring(t.win, false, true), "winner ✓"))
                        .child(item(ring(t.lose, false, false), "lost"))
                        .child(item(ring(t.off, true, false), "mkIf false"))
                        .child(div().text_color(t.text3).child("import path")),
                )
            })
            .into_any_element()
    }

    fn zoom_buttons(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let btn = |id: &'static str, name: &'static str| {
            div()
                .id(id)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(t.s3))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(icon(name, 16., t.text3))
        };
        div()
            .absolute()
            .right(px(12.))
            .bottom(px(12.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(3.))
            .border_1()
            .border_color(t.line)
            .rounded(px(R_MD))
            .bg(t.s1.opacity(0.92))
            .shadow(shadow_1(t))
            .child(btn("zoom-in", "plus").on_click(cx.listener(|this, _, _, cx| {
                this.graph.zoom_by(1.25);
                cx.notify();
            })))
            .child(btn("zoom-out", "minus").on_click(cx.listener(|this, _, _, cx| {
                this.graph.zoom_by(0.8);
                cx.notify();
            })))
            .child(btn("zoom-fit", "fit").on_click(cx.listener(|this, _, _, cx| {
                this.graph.fit();
                cx.notify();
            })))
            .into_any_element()
    }

    fn bench_frame(&mut self, cpu: f64, cx: &mut Context<Self>) {
        let Some(b) = &mut self.bench else { return };
        let now = Instant::now();
        if let Some(last) = b.last {
            b.intervals.push((now - last).as_secs_f64() * 1000.);
            b.cpu.push(cpu);
        }
        b.last = Some(now);
        if b.intervals.len() >= b.frames {
            b.report();
            self.bench = None;
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    }
}

fn frame_data(g: &GraphState) -> Option<&std::rc::Rc<graph::GraphData>> {
    g.data.as_ref()
}
