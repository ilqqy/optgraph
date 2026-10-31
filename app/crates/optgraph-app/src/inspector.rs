//! The inspector (right pane): the start summary, an option's priority
//! ladder, a module's detail. After viewer/src/panels.js.

use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, IntoElement, MouseButton, Pixels, StyledText, div,
    linear_color_stop, linear_gradient, prelude::*, px,
};
use optgraph_core::instances::CrumbTarget;
use optgraph_core::text::{plural, prio_name, priority_name, short_path, tail_path, thousands};
use optgraph_core::{
    Model, Status, ValueView, module_detail_label, module_label, module_short_label, omitted_badge, value_view,
};

use crate::theme::{FONT_MONO, FS_2XL, FS_LG, FS_MD, FS_SM, FS_XL, FS_XS, R_LG, R_MD, R_SM, Theme};
use crate::ui::{
    dashed_badge, icon, instance_chip, kbd, mini_status, origin_chip, origin_dot, overline, shadow_1, shadow_2,
    small_button, status_badge,
};
use crate::view::{Confirm, OptgraphView, Sel};

impl OptgraphView {
    pub fn render_inspector(
        &mut self,
        t: &Theme,
        model: &Model,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body: Vec<AnyElement> = match self.sel {
            Sel::None => self.render_start(t, model, cx),
            Sel::Option(oi) => self.render_option(t, model, oi, cx),
            Sel::Module(mi) => self.render_module(t, model, mi, cx),
        };
        div()
            .id("inspector")
            .flex_none()
            .w(width)
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.inspector_scroll)
            .bg(t.s1)
            .border_l_1()
            .border_color(t.line)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(20.))
                    .pt(px(20.))
                    .pb(px(32.))
                    .children(body),
            )
    }

    /// Nothing selected: summary, the priority scale, the first overrides.
    fn render_start(&mut self, t: &Theme, model: &Model, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let a = &model.analysis;
        let s = &a.stats;
        let tile = |n: usize, label: &str, color: Option<gpui::Hsla>| {
            div()
                .flex_1()
                .pt(px(11.))
                .px(px(12.))
                .pb(px(9.))
                .border_1()
                .border_color(t.line)
                .rounded(px(R_LG))
                .bg(t.s2)
                .shadow(shadow_1(t))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(FS_2XL))
                        .font_weight(FontWeight(650.))
                        .line_height(px(25.))
                        .text_color(color.unwrap_or(t.text))
                        .child(thousands(n as u64)),
                )
                .child(div().text_color(t.text3).text_size(px(FS_SM)).child(label.to_string()))
        };
        let row = |a: gpui::Div, b: gpui::Div, c: gpui::Div| div().flex().gap(px(8.)).child(a).child(b).child(c);
        let tick = |p: i64, align: &str| {
            let d = div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(p.to_string()),
                )
                .child(
                    div()
                        .text_color(t.text3)
                        .font_family(FONT_MONO)
                        .text_size(px(FS_XS))
                        .child(priority_name(p).unwrap_or("").to_string()),
                );
            match align {
                "left" => d.items_start(),
                "right" => d.items_end(),
                _ => d.items_center(),
            }
        };
        let mut out: Vec<AnyElement> = vec![
            overline(t, "Overview").into_any_element(),
            div()
                .mt(px(6.))
                .mb(px(4.))
                .text_size(px(FS_2XL))
                .font_weight(FontWeight(650.))
                .line_height(px(27.5))
                .child("Who set what, and why it won.")
                .into_any_element(),
            div()
                .mb(px(20.))
                .text_color(t.text2)
                .child(
                    "Pick an option to see every definition stacked by priority, or click a module in the graph to see what it sets.",
                )
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .mb(px(24.))
                .child(row(
                    tile(s.modules, "modules", None),
                    tile(s.options, "options", None),
                    tile(s.definitions, "definitions", None),
                ))
                .child(row(
                    tile(s.overrides, "overrides", (s.overrides > 0).then_some(t.lose)),
                    tile(s.switched_off, "switched off", None),
                    tile(s.errors, "errors", (s.errors > 0).then_some(t.err)),
                ))
                .into_any_element(),
            subhead(t, "Priority: lowest number wins").into_any_element(),
            div()
                .mb(px(24.))
                .px(px(14.))
                .py(px(12.))
                .border_1()
                .border_color(t.line)
                .rounded(px(R_LG))
                .bg(t.s2)
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .mb(px(8.))
                        .text_color(t.text3)
                        .text_size(px(FS_XS))
                        .child("stronger")
                        .child("weaker"),
                )
                .child(
                    div()
                        .h(px(6.))
                        .rounded(px(3.))
                        .flex()
                        .overflow_hidden()
                        .child(div().flex_1().h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(t.win, 0.),
                            linear_color_stop(t.lose, 1.),
                        )))
                        .child(div().w(px(0.3 * 380.)).h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(t.lose, 0.),
                            linear_color_stop(t.off, 1.),
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .mt(px(10.))
                        .child(tick(50, "left"))
                        .child(tick(100, "center"))
                        .child(tick(1000, "center"))
                        .child(tick(1500, "right")),
                )
                .into_any_element(),
        ];
        let first: Vec<usize> = a.overrides.iter().take(4).copied().collect();
        if let Some(&f) = first.first() {
            let title = if a.info[f].user_vs_user {
                "Overrides between your modules"
            } else {
                "Overrides"
            };
            out.push(subhead(t, title).into_any_element());
        }
        for oi in first {
            let b = a.info[oi].beats.clone().unwrap();
            let text = format!("{} in {} beats {} in {}", b.win, b.win_who, b.lose, b.lose_who);
            let win_r = 0..b.win.len();
            let lose_start = b.win.len() + 4 + b.win_who.len() + 7;
            let lose_r = lose_start..lose_start + b.lose.len();
            out.push(
                div()
                    .id(("override-card", oi))
                    .mb(px(8.))
                    .px(px(12.))
                    .py(px(10.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(R_LG))
                    .bg(t.s2)
                    .shadow(shadow_1(t))
                    .cursor_pointer()
                    .hover(|s| s.border_color(t.line2).bg(t.s3))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(Sel::Option(oi), cx)))
                    .child(
                        div()
                            .font_family(FONT_MONO)
                            .font_weight(FontWeight::MEDIUM)
                            .child(model.options()[oi].path.clone()),
                    )
                    .child(
                        div().text_color(t.text3).text_size(px(FS_SM)).child(
                            StyledText::new(text)
                                .with_highlights([
                                    (
                                        win_r.clone(),
                                        HighlightStyle {
                                            color: Some(t.win),
                                            ..Default::default()
                                        },
                                    ),
                                    (
                                        lose_r.clone(),
                                        HighlightStyle {
                                            color: Some(t.lose),
                                            ..Default::default()
                                        },
                                    ),
                                ])
                                .with_font_family_overrides([(win_r, FONT_MONO.into()), (lose_r, FONT_MONO.into())]),
                        ),
                    )
                    .into_any_element(),
            );
        }
        out.push(
            div()
                .flex()
                .gap(px(16.))
                .mt(px(20.))
                .text_color(t.text3)
                .text_size(px(FS_SM))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(kbd(t, "/"))
                        .child("search"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(kbd(t, "↑↓"))
                        .child("lists"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(kbd(t, "Esc"))
                        .child("clear"),
                )
                .into_any_element(),
        );
        out
    }

    /// Option detail: header, verdict, the priority ladder as stacked cards.
    fn render_option(&mut self, t: &Theme, model: &Model, oi: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let o = &model.options()[oi];
        let rows = model.ladder(oi);
        // Omitted nixpkgs winners count as winners; when every winner is one
        // of them, `winners` is empty.
        let hidden = o.hidden_winners();
        let multi = o.winner_count() > 1;
        // A dropped definition (shadowedAt) never competed: it is not "weaker".
        let losers = rows
            .iter()
            .filter(|r| r.status == Status::Lose && o.definitions[r.def].shadowed_at.is_none())
            .count() as u64;
        let offs = rows.iter().filter(|r| r.status == Status::Off).count() as u64;
        let winner = (o.winners.len() == 1).then(|| &o.definitions[o.winners[0]]);
        let type_ = o.type_.clone().unwrap_or_else(|| "?".into());

        let path = o.path.clone();
        let copy_label = if self.path_copied { "Copied" } else { "Copy path" };
        let head = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .min_h(px(26.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(overline(t, "Option"))
                    .child(type_chip(t, &type_).min_w_0().truncate()),
            )
            .child(
                small_button(t)
                    .id("copy-path")
                    .hover(|s| s.bg(t.s3).text_color(t.text))
                    .on_click(cx.listener(move |this, _, _, cx| this.copy_path(path.clone(), cx)))
                    .child(icon(if self.path_copied { "check" } else { "copy" }, 14., t.text3))
                    .child(copy_label),
            );
        let (pre, last) = match o.path.rfind('.') {
            Some(i) => (o.path[..=i].to_string(), o.path[i + 1..].to_string()),
            None => (String::new(), o.path.clone()),
        };
        let title = div()
            .mt(px(6.))
            .mb(px(6.))
            .font_family(FONT_MONO)
            .text_size(px(FS_XL))
            .line_height(px(24.7))
            .font_weight(FontWeight::SEMIBOLD)
            .child(StyledText::new(format!("{pre}{last}")).with_highlights([(
                0..pre.len(),
                HighlightStyle {
                    color: Some(t.text3),
                    font_weight: Some(FontWeight::MEDIUM),
                    ..Default::default()
                },
            )]));

        let strong = |color| HighlightStyle {
            color: Some(color),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let verdict: Option<StyledText> = if o.highest_prio.is_none() {
            Some(StyledText::new(format!(
                "No known winner: {}",
                if o.error.is_some() {
                    "an active definition could not be read."
                } else {
                    "nothing active defines it."
                }
            )))
        } else if o.winners.is_empty() && hidden > 0 {
            let who = plural(hidden, &format!("{} definition", o.hidden_kind()), None);
            let code = o.highest_prio.unwrap_or_default().to_string();
            let tail = if losers > 0 {
                format!(", over {}.", plural(losers, "weaker definition", None))
            } else {
                ".".into()
            };
            let s = format!("{who} win at priority {code} (not listed){tail}");
            let c0 = who.len() + 17;
            Some(
                StyledText::new(s)
                    .with_highlights([(0..who.len(), strong(t.text)), (c0..c0 + code.len(), strong(t.win))])
                    .with_font_family_overrides([(c0..c0 + code.len(), FONT_MONO.into())]),
            )
        } else if multi {
            None
        } else if let Some(w) = winner {
            if w.is_default() {
                let rest = if offs > 0 {
                    format!(
                        ": {} switched off.",
                        plural(offs, "definition is", Some("definitions are"))
                    )
                } else {
                    ": nothing stronger is set.".into()
                };
                let s = format!("The option default applies{rest}");
                Some(StyledText::new(s).with_highlights([(4..18, strong(t.text))]))
            } else {
                let who = model.def_who(w);
                let code = format!("{} {}", prio_name(w.priority), w.priority.unwrap_or_default());
                let tail = if losers > 0 {
                    format!(", over {}.", plural(losers, "weaker definition", None))
                } else {
                    ".".into()
                };
                let s = format!("{who} wins with {code}{tail}");
                let c0 = who.len() + 11;
                Some(
                    StyledText::new(s)
                        .with_highlights([(0..who.len(), strong(t.text)), (c0..c0 + code.len(), strong(t.win))])
                        .with_font_family_overrides([(c0..c0 + code.len(), FONT_MONO.into())]),
                )
            }
        } else {
            Some(StyledText::new("No winner is listed."))
        };

        let crumbs = self.render_crumbs(t, model, oi, cx);
        let mut out: Vec<AnyElement> = vec![head.into_any_element()];
        out.extend(crumbs);
        out.push(title.into_any_element());
        if let Some(v) = verdict {
            out.push(
                div()
                    .mb(px(20.))
                    .text_color(t.text2)
                    .text_size(px(13.5))
                    .child(v)
                    .into_any_element(),
            );
        } else {
            out.push(div().h(px(14.)).into_any_element());
        }
        if let Some(err) = &o.error {
            out.push(
                div()
                    .mb(px(16.))
                    .px(px(12.))
                    .py(px(9.))
                    .flex()
                    .gap(px(8.))
                    .items_start()
                    .border_1()
                    .border_color(t.err.opacity(0.4))
                    .rounded(px(R_MD))
                    .bg(t.err_soft)
                    .text_color(t.err)
                    .text_size(px(FS_SM))
                    .child(icon("error", 16., t.err))
                    .child(div().flex_1().min_w_0().child(err.clone()))
                    .into_any_element(),
            );
        }
        if multi && !o.winners.is_empty() {
            let n = model.merged_modules(oi) as u64;
            let mut merged_title = format!("Merged from {}", plural(n, "module", None));
            let mut sub = format!(
                "Every active definition at priority {} ({}) is a winner; the values are merged.",
                o.highest_prio.unwrap_or_default(),
                prio_name(o.highest_prio)
            );
            if hidden > 0 {
                let kind = o.hidden_kind();
                merged_title.push_str(&format!(" and {}", plural(hidden, &format!("{kind} definition"), None)));
                sub.push_str(&format!(" The {kind} ones are not listed."));
            }
            let dots = o.winners.iter().take(8).map(|&w| {
                let origin = model.def_module(&o.definitions[w]).map(|m| m.origin.clone());
                match origin {
                    Some(or) => origin_dot(t, &or),
                    None => div()
                        .size(px(8.))
                        .rounded_full()
                        .border_1()
                        .border_color(t.text3)
                        .into_any_element(),
                }
            });
            out.push(
                div()
                    .mb(px(20.))
                    .px(px(14.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .border_1()
                    .border_color(t.win.opacity(0.45))
                    .rounded(px(R_LG))
                    .bg(t.win_soft)
                    .child(icon("merge", 20., t.win))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(FS_LG))
                                    .font_weight(FontWeight(650.))
                                    .child(merged_title),
                            )
                            .child(div().text_color(t.text2).text_size(px(FS_SM)).child(sub)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(56.))
                            .flex()
                            .flex_wrap()
                            .gap(px(5.))
                            .children(dots),
                    )
                    .into_any_element(),
            );
        }
        let mut lh = div()
            .flex()
            .items_baseline()
            .gap(px(8.))
            .mb(px(10.))
            .text_color(t.text3)
            .text_size(px(FS_SM))
            .whitespace_nowrap()
            .child(
                div()
                    .text_color(t.text)
                    .text_size(px(FS_MD))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Priority ladder"),
            )
            .child(plural(rows.len() as u64, "definition", None));
        if let Some(b) = omitted_badge(o) {
            lh = lh.child(dashed_badge(t, b));
        }
        out.push(lh.child(div().ml_auto().child("lower number wins")).into_any_element());

        if rows.is_empty() {
            out.push(
                div()
                    .text_color(t.text3)
                    .child("No definitions listed.")
                    .into_any_element(),
            );
        } else {
            let cards: Vec<AnyElement> = rows
                .iter()
                .enumerate()
                .map(|(k, r)| self.render_card(t, model, oi, k, r, multi, cx))
                .collect();
            out.push(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    // The rail behind the priority badges.
                    .child(
                        div()
                            .absolute()
                            .left(px(32.))
                            .top(px(22.))
                            .bottom(px(22.))
                            .w(px(2.))
                            .rounded(px(1.))
                            .bg(t.line2),
                    )
                    .children(cards)
                    .into_any_element(),
            );
        }

        let omitted_n = o.omitted_total();
        let mut facts: Vec<(&str, AnyElement)> = vec![(
            "type",
            div()
                .font_family(FONT_MONO)
                .text_size(px(FS_XS))
                .child(type_)
                .into_any_element(),
        )];
        if !o.declared_in.is_empty() {
            facts.push((
                "declared in",
                div()
                    .flex()
                    .flex_col()
                    .font_family(FONT_MONO)
                    .text_size(px(FS_XS))
                    .children(o.declared_in.iter().map(|f| div().child(short_path(f))))
                    .into_any_element(),
            ));
        }
        if omitted_n > 0 {
            facts.push((
                "not listed",
                div()
                    .child(format!(
                        "{} ({} active, {} inactive); optgraph --include-all-definitions lists them",
                        plural(omitted_n, "nixpkgs definition", None),
                        o.omitted.nixpkgs_active,
                        o.omitted.nixpkgs_inactive
                    ))
                    .into_any_element(),
            ));
        }
        let decl_n = o.omitted_declaration_total();
        if decl_n > 0 {
            facts.push((
                "not listed",
                div()
                    .child(format!(
                        "{} of the submodule's own modules{}: {} active, {} inactive; optgraph --include-all-definitions lists them",
                        plural(decl_n, "definition", None),
                        o.omitted
                            .declaration_origin
                            .as_deref()
                            .map(|s| format!(" ({s})"))
                            .unwrap_or_default(),
                        o.omitted.declaration_active,
                        o.omitted.declaration_inactive
                    ))
                    .into_any_element(),
            ));
        }
        out.push(facts_list(t, facts, true).into_any_element());
        out
    }

    /// A nested option's breadcrumb: the option holding the instance (jumps
    /// to its ladder), the instance (filters the sidebar to it), the option's
    /// path below the instance; and a chip with the instance's full path.
    fn render_crumbs(&mut self, t: &Theme, model: &Model, oi: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let is = &model.instances;
        let inst = is.of_option[oi]?;
        let o = &model.options()[oi];
        let mut row = div()
            .mt(px(6.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(4.))
            .font_family(FONT_MONO)
            .text_size(px(FS_SM));
        for (k, c) in is.breadcrumb(oi, &o.path).into_iter().enumerate() {
            if k > 0 {
                row = row.child(div().text_color(t.text3).child("›"));
            }
            let seg = div().id(("crumb", k)).rounded(px(R_SM)).px(px(3.)).child(c.text);
            row = row.child(match c.target {
                CrumbTarget::Here => seg.text_color(t.text).font_weight(FontWeight::MEDIUM),
                CrumbTarget::Option(p) => seg
                    .text_color(t.text2)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.s3).text_color(t.text))
                    .tooltip(crate::ui::tip(format!("Open {}", model.options()[p].path)))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(Sel::Option(p), cx))),
                CrumbTarget::Instance(i) => seg
                    .text_color(t.accent)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.s3))
                    .tooltip(crate::ui::tip(format!("Show only {}", is.list[i].path)))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_instance_filter(Some(i), cx))),
            });
        }
        let path = is.list[inst].path.clone();
        let name = is.chip_name(inst).to_string();
        Some(
            div()
                .flex()
                .flex_col()
                .child(row)
                .child(
                    div()
                        .id("instance-chip")
                        .mt(px(5.))
                        .flex()
                        .cursor_pointer()
                        .tooltip(crate::ui::tip(format!("{path}: show only this instance")))
                        .on_click(cx.listener(move |this, _, _, cx| this.set_instance_filter(Some(inst), cx)))
                        .child(instance_chip(t, &name)),
                )
                .into_any_element(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_card(
        &mut self,
        t: &Theme,
        model: &Model,
        oi: usize,
        k: usize,
        r: &optgraph_core::LadderRow,
        multi: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let o = &model.options()[oi];
        let d = &o.definitions[r.def];
        let module = model.def_module(d);
        let file = model.def_file(d);
        let is_default = d.is_default();
        let compact = multi && r.status == Status::Win;
        let open = compact && self.expanded.contains(&r.def);
        let def = r.def;

        // Priority badge column.
        let (num_bg, num_fg, num_border, dashed) = match r.status {
            Status::Win => (t.win, t.on_win, None, false),
            Status::Lose => (t.lose_soft, t.lose, Some(t.lose.opacity(0.5)), false),
            Status::Off | Status::Unknown => (t.s1, t.text3, Some(t.line2), true),
        };
        let small = compact && !open;
        let num = div()
            .min_w(px(if small { 42. } else { 46. }))
            .h(px(if small { 24. } else { 30. }))
            .px(px(6.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(R_MD))
            .bg(num_bg)
            .text_color(num_fg)
            .font_family(FONT_MONO)
            .font_weight(FontWeight(650.))
            .text_size(px(if small { FS_MD } else { FS_LG }))
            .when_some(num_border, |n, c| n.border_1().border_color(c))
            .when(dashed, |n| n.border_dashed())
            .child(d.priority.map(|p| p.to_string()).unwrap_or_else(|| "—".into()));
        let nm = match d.priority {
            None if d.active => "unknown".to_string(),
            None => "off".to_string(),
            Some(_) => prio_name(d.priority),
        };
        let prio = div()
            .flex_none()
            .w(px(64.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(if small { 1. } else { 3. }))
            .pt(px(if small { 0. } else { 1. }))
            .child(num)
            .child(
                div()
                    .max_w(px(62.))
                    .text_color(t.text3)
                    .font_family(FONT_MONO)
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(10.5))
                    .line_height(px(12.6))
                    .text_center()
                    .child(nm),
            );

        let who = |id: &'static str| -> AnyElement {
            if is_default {
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h(px(26.))
                    .pl(px(9.))
                    .pr(px(11.))
                    .rounded_full()
                    .border_1()
                    .border_dashed()
                    .border_color(t.line2)
                    .text_color(t.text2)
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .child(icon("diamond", 13., t.text2))
                    .child("option default")
                    .into_any_element()
            } else if let Some(m) = module {
                let mi = model.mod_index[&m.id];
                // Long names wrap: the pill keeps its height for one line.
                div()
                    .id((id, def))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .min_h(px(26.))
                    .min_w_0()
                    .pl(px(9.))
                    .pr(px(11.))
                    .py(px(3.))
                    .rounded(px(13.))
                    .border_1()
                    .border_color(t.line2)
                    .bg(t.s3)
                    .text_color(t.text)
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(px(17.))
                    .cursor_pointer()
                    .hover(|s| s.border_color(t.accent).bg(t.accent_soft))
                    .tooltip(crate::ui::tip(module_label(m)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.select(Sel::Module(mi), cx);
                    }))
                    .child(origin_dot(t, &m.origin))
                    .child(div().min_w_0().child(module_detail_label(m)))
                    .into_any_element()
            } else {
                div()
                    .h(px(26.))
                    .px(px(11.))
                    .rounded_full()
                    .border_1()
                    .border_color(t.line2)
                    .child(
                        file.as_deref()
                            .map(optgraph_core::text::base_name)
                            .unwrap_or_else(|| "?".into()),
                    )
                    .into_any_element()
            }
        };

        let view = value_view(d);
        let card = div().relative().flex().pr(px(14.)).border_1().rounded(px(R_LG));
        let card = match (r.status, is_default) {
            (Status::Win, _) if small => card
                .py(px(8.))
                .items_center()
                .border_color(t.win.opacity(0.6))
                .bg(t.s2)
                .child(gradient_layer(linear_gradient(
                    90.,
                    linear_color_stop(crate::theme::over(t.s2, t.win_soft), 0.),
                    linear_color_stop(t.s2, 0.6),
                )))
                .shadow(shadow_1(t)),
            (Status::Win, _) => card
                .pt(px(13.))
                .pb(px(12.))
                .border_color(t.win.opacity(0.6))
                .bg(t.s2)
                .child(gradient_layer(linear_gradient(
                    180.,
                    linear_color_stop(crate::theme::over(t.s2, t.win_soft), 0.),
                    linear_color_stop(t.s2, 0.8),
                )))
                .shadow(vec![gpui::BoxShadow {
                    color: t.win.opacity(0.35),
                    offset: gpui::point(px(0.), px(18.)),
                    blur_radius: px(40.),
                    spread_radius: px(-22.),
                    inset: false,
                }]),
            (Status::Off | Status::Unknown, _) | (_, true) => card
                .pt(px(13.))
                .pb(px(12.))
                .border_dashed()
                .border_color(t.line)
                .bg(t.s1),
            _ => card
                .pt(px(13.))
                .pb(px(12.))
                .border_color(t.line)
                .bg(t.s2)
                .shadow(shadow_2(t)),
        };

        if small {
            let preview = match &view {
                ValueView::Preview(v) => v.split_whitespace().collect::<Vec<_>>().join(" "),
                ValueView::Redacted => "<redacted>".into(),
                ValueView::Missing(why) => (*why).into(),
            };
            let none = !matches!(view, ValueView::Preview(_));
            let origin = module
                .map(|m| m.origin.clone())
                .unwrap_or(optgraph_core::Origin::Unknown);
            let focus = self.row_focus.get(k).cloned();
            let summary = div()
                .id(("summary", def))
                .key_context("CompactRow")
                .when_some(focus, |d, f| d.track_focus(&f))
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(2.))
                .rounded(px(R_MD))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| this.toggle_row(def, k, window, cx)))
                .on_action(cx.listener(move |this, _: &Confirm, window, cx| this.toggle_row(def, k, window, cx)))
                .child(div().flex_none().max_w(px(190.)).child(who("pill-row")))
                .child(origin_chip(t, &origin))
                .child(mini_status(t, Status::Win))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if none { t.text3 } else { t.text2 })
                        .text_size(px(FS_SM))
                        .when(!none, |d| d.font_family(FONT_MONO))
                        .child(preview),
                )
                .child(icon("chevron", 16., t.text3));
            let focused = self.focused_row == Some(k);
            return card
                .id(("card", def))
                .hover(|s| s.border_color(t.win.opacity(0.8)))
                .when(focused, |d| d.border_color(t.accent))
                .child(prio)
                .child(summary)
                .into_any_element();
        }

        let mut head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_wrap()
            .min_h(px(26.))
            .child(who("pill"));
        if !is_default {
            let origin = module
                .map(|m| m.origin.clone())
                .unwrap_or(optgraph_core::Origin::Unknown);
            head = head.child(origin_chip(t, &origin));
        }
        head = head.child(
            div()
                .ml_auto()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(status_badge(t, r.status))
                .when(compact, |d| {
                    d.child(
                        div()
                            .id(("collapse", def))
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(R_SM))
                            .cursor_pointer()
                            .hover(|s| s.bg(t.s3))
                            .on_click(cx.listener(move |this, _, window, cx| this.toggle_row(def, k, window, cx)))
                            .child(icon("chevron-up", 16., t.text3)),
                    )
                }),
        );
        let mut body = div().flex_1().min_w_0().flex().flex_col().child(head);
        if !is_default
            && let (Some(m), Some(f)) = (module, &file)
            && *f != m.file
        {
            body = body.child(where_line(t, format!("file {}", short_path(f))));
        }
        if is_default && let Some(f) = &file {
            body = body.child(where_line(t, format!("declared in {}", tail_path(f, 2))));
        }
        body = body.child(match view {
            ValueView::Missing(why) => div()
                .mt(px(10.))
                .mb(px(8.))
                .px(px(12.))
                .py(px(8.))
                .border_1()
                .border_dashed()
                .border_color(t.line2)
                .rounded(px(R_MD))
                .text_color(t.text3)
                .text_size(px(FS_SM))
                .child(why)
                .into_any_element(),
            ValueView::Redacted | ValueView::Preview(_) => {
                let redacted = matches!(view, ValueView::Redacted);
                let text = match &view {
                    ValueView::Preview(v) => v.clone(),
                    _ => "<redacted>".to_string(),
                };
                let copied = self.copied == Some(def);
                let copy_text = text.clone();
                div()
                    .relative()
                    .mt(px(10.))
                    .mb(px(8.))
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(R_MD))
                    .bg(t.bg)
                    .child(
                        div()
                            .id(("code", def))
                            .max_h(px(220.))
                            .overflow_y_scroll()
                            .pl(px(12.))
                            .pr(px(40.))
                            .py(px(8.))
                            .font_family(FONT_MONO)
                            .text_size(px(FS_MD))
                            .line_height(px(19.5))
                            .text_color(if redacted {
                                t.text3
                            } else if r.status == Status::Lose {
                                t.text2
                            } else {
                                t.text
                            })
                            .when(redacted, |d| d.italic())
                            .child(text),
                    )
                    .child(
                        div()
                            .id(("copy", def))
                            .absolute()
                            .top(px(5.))
                            .right(px(5.))
                            .w(px(26.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(R_SM))
                            .border_1()
                            .border_color(if copied { t.win } else { t.line })
                            .bg(t.s2)
                            .cursor_pointer()
                            .hover(|s| s.border_color(t.line2))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.copy_value(def, copy_text.clone(), cx)
                            }))
                            .child(icon(
                                if copied { "check" } else { "copy" },
                                14.,
                                if copied { t.win } else { t.text3 },
                            )),
                    )
                    .into_any_element()
            }
        });
        body = body.child(
            div()
                .text_color(if r.status == Status::Win { t.text } else { t.text2 })
                .text_size(px(FS_SM))
                .child(r.reason.clone()),
        );
        let focus = self.row_focus.get(k).cloned();
        card.id(("card", def))
            .when(compact, |d| {
                d.key_context("CompactRow")
                    .when_some(focus, |d, f| d.track_focus(&f))
                    .on_action(cx.listener(move |this, _: &Confirm, window, cx| this.toggle_row(def, k, window, cx)))
            })
            .child(prio)
            .child(body)
            .into_any_element()
    }

    /// Module detail: what it is, where it sits, what it sets and whether it wins.
    fn render_module(&mut self, t: &Theme, model: &Model, mi: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let m = &model.modules()[mi];
        let opts = &model.mod_options[mi];
        let importers = model.importers[mi].len();
        let stat = |n: usize, label: &str| {
            div()
                .flex_1()
                .pt(px(11.))
                .px(px(12.))
                .pb(px(9.))
                .border_1()
                .border_color(t.line)
                .rounded(px(R_LG))
                .bg(t.s2)
                .shadow(shadow_1(t))
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(FS_2XL))
                        .font_weight(FontWeight(650.))
                        .line_height(px(25.))
                        .child(n.to_string()),
                )
                .child(div().text_color(t.text3).text_size(px(FS_SM)).child(label.to_string()))
        };
        let mut facts: Vec<(&str, AnyElement)> = vec![
            (
                "origin",
                div().flex().child(origin_chip(t, &m.origin)).into_any_element(),
            ),
            ("file", code_fact(m.file.clone())),
        ];
        if m.id != m.file {
            facts.push(("id", code_fact(m.id.clone())));
        }
        if let Some(p) = &m.position {
            facts.push(("position", code_fact(p.clone())));
        }
        if let Some(i) = model.instances.of_module[mi] {
            let path = model.instances.list[i].path.clone();
            let name = model.instances.chip_name(i).to_string();
            facts.push((
                "instance",
                div()
                    .id("module-instance")
                    .flex()
                    .cursor_pointer()
                    .tooltip(crate::ui::tip(format!("{path}: show only this instance")))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_instance_filter(Some(i), cx)))
                    .child(instance_chip(t, &name))
                    .into_any_element(),
            ));
        }
        if let Some(&owner) = m.owner.as_deref().and_then(|o| model.mod_index.get(o)) {
            let label = module_short_label(&model.modules()[owner]);
            facts.push((
                "written by",
                div()
                    .id("module-owner")
                    .font_family(FONT_MONO)
                    .text_size(px(FS_XS))
                    .text_color(t.text2)
                    .cursor_pointer()
                    .hover(|s| s.text_color(t.text))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(Sel::Module(owner), cx)))
                    .child(label)
                    .into_any_element(),
            ));
        }
        let mut out: Vec<AnyElement> = vec![
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_h(px(26.))
                .child(overline(t, "Module"))
                .when(m.disabled, |d| d.child(type_chip(t, "disabled")))
                .into_any_element(),
            div()
                .mt(px(6.))
                .mb(px(2.))
                .flex()
                .items_center()
                .gap(px(10.))
                .text_size(px(FS_2XL))
                .font_weight(FontWeight(650.))
                .child(origin_dot(t, &m.origin))
                .child(module_short_label(m))
                .into_any_element(),
            div()
                .mb(px(16.))
                .text_color(t.text3)
                .font_family(FONT_MONO)
                .text_size(px(FS_SM))
                .child(module_label(m))
                .into_any_element(),
            div()
                .flex()
                .gap(px(8.))
                .mb(px(16.))
                .child(stat(opts.len(), "options set"))
                .child(stat(m.imports.len(), "imports"))
                .child(stat(importers, "importers"))
                .into_any_element(),
            facts_list(t, facts, false).into_any_element(),
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .mb(px(10.))
                .text_color(t.text3)
                .text_size(px(FS_SM))
                .child(
                    div()
                        .text_color(t.text)
                        .text_size(px(FS_MD))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Options it sets"),
                )
                .child(plural(opts.len() as u64, "listed option", None))
                .into_any_element(),
        ];
        if opts.is_empty() {
            out.push(
                div()
                    .text_color(t.text3)
                    .child("This module sets no listed option.")
                    .into_any_element(),
            );
        } else {
            let rows = opts.iter().map(|&oi| {
                let st = model.module_status(mi, oi);
                div()
                    .id(("mod-opt", oi))
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(R_SM))
                    .text_color(t.text2)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.s3).text_color(t.text))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(Sel::Option(oi), cx)))
                    .child(mini_status(t, st.map(|s| s.status).unwrap_or(Status::Unknown)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(FONT_MONO)
                            .text_size(px(FS_SM))
                            .child(model.options()[oi].path.clone()),
                    )
                    .when_some(st.and_then(|s| s.priority), |d, p| {
                        d.child(
                            div()
                                .text_color(t.text3)
                                .font_family(FONT_MONO)
                                .text_size(px(FS_XS))
                                .child(p.to_string()),
                        )
                    })
            });
            out.push(
                div()
                    .p(px(4.))
                    .border_1()
                    .border_color(t.line)
                    .rounded(px(R_LG))
                    .bg(t.s2)
                    .children(rows)
                    .into_any_element(),
            );
        }
        out
    }
}

