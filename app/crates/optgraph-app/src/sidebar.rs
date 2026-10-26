//! Sidebar (brand, search trigger, the Overrides / Switched off / Errors
//! lists with counts) and the top bar (host, nixpkgs version, status chips).

use gpui::{Context, FontWeight, IntoElement, MouseButton, div, prelude::*, px};
use optgraph_core::Model;
use optgraph_core::text::{plural, thousands};

use crate::theme::{FONT_MONO, FS_LG, FS_SM, FS_XS, R_MD, R_SM, SIDE_W, TOP_H, Theme};
use crate::ui::{icon, kbd, status_chip};
use crate::view::{ListKind, OptgraphView, Sel};

const LISTS: [(ListKind, &str, &str, &str); 3] = [
    (
        ListKind::Overrides,
        "Overrides",
        "override",
        "No listed definition lost.",
    ),
    (ListKind::Off, "Switched off", "off", "No definition is switched off."),
    (ListKind::Errors, "Errors", "error", "No option threw."),
];

impl OptgraphView {
    /// Child index in the sidebar's scroll list of sidebar item `cursor`
    /// (each list has a header, and an empty list a note).
    pub fn side_child_index(&self, cursor: usize) -> usize {
        let mut child = 0;
        let mut item = 0;
        for (kind, ..) in LISTS {
            child += 1;
            let n = self.side_items.iter().filter(|s| s.list == kind).count();
            if cursor < item + n {
                return child + (cursor - item);
            }
            item += n;
            child += n.max(1) + usize::from(self.list_total(kind) > n);
        }
        child
    }

    fn list_total(&self, kind: ListKind) -> usize {
        let Some(m) = &self.model else { return 0 };
        let a = &m.analysis;
        match kind {
            ListKind::Overrides => a.overrides.len(),
            ListKind::Off => a.switched_off.len(),
            ListKind::Errors => a.errors.len(),
        }
    }

