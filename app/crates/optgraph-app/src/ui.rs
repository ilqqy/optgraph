//! Small pieces shared by the panels, after the web viewer's CSS classes:
//! icons, origin dots and chips, status badges, keys, buttons.

use gpui::{
    AnyElement, BoxShadow, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, point, px, svg,
};
use optgraph_core::{Origin, OriginClass, Status};

use crate::theme::{FS_SM, FS_XS, R_MD, R_SM, Theme};

/// A 16x16 stroke icon (assets.rs), tinted.
pub fn icon(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// The same with a heavier stroke (status marks).
pub fn icon_bold(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}@2.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// Origin mark: user a disc, user-inline a ring, input a diamond, unknown a
/// square, nixpkgs a small disc. Shape and colour both differ.
pub fn origin_dot(t: &Theme, origin: &Origin) -> AnyElement {
    let class = origin.class();
    let color = t.origin(class);
    match class {
        OriginClass::User => div()
            .flex_none()
            .size(px(8.))
            .rounded_full()
            .bg(color)
            .into_any_element(),
        OriginClass::UserInline => div()
            .flex_none()
            .size(px(8.))
            .rounded_full()
            .border(px(1.8))
            .border_color(color)
            .into_any_element(),
        OriginClass::Nixpkgs => div()
            .flex_none()
            .mx(px(1.))
            .size(px(6.))
            .rounded_full()
            .bg(color)
            .into_any_element(),
        OriginClass::Unknown => div()
            .flex_none()
            .size(px(7.))
            .rounded(px(1.))
            .bg(color)
            .into_any_element(),
        OriginClass::Input => svg()
            .path("icons/dot-diamond.svg")
            .size(px(10.))
            .flex_none()
            .text_color(color)
            .into_any_element(),
    }
}

/// Origin chip: `user`, `input:shared`, ...
pub fn origin_chip(t: &Theme, origin: &Origin) -> Div {
    let color = t.origin(origin.class());
    div()
        .flex_none()
        .px(px(8.))
        .h(px(20.))
        .flex()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(color.opacity(0.45))
        .text_color(color)
        .text_size(px(FS_XS))
        .whitespace_nowrap()
        .child(origin.label())
}

/// The chip naming a submodule instance.
pub fn instance_chip(t: &Theme, name: &str) -> Div {
    div()
        .flex_none()
        .max_w(px(200.))
        .h(px(18.))
        .pl(px(5.))
        .pr(px(7.))
        .flex()
        .items_center()
        .gap(px(3.))
        .overflow_hidden()
        .rounded_full()
        .border_1()
        .border_color(t.accent.opacity(0.4))
        .bg(t.accent_soft)
        .text_color(t.text2)
        .text_size(px(FS_XS))
        .font_family(crate::theme::FONT_UI)
        .whitespace_nowrap()
        .child(icon("instance", 11., t.accent))
        .child(div().truncate().child(name.to_string()))
}

/// Winner / Lost / Switched off / Unknown badge: a mark and a label.
pub fn status_badge(t: &Theme, s: Status) -> Div {
    let (ico, label) = match s {
        Status::Win => ("check", "Winner"),
        Status::Lose => ("x", "Lost"),
        Status::Off => ("off", "Switched off"),
        Status::Unknown => ("error", "Unknown"),
    };
    let base = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .h(px(22.))
        .pl(px(6.))
        .pr(px(9.))
        .rounded_full()
        .text_size(px(FS_XS))
        .font_weight(FontWeight(650.))
        .whitespace_nowrap();
    match s {
        Status::Win => base
            .bg(t.win)
            .text_color(t.on_win)
            .child(icon_bold(ico, 13., t.on_win))
            .child(label),
        Status::Lose => base
            .border_1()
            .border_color(t.lose.opacity(0.6))
            .text_color(t.lose)
            .child(icon_bold(ico, 13., t.lose))
            .child(label),
        Status::Off => base
            .border_1()
            .border_dashed()
            .border_color(t.off)
            .text_color(t.off)
            .child(icon_bold(ico, 13., t.off))
            .child(label),
        Status::Unknown => base
            .border_1()
            .border_color(t.err.opacity(0.55))
            .text_color(t.err)
            .child(icon_bold(ico, 13., t.err))
            .child(label),
    }
}

/// The 18px round status mark of compact rows and module option lists.
pub fn mini_status(t: &Theme, s: Status) -> Div {
    let base = div()
        .flex_none()
        .size(px(18.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center();
    match s {
        Status::Win => base.bg(t.win).child(icon_bold("check", 11., t.on_win)),
        Status::Lose => base.border_1().border_color(t.lose).child(icon_bold("x", 11., t.lose)),
        Status::Off => base
            .border_1()
            .border_dashed()
            .border_color(t.off)
            .child(icon_bold("off", 11., t.off)),
        Status::Unknown => base
            .border_1()
            .border_color(t.err)
            .child(icon_bold("error", 11., t.err)),
    }
}

/// Status chip of the top bar: a mark (✓ or !) and text.
pub fn status_chip(t: &Theme, ok: bool, text: impl Into<SharedString>) -> Div {
    let mark = div()
        .flex_none()
        .size(px(16.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD);
    let mark = if ok {
        mark.bg(t.win_soft).text_color(t.win).child("✓")
    } else {
        mark.bg(t.err).text_color(t.s1).child("!")
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(24.))
        .pl(px(4.))
        .pr(px(10.))
        .rounded_full()
        .border_1()
        .border_color(if ok { t.line } else { t.err.opacity(0.4) })
        .bg(t.s2)
        .text_color(if ok { t.text2 } else { t.err })
        .text_size(px(FS_SM))
        .whitespace_nowrap()
        .child(mark)
        .child(text.into())
}

pub fn kbd(t: &Theme, key: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .min_w(px(18.))
        .px(px(5.))
        .h(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .border_1()
        .border_b(px(2.))
        .border_color(t.line2)
        .bg(t.s2)
        .text_color(t.text2)
        .text_size(px(FS_XS))
        .child(key.into())
}

/// Uppercase section label (overline, subhead, list headers).
pub fn overline(t: &Theme, text: &str) -> Div {
    div()
        .text_color(t.text3)
        .text_size(px(FS_XS))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.to_uppercase())
}

/// The "+N nixpkgs" badge and other dashed pills.
pub fn dashed_badge(t: &Theme, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(7.))
        .rounded_full()
        .border_1()
        .border_dashed()
        .border_color(t.line2)
        .text_color(t.text3)
        .text_size(px(FS_XS))
        .whitespace_nowrap()
        .child(text.into())
}

pub fn shadow_1(t: &Theme) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: t.shadow.opacity(0.3),
        offset: point(px(0.), px(1.)),
        blur_radius: px(2.),
        spread_radius: px(0.),
        inset: false,
    }]
}

