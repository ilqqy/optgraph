//! Command palette (`/` or Ctrl+K): fuzzy search over options and modules
//! with the matched characters highlighted, results grouped (Overrides,
//! Switched off, Errors, Options, Modules). Arrows move, Enter opens, Esc
//! closes, a click opens. Also the warnings popover. After
//! viewer/src/palette.js.

use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, HighlightStyle, IntoElement, MouseButton, ScrollHandle,
    StyledText, Window, div, prelude::*, px,
};
use optgraph_core::search::{Group, GroupResult, Item, Kind, Where, palette_search};
use optgraph_core::text::thousands;
use optgraph_core::{Model, module_label, module_short_label};

use crate::input::TextInput;
use crate::theme::{FONT_MONO, FS_LG, FS_MD, FS_SM, FS_XS, R_LG, R_MD, R_XL, Theme};
use crate::ui::{icon, kbd, origin_chip, shadow_3};
use crate::view::{OptgraphView, PalClose, PalConfirm, PalDown, PalUp, Sel};

pub struct PaletteState {
    pub open: bool,
    pub input: Entity<TextInput>,
    pub results: Vec<GroupResult>,
    pub active: usize,
    pub scroll: ScrollHandle,
}

impl PaletteState {
    pub fn new(input: Entity<TextInput>) -> Self {
        PaletteState {
            open: false,
            input,
            results: Vec::new(),
            active: 0,
            scroll: ScrollHandle::new(),
        }
    }

    fn items(&self) -> impl Iterator<Item = (Group, &Item)> {
        self.results
            .iter()
            .flat_map(|g| g.items.iter().map(move |it| (g.group, it)))
    }

    fn len(&self) -> usize {
        self.results.iter().map(|g| g.items.len()).sum()
    }
}

fn group_icon(g: Group) -> &'static str {
    match g {
        Group::Overrides => "override",
        Group::Off => "off",
        Group::Errors => "error",
        Group::Options => "option",
        Group::Modules => "module",
    }
}

