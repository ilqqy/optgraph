//! Sidebar (brand, search trigger, the Overrides / Switched off / Errors
//! lists with counts) and the top bar (host, nixpkgs version, status chips).

use gpui::{AnyElement, Context, FontWeight, IntoElement, MouseButton, div, prelude::*, px};
use optgraph_core::Model;
use optgraph_core::text::{plural, thousands};

use crate::theme::{FONT_MONO, FS_LG, FS_SM, FS_XS, R_MD, R_SM, SIDE_W, TOP_H, Theme};
use crate::ui::{icon, instance_chip, kbd, status_chip};
use crate::view::{ListKind, OptgraphView, Sel, SideRow};

impl OptgraphView {
    /// Child index in the sidebar's scroll list of sidebar item `cursor`.
    pub fn side_child_index(&self, cursor: usize) -> usize {
        self.side_rows
            .iter()
            .position(|r| matches!(r, SideRow::Item(i) if *i == cursor))
            .unwrap_or(0)
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

        let rows: Vec<AnyElement> = self
            .side_rows
            .clone()
            .into_iter()
            .map(|r| self.side_row(t, model, r, cx))
            .collect();
        let filter = self.instance_filter.map(|i| {
            let inst = &model.instances.list[i];
            div()
                .flex_none()
                .mx(px(12.))
                .mb(px(4.))
                .h(px(28.))
                .pl(px(9.))
                .pr(px(3.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(px(R_MD))
                .border_1()
                .border_color(t.accent.opacity(0.5))
                .bg(t.accent_soft)
                .text_size(px(FS_SM))
                .child(icon("filter", 13., t.accent))
                .child(
                    div()
                        .id("filter-chip")
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .tooltip(crate::ui::tip(inst.path.clone()))
                        .child(format!("instance: {}", inst.name)),
                )
                .child(
                    div()
                        .id("filter-clear")
                        .flex_none()
                        .size(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(R_SM))
                        .cursor_pointer()
                        .hover(|s| s.bg(t.s3))
                        .tooltip(crate::ui::tip("Clear the instance filter"))
                        .on_click(cx.listener(|this, _, _, cx| this.set_instance_filter(None, cx)))
                        .child(icon("x", 13., t.text2)),
                )
        });
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
            .children(filter)
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

    fn side_row(&mut self, t: &Theme, model: &Model, row: SideRow, cx: &mut Context<Self>) -> AnyElement {
        let head = |title: &str, ico: AnyElement, count: usize| {
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
                .child(ico)
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
                        .child(thousands(count as u64)),
                )
        };
        let note = |text: String| {
            div()
                .px(px(8.))
                .py(px(4.))
                .text_color(t.text3)
                .text_size(px(FS_SM))
                .child(text)
                .into_any_element()
        };
        match row {
            SideRow::Head(ListKind::Options, total) => {
                let open = self
                    .options_open
                    .unwrap_or(model.instances.top_level.len() <= crate::view::OPTIONS_OPEN_MAX);
                head(
                    "Options",
                    icon(if open { "chevron" } else { "chevron-right" }, 16., t.text3).into_any_element(),
                    total,
                )
                .id("options-head")
                .cursor_pointer()
                .rounded(px(R_SM))
                .hover(|s| s.bg(t.s3))
                .tooltip(crate::ui::tip("Options outside any submodule instance"))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_options_list(cx)))
                .into_any_element()
            }
            SideRow::Head(kind, total) => {
                let (title, ico, color) = match kind {
                    ListKind::Overrides => ("Overrides", "override", t.lose),
                    ListKind::Off => ("Switched off", "off", t.off),
                    _ => ("Errors", "error", t.err),
                };
                head(title, icon(ico, 16., color).into_any_element(), total).into_any_element()
            }
            SideRow::Empty(kind) => note(
                match kind {
                    ListKind::Overrides => "No listed definition lost.",
                    ListKind::Off => "No definition is switched off.",
                    ListKind::Errors => "No option threw.",
                    _ => "None.",
                }
                .to_string(),
            ),
            SideRow::More(n) => note(format!("{} more: use the search", thousands(n as u64))),
            SideRow::GroupsHead(n) => {
                head("Instances", icon("instance", 16., t.accent).into_any_element(), n).into_any_element()
            }
            SideRow::Group(i, depth) => self.group_row(t, model, i, depth, cx),
            SideRow::Item(i) => self.item_row(t, model, i, cx),
        }
    }