pub fn shadow_2(t: &Theme) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: t.shadow,
        offset: point(px(0.), px(12.)),
        blur_radius: px(30.),
        spread_radius: px(-14.),
        inset: false,
    }]
}

pub fn shadow_3(t: &Theme) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: t.text.opacity(0.05),
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(1.),
            inset: false,
        },
        BoxShadow {
            color: t.shadow,
            offset: point(px(0.), px(30.)),
            blur_radius: px(80.),
            spread_radius: px(-20.),
            inset: false,
        },
    ]
}

/// A small bordered button (`.btn.small`).
pub fn small_button(t: &Theme) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.))
        .h(px(26.))
        .px(px(9.))
        .rounded(px(R_MD))
        .border_1()
        .border_color(t.line)
        .bg(t.s2)
        .text_color(t.text2)
        .text_size(px(FS_SM))
        .whitespace_nowrap()
        .cursor_pointer()
}

/// A square icon button (`.icon-btn`).
pub fn icon_button(t: &Theme, name: &str) -> Div {
    div()
        .flex_none()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(R_SM))
        .text_color(t.text3)
        .cursor_pointer()
        .child(icon(name, 16., t.text3))
}

/// A small text tooltip (the web viewer's `title`).
pub struct TextTip(SharedString);

impl gpui::Render for TextTip {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let t = crate::theme::theme(cx);
        div()
            .max_w(px(420.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(R_SM))
            .border_1()
            .border_color(t.line2)
            .bg(t.s1)
            .text_color(t.text2)
            .text_size(px(FS_SM))
            .font_family(crate::theme::FONT_UI)
            .shadow(shadow_1(t))
            .child(self.0.clone())
    }
}

/// `.tooltip(tip("text"))`
pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let text: SharedString = text.into();
    move |_, cx| gpui::AppContext::new(cx, |_| TextTip(text.clone())).into()
}