    pub fn render_sidebar(&mut self, t: &Theme, model: &Model, cx: &mut Context<Self>) -> impl IntoElement {
        let brand = div()
            .flex_none()
            .h(px(TOP_H))
            .flex()
            .items_center()
            .gap(px(9.))
            .px(px(16.))
            .border_b_1()
            .border_color(t.line)
            .text_size(px(FS_LG))
            .font_weight(FontWeight(650.))
            .child(logo(t, 1.))
            .child("optgraph");
        let trigger = div()
            .id("search-trigger")
            .flex_none()
            .mx(px(12.))
            .mt(px(12.))
            .mb(px(8.))
            .h(px(34.))
            .pl(px(10.))
            .pr(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(R_MD))
            .border_1()
            .border_color(t.line)
            .bg(t.s2)
            .text_color(t.text3)
            .cursor_pointer()
            .hover(|s| s.border_color(t.line2).text_color(t.text2))
            .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx)))
            .child(icon("search", 16., t.text3))
            .child(div().flex_1().truncate().child("Search options, modules"))
            .child(kbd(t, "/"));

        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let selected = match self.sel {
            Sel::Option(oi) => Some(oi),
            _ => None,
        };
        let mut index = 0usize;
        for (kind, title, ico, empty) in LISTS {
            let total = self.list_total(kind);
            let color = match kind {
                ListKind::Overrides => t.lose,
                ListKind::Off => t.off,
                ListKind::Errors => t.err,
            };
            rows.push(
                div()
                    .mt(px(12.))
                    .px(px(8.))
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .text_color(t.text3)
                    .text_size(px(FS_XS))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(icon(ico, 16., color))
                    .child(title.to_uppercase())
                    .child(
                        div()
                            .ml_auto()
                            .px(px(6.))
                            .rounded(px(R_SM))
                            .bg(t.s3)
                            .text_color(t.text2)
                            .font_family(FONT_MONO)
                            .font_weight(FontWeight::MEDIUM)
                            .child(thousands(total as u64)),
                    )
                    .into_any_element(),
            );
            let items: Vec<(usize, crate::view::SideItem)> = self
                .side_items
                .iter()
                .enumerate()
                .filter(|(_, s)| s.list == kind)
                .map(|(i, s)| (i, s.clone()))
                .collect();
            if items.is_empty() {
                rows.push(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .text_color(t.text3)
                        .text_size(px(FS_SM))
                        .child(empty)
                        .into_any_element(),
                );
            }
            for (i, item) in items {
                let path = model.options()[item.option].path.clone();
                let is_sel = selected == Some(item.option);
                let is_cursor = self.side_cursor == Some(i);
                let oi = item.option;
                rows.push(
                    div()
                        .id(("side", i))
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(R_SM))
                        .text_color(if is_sel { t.text } else { t.text2 })
                        .font_family(FONT_MONO)
                        .text_size(px(FS_SM))
                        .line_height(px(18.))
                        .cursor_pointer()
                        .when(is_sel, |d| d.bg(t.accent_soft).border_l_2().border_color(t.accent))
                        .when(is_cursor && !is_sel, |d| d.bg(t.s3))
                        .hover(|s| s.bg(t.s3).text_color(t.text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.side_cursor = Some(i);
                            this.select(Sel::Option(oi), cx);
                            this.side_cursor = Some(i);
                        }))
                        .child(div().truncate().child(path))
                        .into_any_element(),
                );
                index += 1;
            }
            if total > SIDE_LIMIT_SHOWN.min(total) {
                rows.push(
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .text_color(t.text3)
                        .text_size(px(FS_SM))
                        .child(format!(
                            "{} more: use the search",
                            thousands((total - SIDE_LIMIT_SHOWN) as u64)
                        ))
                        .into_any_element(),
                );
            }
        }
        let _ = index;
        div()
            .flex_none()
            .w(px(SIDE_W))
            .h_full()
            .flex()
            .flex_col()
            .bg(t.s1)
            .border_r_1()
            .border_color(t.line)
            .child(brand)
            .child(trigger)
            .child(
                div()
                    .id("lists")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.sidebar_scroll)
                    .px(px(8.))
                    .pt(px(4.))
                    .pb(px(16.))
                    .children(rows),
            )
    }

    pub fn render_topbar(&mut self, t: &Theme, model: &Model, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &model.graph.meta;
        let n = m.warnings.len() as u64;
        let errs = model.analysis.errors.len() as u64;
        let crumb = div()
            .flex()
            .items_baseline()
            .gap(px(8.))
            .whitespace_nowrap()
            .min_w_0()
            .child(
                div()
                    .text_size(px(FS_LG))
                    .font_weight(FontWeight(650.))
                    .child(m.host.clone().unwrap_or_else(|| "?".into())),
            )
            .child(div().text_color(t.text3).child("/"))
            .child(
                div()
                    .text_color(t.text2)
                    .font_family(FONT_MONO)
                    .text_size(px(FS_SM))
                    .truncate()
                    .child(format!("nixpkgs {}", m.nixpkgs_version)),
            );
        let warnings_chip = status_chip(t, n == 0, plural(n, "warning", None));
        let errors_chip = status_chip(t, errs == 0, plural(errs, "option error", None));
        let first_error = model.analysis.errors.first().copied();
        div()
            .flex_none()
            .h(px(TOP_H))
            .min_w_0()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .bg(t.s1)
            .border_b_1()
            .border_color(t.line)
            .child(crumb)
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .ml(px(8.))
                    .child(status_chip(
                        t,
                        m.attribution == optgraph_core::model::Attribution::Aligned,
                        m.attribution.as_str(),
                    ))
                    .child(status_chip(
                        t,
                        m.complete,
                        if m.complete { "complete" } else { "partial" },
                    ))
                    .child(
                        div()
                            .id("warnings-chip")
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.warnings_open = !this.warnings_open;
                                cx.notify();
                            }))
                            .child(warnings_chip),
                    )
                    .child(
                        div()
                            .id("errors-chip")
                            .when(errs > 0, |d| d.cursor_pointer())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(oi) = first_error {
                                    this.select(Sel::Option(oi), cx);
                                }
                            }))
                            .child(errors_chip),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex_shrink_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text3)
                    .text_size(px(FS_SM))
                    .child(format!(
                        "{} modules · {} options · scope {}",
                        thousands(model.modules().len() as u64),
                        thousands(model.options().len() as u64),
                        m.scope.as_str()
                    )),
            )

    }
}

const SIDE_LIMIT_SHOWN: usize = crate::view::SIDE_LIST_LIMIT;

/// The logo (viewer/template.html): three accent bars, a priority ladder.
pub fn logo(t: &Theme, scale: f32) -> impl IntoElement {
    let bar = |x: f32, y: f32, w: f32, a: f32| {
        div()
            .absolute()
            .left(px(x * scale))
            .top(px(y * scale))
            .w(px(w * scale))
            .h(px(4.2 * scale))
            .rounded(px(2.1 * scale))
            .bg(t.accent.opacity(a))
    };
    div()
        .flex_none()
        .relative()
        .size(px(22. * scale))
        .child(bar(1., 2., 20., 1.))
        .child(bar(5., 8.9, 16., 0.62))
        .child(bar(9., 15.8, 12., 0.34))
}