    fn group_row(&mut self, t: &Theme, model: &Model, i: usize, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        let is = &model.instances;
        let inst = &is.list[i];
        let open = self.group_is_open(model, i);
        let label = is.label(i).to_string();
        let mut facts = vec![plural(inst.total_options as u64, "option", None)];
        facts.push(plural(inst.total_modules as u64, "module", None));
        let counts = |n: usize, ico: &'static str, color: gpui::Hsla| {
            (n > 0).then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .text_color(color)
                    .child(icon(ico, 11., color))
                    .child(n.to_string())
            })
        };
        div()
            .id(("group", i))
            .mt(px(if depth == 0 { 2. } else { 0. }))
            .ml(px(depth as f32 * 10.))
            .pl(px(4.))
            .pr(px(2.))
            .py(px(4.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(R_SM))
            .cursor_pointer()
            .hover(|s| s.bg(t.s3))
            .tooltip(crate::ui::tip(inst.path.clone()))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(i, cx)))
            .child(icon(if open { "chevron" } else { "chevron-right" }, 14., t.text3))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis_start()
                            .font_family(FONT_MONO)
                            .text_size(px(FS_SM))
                            .line_height(px(17.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(label),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_color(t.text3)
                            .text_size(px(FS_XS))
                            .line_height(px(14.))
                            .child(facts.join(" · "))
                            .children(counts(inst.overrides, "override", t.lose))
                            .children(counts(inst.switched_off, "off", t.off))
                            .children(counts(inst.errors, "error", t.err)),
                    ),
            )
            .child(
                div()
                    .id(("group-filter", i))
                    .flex_none()
                    .size(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(R_SM))
                    .hover(|s| s.bg(t.s2))
                    .tooltip(crate::ui::tip("Show only this instance"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.set_instance_filter(Some(i), cx);
                    }))
                    .child(icon("filter", 13., t.text3)),
            )
            .into_any_element()
    }

    fn item_row(&mut self, t: &Theme, model: &Model, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let item = self.side_items[i].clone();
        let selected = match self.sel {
            Sel::Option(oi) => Some(oi),
            _ => None,
        };
        let is_sel = selected == Some(item.option);
        let is_cursor = self.side_cursor == Some(i);
        let oi = item.option;
        let path = model.options()[oi].path.clone();
        let is = &model.instances;
        let in_group = matches!(item.list, ListKind::Instance(_));
        let depth = match item.list {
            ListKind::Instance(g) => is.list[g].depth + 1,
            _ => 0,
        };
        // In a group the path below the instance; else the full path, which
        // wraps, and a chip with its instance.
        let text = if in_group {
            is.relative_path(oi, &path).to_string()
        } else {
            path
        };
        let chip = if in_group { None } else { is.of_option[oi] };
        div()
            .id(("side", i))
            .ml(px(depth as f32 * 10.))
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
            .tooltip(crate::ui::tip(item.note.clone()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.side_cursor = Some(i);
                this.select(Sel::Option(oi), cx);
                this.side_cursor = Some(i);
            }))
            .when_some(chip.is_none().then_some(text.clone()), |d, s| {
                d.child(div().truncate().child(s))
            })
            .when_some(chip, |d, c| {
                d.child(div().child(text))
                    .child(div().mt(px(3.)).flex().child(instance_chip(t, &is.list[c].name)))
            })
            .into_any_element()
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
                    )
                    .when(!model.instances.is_empty(), |d| {
                        d.child(
                            div()
                                .id("instances-chip")
                                .flex_none()
                                .h(px(24.))
                                .px(px(9.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .rounded_full()
                                .border_1()
                                .border_color(t.line)
                                .bg(t.s2)
                                .text_color(t.text2)
                                .text_size(px(FS_SM))
                                .whitespace_nowrap()
                                .tooltip(crate::ui::tip(
                                    "Options inside submodule instances (users.users.<name>, ...)",
                                ))
                                .child(icon("instance", 13., t.accent))
                                .child(format!(
                                    "{} · {}",
                                    plural(model.instances.list.len() as u64, "instance", None),
                                    plural(model.instances.nested_options() as u64, "nested option", None)
                                )),
                        )
                    }),
            )
            .child(div().flex_1())
            .child({
                let summary = format!(
                    "{} modules · {} options · scope {}",
                    thousands(model.modules().len() as u64),
                    thousands(model.options().len() as u64),
                    m.scope.as_str()
                );
                div()
                    .id("graph-summary")
                    .flex_shrink_1()
                    .min_w_0()
                    .truncate()
                    .text_color(t.text3)
                    .text_size(px(FS_SM))
                    .tooltip(crate::ui::tip(summary.clone()))
                    .child(summary)
            })
            .child(
                crate::ui::small_button(t)
                    .id("theme-toggle")
                    .h(px(30.))
                    .px(px(8.))
                    .hover(|s| s.bg(t.s3).text_color(t.text))
                    .tooltip(crate::ui::tip(if t.mode == crate::theme::Mode::Dark {
                        "Light theme"
                    } else {
                        "Dark theme"
                    }))
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_scheme(window, cx)))
                    .child(icon(
                        if t.mode == crate::theme::Mode::Dark {
                            "sun"
                        } else {
                            "moon"
                        },
                        16.,
                        t.text3,
                    )),
            )
            .child(
                crate::ui::small_button(t)
                    .id("open-file")
                    .h(px(30.))
                    .px(px(12.))
                    .text_size(px(crate::theme::FS_MD))
                    .hover(|s| s.bg(t.s3).text_color(t.text))
                    .on_click(cx.listener(|this, _, window, cx| this.prompt_open(window, cx)))
                    .child(icon("open", 16., t.text3))
                    .child("Open graph.json"),
            )
    }
}

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
