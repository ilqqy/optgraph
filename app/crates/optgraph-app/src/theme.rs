//! Design tokens of the web viewer (viewer/style.css: "Graphite"), dark and
//! light. Statuses (win, lose, off) always come with a mark or a label too,
//! never colour alone.

use gpui::{App, Global, Hsla, Rgba, rgb, rgba};
use optgraph_core::OriginClass;

pub const FONT_UI: &str = "Geist";
pub const FONT_MONO: &str = "Geist Mono";

// Type scale and radii (px), as in style.css.
pub const FS_XS: f32 = 11.;
pub const FS_SM: f32 = 12.;
pub const FS_MD: f32 = 13.;
pub const FS_LG: f32 = 15.;
pub const FS_XL: f32 = 19.;
pub const FS_2XL: f32 = 22.;
pub const R_SM: f32 = 6.;
pub const R_MD: f32 = 9.;
pub const R_LG: f32 = 13.;
pub const R_XL: f32 = 16.;
pub const SIDE_W: f32 = 248.;
pub const TOP_H: f32 = 48.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub mode: Mode,
    pub bg: Hsla,
    pub s1: Hsla,
    pub s2: Hsla,
    pub s3: Hsla,
    pub line: Hsla,
    pub line2: Hsla,
    pub text: Hsla,
    pub text2: Hsla,
    pub text3: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub on_accent: Hsla,
    pub win: Hsla,
    pub win_soft: Hsla,
    pub on_win: Hsla,
    pub lose: Hsla,
    pub lose_soft: Hsla,
    pub off: Hsla,
    pub err: Hsla,
    pub err_soft: Hsla,
    pub o_user: Hsla,
    pub o_user_inline: Hsla,
    pub o_input: Hsla,
    pub o_nixpkgs: Hsla,
    pub o_unknown: Hsla,
    /// Labels of receded graph modules: >= 3:1 on bg.
    pub dim_text: Hsla,
    pub edge: Hsla,
    pub grid: Hsla,
    pub scrim: Hsla,
    /// Shadow colour of raised surfaces.
    pub shadow: Hsla,
}

impl Global for Theme {}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn ca(hex_rgba: u32) -> Hsla {
    rgba(hex_rgba).into()
}

impl Theme {
    pub fn dark() -> Theme {
        Theme {
            mode: Mode::Dark,
            bg: c(0x0a0c10),
            s1: c(0x0f1218),
            s2: c(0x151922),
            s3: c(0x1c212c),
            line: c(0x232937),
            line2: c(0x313a4c),
            text: c(0xe8ebf1),
            text2: c(0xaab1c0),
            text3: c(0x8992a5),
            accent: c(0xa093ff),
            accent_soft: ca(0xa093ff24),
            on_accent: c(0x0e0b22),
            win: c(0x3ddc97),
            win_soft: ca(0x3ddc9713),
            on_win: c(0x03140b),
            lose: c(0xffa45e),
            lose_soft: ca(0xffa45e0f),
            off: c(0x8f98ab),
            err: c(0xff7f7f),
            err_soft: ca(0xff7f7f14),
            o_user: c(0x62a6ff),
            o_user_inline: c(0xea8ccf),
            o_input: c(0x34cdb8),
            o_nixpkgs: c(0x8790a3),
            o_unknown: c(0xe6c46e),
            dim_text: c(0x5f6778),
            edge: c(0x2f3748),
            grid: ca(0xffffff11),
            scrim: ca(0x03050990),
            shadow: ca(0x000000bf),
        }
    }

    pub fn light() -> Theme {
        Theme {
            mode: Mode::Light,
            bg: c(0xf4f5f8),
            s1: c(0xffffff),
            s2: c(0xffffff),
            s3: c(0xeff1f5),
            line: c(0xe1e4ea),
            line2: c(0xcdd2dc),
            text: c(0x13161c),
            text2: c(0x424a59),
            text3: c(0x5b6474),
            accent: c(0x5b47e0),
            accent_soft: ca(0x5b47e01a),
            on_accent: c(0xffffff),
            win: c(0x0d7743),
            win_soft: ca(0x0d774312),
            on_win: c(0xffffff),
            lose: c(0xb04e0a),
            lose_soft: ca(0xb04e0a0f),
            off: c(0x5f6878),
            err: c(0xc0262d),
            err_soft: ca(0xc0262d12),
            o_user: c(0x2563eb),
            o_user_inline: c(0xb4298f),
            o_input: c(0x0c7d70),
            o_nixpkgs: c(0x626a79),
            o_unknown: c(0x956600),
            dim_text: c(0x7d8594),
            edge: c(0xc2c8d3),
            grid: ca(0x141e3c1a),
            scrim: ca(0x12162052),
            shadow: ca(0x10182833),
        }
    }

    pub fn for_mode(mode: Mode) -> Theme {
        match mode {
            Mode::Dark => Theme::dark(),
            Mode::Light => Theme::light(),
        }
    }

    pub fn origin(&self, class: OriginClass) -> Hsla {
        match class {
            OriginClass::User => self.o_user,
            OriginClass::UserInline => self.o_user_inline,
            OriginClass::Input => self.o_input,
            OriginClass::Nixpkgs => self.o_nixpkgs,
            OriginClass::Unknown => self.o_unknown,
        }
    }
}

pub fn theme(cx: &App) -> &Theme {
    cx.global::<Theme>()
}

/// `top` composited over the opaque `base`: an opaque colour. GPUI's
/// gradients interpolate unpremultiplied colours, so a translucent stop
/// tints the whole gradient; CSS composites first.
pub fn over(base: Hsla, top: Hsla) -> Hsla {
    let (b, t) = (Rgba::from(base), Rgba::from(top));
    let a = t.a;
    Rgba {
        r: t.r * a + b.r * (1. - a),
        g: t.g * a + b.g * (1. - a),
        b: t.b * a + b.b * (1. - a),
        a: 1.,
    }
    .into()
}