/// `text` with the chars at `hits` in the accent colour, bold, on a soft background.
fn highlighted(t: &Theme, text: &str, hits: &[usize]) -> StyledText {
    let mut ranges = Vec::new();
    for (ci, (bi, ch)) in text.char_indices().enumerate() {
        if hits.contains(&ci) {
            ranges.push((
                bi..bi + ch.len_utf8(),
                HighlightStyle {
                    color: Some(t.accent),
                    background_color: Some(t.accent_soft),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            ));
        }
    }
    StyledText::new(text.to_string()).with_highlights(ranges)
}

impl OptgraphView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.is_none() {
            return;
        }
        self.palette.open = true;
        self.warnings_open = false;
        self.palette.input.update(cx, |i, cx| i.set_text("", cx));
        self.run_palette(cx);
        let f = self.palette.input.read(cx).focus_handle(cx);
        window.focus(&f, cx);
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette.open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub fn palette_query_changed(&mut self, cx: &mut Context<Self>) {
        if self.palette.open {
            self.run_palette(cx);
        }
    }

    fn run_palette(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self.model.clone() else { return };
        let q = self.palette.input.read(cx).text().to_string();
        self.palette.results = palette_search(&model, &q);
        self.palette.active = 0;
        self.palette.scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    fn pal_move(&mut self, d: isize, cx: &mut Context<Self>) {
        let n = self.palette.len();
        if n == 0 {
            return;
        }
        self.palette.active = (self.palette.active as isize + d).rem_euclid(n as isize) as usize;
        // Scroll the row (or, for the first row of a group, its header) into view.
        let mut child = 0;
        let mut k = 0;
        'outer: for g in &self.palette.results {
            child += 1;
            for _ in &g.items {
                if k == self.palette.active {
                    break 'outer;
                }
                k += 1;
                child += 1;
            }
        }
        self.palette.scroll.scroll_to_item(child);
        cx.notify();
    }

    fn pal_choose(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, it)) = self.palette.items().nth(n) else {
            return;
        };
        let sel = match it.kind {
            Kind::Option => Sel::Option(it.index),
            Kind::Module => Sel::Module(it.index),
        };
        self.close_palette(window, cx);
        self.select(sel, cx);
    }

    pub fn render_palette(&mut self, t: &Theme, model: &Model, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.palette.input.read(cx).text().trim().to_string();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut n = 0usize;
        let results = self.palette.results.clone();
        for g in &results {
            let ico_color = match g.group {
                Group::Overrides => t.lose,
                Group::Errors => t.err,
                _ => t.text3,
            };
            let total = if g.total > g.items.len() {
                format!("{} of {}", g.items.len(), thousands(g.total as u64))
            } else {
                g.total.to_string()
            };
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .pt(px(10.))
                    .px(px(10.))
                    .pb(px(4.))
                    .text_color(t.text3)
                    .text_size(px(FS_XS))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(icon(group_icon(g.group), 13., ico_color))
                    .child(g.group.title().to_uppercase())
                    .child(
                        div()
                            .ml_auto()
                            .font_family(FONT_MONO)
                            .font_weight(FontWeight::MEDIUM)
                            .child(total),
                    )
                    .into_any_element(),
            );
            for it in &g.items {
                rows.push(self.palette_row(t, model, g.group, it, n, cx));
                n += 1;
            }
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .py(px(28.))
                    .px(px(16.))
                    .flex()
                    .justify_center()
                    .text_color(t.text3)
                    .child(format!("Nothing matches {query}."))
                    .into_any_element(),
            );
        }
        let total: usize = self.palette.results.iter().map(|g| g.total).sum();
        let count = if query.is_empty() {
            String::new()
        } else {
            format!(
                "{} {}",
                thousands(total as u64),
                if total == 1 { "match" } else { "matches" }
            )
        };
        let input = self.palette.input.clone();
        input.update(cx, |i, _| {
            i.placeholder_color = t.text3;
            i.caret_color = t.accent;
            i.selection_color = t.accent_soft;
        });
        div()
            .id("palette")
            .key_context("Palette")
            .on_action(cx.listener(|this, _: &PalUp, _, cx| this.pal_move(-1, cx)))
            .on_action(cx.listener(|this, _: &PalDown, _, cx| this.pal_move(1, cx)))
            .on_action(cx.listener(|this, _: &PalConfirm, window, cx| {
                let a = this.palette.active;
                this.pal_choose(a, window, cx)
            }))
            .on_action(cx.listener(|this, _: &PalClose, window, cx| this.close_palette(window, cx)))
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt(gpui::relative(0.12))
            .px(px(16.))
            .child(
                div()
                    .id("palette-scrim")
                    .absolute()
                    .inset_0()
                    .bg(t.scrim)
                    .on_click(cx.listener(|this, _, window, cx| this.close_palette(window, cx))),
            )
            .child(
                div()
                    .id("palette-box")
                    .relative()
                    .w(px(680.))
                    .max_h(px(560.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .border_1()
                    .border_color(t.line2)
                    .rounded(px(R_XL))
                    .bg(t.s1)
                    .shadow(shadow_3(t))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(16.))
                            .h(px(54.))
                            .border_b_1()
                            .border_color(t.line)
                            .text_color(t.text)
                            .text_size(px(FS_LG))
                            .child(icon("search", 18., t.text3))
                            .child(input),
                    )
                    .child(
                        div()
                            .id("palette-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.palette.scroll)
                            .p(px(6.))
                            .children(rows),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .px(px(16.))
                            .py(px(8.))
                            .border_t_1()
                            .border_color(t.line)
                            .bg(t.s2)
                            .text_color(t.text3)
                            .text_size(px(FS_XS))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .child(kbd(t, "↑"))
                                    .child(kbd(t, "↓"))
                                    .child("move"),
                            )
                            .child(div().flex().items_center().gap(px(3.)).child(kbd(t, "↵")).child("open"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.))
                                    .child(kbd(t, "Esc"))
                                    .child("close"),
                            )
                            .child(div().ml_auto().font_family(FONT_MONO).child(count)),
                    ),
            )
    }

    fn palette_row(
        &mut self,
        t: &Theme,
        model: &Model,
        group: Group,
        it: &Item,
        n: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = n == self.palette.active;
        let ico_color = match group {
            Group::Overrides => t.lose,
            Group::Off => t.off,
            Group::Errors => t.err,
            _ => t.text3,
        };
        let (main, meta): (AnyElement, AnyElement) = match it.kind {
            Kind::Module => {
                let m = &model.modules()[it.index];
                let label = module_short_label(m);
                let path = module_label(m);
                let none: &[usize] = &[];
                (
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().font_weight(FontWeight::MEDIUM).child(highlighted(
                            t,
                            &label,
                            if it.place == Where::Label { &it.hits } else { none },
                        )))
                        .child(
                            div()
                                .truncate()
                                .text_color(t.text3)
                                .font_family(FONT_MONO)
                                .text_size(px(FS_XS))
                                .child(highlighted(
                                    t,
                                    &path,
                                    if it.place == Where::Path { &it.hits } else { none },
                                )),
                        )
                        .into_any_element(),
                    origin_chip(t, &m.origin).into_any_element(),
                )
            }
            Kind::Option => {
                let o = &model.options()[it.index];
                let a = &model.analysis;
                let meta = div()
                    .flex_none()
                    .max_w(gpui::relative(0.4))
                    .truncate()
                    .text_color(t.text3)
                    .font_family(FONT_MONO)
                    .text_size(px(FS_XS));
                let meta = match group {
                    Group::Overrides => {
                        let b = a.info[it.index].beats.clone().unwrap_or_else(|| optgraph_core::Beats {
                            win: String::new(),
                            win_who: String::new(),
                            win_origin: String::new(),
                            lose: String::new(),
                            lose_who: String::new(),
                            lose_origin: String::new(),
                        });
                        let s = format!("{} › {}", b.win, b.lose);
                        let w = b.win.len();
                        meta.child(StyledText::new(s.clone()).with_highlights([
                            (
                                0..w,
                                HighlightStyle {
                                    color: Some(t.win),
                                    ..Default::default()
                                },
                            ),
                            (
                                s.len() - b.lose.len()..s.len(),
                                HighlightStyle {
                                    color: Some(t.lose),
                                    ..Default::default()
                                },
                            ),
                        ]))
                    }
                    Group::Off => {
                        let off = &a.info[it.index].off;
                        let who = off
                            .first()
                            .map(|&d| model.def_who(&o.definitions[d]))
                            .unwrap_or_default();
                        let more = if off.len() > 1 {
                            format!(" +{}", off.len() - 1)
                        } else {
                            String::new()
                        };
                        meta.child(format!("mkIf false in {who}{more}"))
                    }
                    Group::Errors => meta.text_color(t.err).child("error"),
                    _ => meta.child(o.type_.clone().unwrap_or_default()),
                };
                (
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(FONT_MONO)
                        .text_size(px(FS_MD))
                        .child(highlighted(t, &o.path, &it.hits))
                        .into_any_element(),
                    meta.into_any_element(),
                )
            }
        };
        div()
            .id(("pal", n))
            .flex()
            .items_center()
            .gap(px(10.))
            .min_h(px(36.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(R_MD))
            .text_color(if active { t.text } else { t.text2 })
            .cursor_pointer()
            .when(active, |d| d.bg(t.accent_soft).border_l_2().border_color(t.accent))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.palette.active != n {
                    this.palette.active = n;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.pal_choose(n, window, cx)))
            .child(icon(group_icon(group), 16., ico_color))
            .child(main)
            .child(meta)
            .child(div().when(!active, |d| d.invisible()).child(kbd(t, "↵")))
            .into_any_element()
    }

    /// The warnings popover from the top bar's chip.
    pub fn render_warnings(&mut self, t: &Theme, model: &Model, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = &model.graph.meta.warnings;
        let list: Vec<AnyElement> = if ws.is_empty() {
            vec![
                div()
                    .px(px(14.))
                    .py(px(10.))
                    .text_color(t.text3)
                    .child("No warnings.")
                    .into_any_element(),
            ]
        } else {
            ws.iter()
                .map(|w| {
                    div()
                        .px(px(14.))
                        .py(px(10.))
                        .border_b_1()
                        .border_color(t.line)
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .font_family(FONT_MONO)
                                        .text_size(px(FS_SM))
                                        .text_color(t.err)
                                        .child(w.code.clone()),
                                )
                                .when_some(w.subject.clone(), |d, s| {
                                    d.child(
                                        div()
                                            .text_color(t.text3)
                                            .font_family(FONT_MONO)
                                            .text_size(px(FS_SM))
                                            .child(s),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .mt(px(4.))
                                .text_color(t.text2)
                                .font_family(FONT_MONO)
                                .text_size(px(FS_SM))
                                .child(w.message.clone()),
                        )
                        .into_any_element()
                })
                .collect()
        };
        div()
            .id("warnings-panel")
            .absolute()
            .top(px(crate::theme::TOP_H + 8.))
            .right(px(12.))
            .w(px(640.))
            .max_h(gpui::relative(0.7))
            .overflow_y_scroll()
            .bg(t.s1)
            .border_1()
            .border_color(t.line)
            .rounded(px(R_LG))
            .shadow(shadow_3(t))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pl(px(14.))
                    .pr(px(8.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(t.line)
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Warnings ({})", ws.len())),
                    )
                    .child(
                        crate::ui::icon_button(t, "x")
                            .id("warnings-close")
                            .hover(|s| s.bg(t.s3))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.warnings_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .children(list)
    }
}