/// A gradient over the card's solid background (GPUI takes one background;
/// the CSS layers the gradient over --s2).
fn gradient_layer(bg: gpui::Background) -> gpui::Div {
    div().absolute().inset_0().rounded(px(R_LG - 1.)).bg(bg)
}

fn subhead(t: &Theme, text: &str) -> gpui::Div {
    overline(t, text).mb(px(8.))
}

fn type_chip(t: &Theme, text: &str) -> gpui::Div {
    div()
        .px(px(6.))
        .rounded(px(R_SM))
        .border_1()
        .border_color(t.line2)
        .text_color(t.text2)
        .font_family(FONT_MONO)
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(FS_XS))
        .line_height(px(18.))
        .child(text.to_string())
}

fn where_line(t: &Theme, text: String) -> gpui::Div {
    div()
        .mt(px(5.))
        .text_color(t.text3)
        .font_family(FONT_MONO)
        .text_size(px(FS_XS))
        .child(text)
}

fn code_fact(text: String) -> AnyElement {
    div()
        .font_family(FONT_MONO)
        .text_size(px(FS_XS))
        .child(text)
        .into_any_element()
}

/// The `dl.facts` grid: labels in a 92px column.
fn facts_list(t: &Theme, facts: Vec<(&str, AnyElement)>, bottom: bool) -> gpui::Div {
    let mut d = div().flex().flex_col().gap(px(6.)).text_size(px(FS_SM));
    d = if bottom {
        d.mt(px(24.)).pt(px(16.)).border_t_1().border_color(t.line)
    } else {
        d.mb(px(20.))
    };
    d.children(facts.into_iter().map(|(k, v)| {
        div()
            .flex()
            .gap(px(12.))
            .child(div().flex_none().w(px(92.)).text_color(t.text3).child(k.to_string()))
            .child(div().flex_1().min_w_0().text_color(t.text2).child(v))
    }))
}
