//! Embedded assets: the web viewer's 16x16 stroke icons (viewer/src/util.js)
//! as SVG files `icons/<name>.svg`, and the fonts made by build.rs.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

const ICONS: &[(&str, &str)] = &[
    (
        "search",
        r#"<circle cx="7" cy="7" r="4.5"/><path d="m10.5 10.5 3.5 3.5"/>"#,
    ),
    ("override", r#"<path d="M8 13.5v-11M4 6.5l4-4 4 4"/>"#),
    ("off", r#"<path d="M8 2.5v5"/><path d="M4.9 4.4a5 5 0 1 0 6.2 0"/>"#),
    ("error", r#"<path d="M8 2.5 14 13H2z"/><path d="M8 6.5v3M8 11.4v.1"/>"#),
    (
        "option",
        r#"<path d="M6 2.5c-1.4 0-2 .6-2 2v1.6c0 .9-.5 1.5-1.5 1.9 1 .4 1.5 1 1.5 1.9v1.6c0 1.4.6 2 2 2M10 2.5c1.4 0 2 .6 2 2v1.6c0 .9.5 1.5 1.5 1.9-1 .4-1.5 1-1.5 1.9v1.6c0 1.4-.6 2-2 2"/>"#,
    ),
    (
        "module",
        r#"<path d="M8 1.8 13.5 5v6L8 14.2 2.5 11V5z"/><path d="M2.5 5 8 8.2 13.5 5M8 8.2v6"/>"#,
    ),
    (
        "copy",
        r#"<rect x="5.5" y="5.5" width="8" height="8" rx="2"/><path d="M10.5 5.5V4A1.5 1.5 0 0 0 9 2.5H4A1.5 1.5 0 0 0 2.5 4v5A1.5 1.5 0 0 0 4 10.5h1.5"/>"#,
    ),
    ("check", r#"<path d="m3.5 8.5 3 3 6-7"/>"#),
    ("x", r#"<path d="m4.5 4.5 7 7M11.5 4.5l-7 7"/>"#),
    (
        "open",
        r#"<path d="M8 10.5v-8M5 5.5l3-3 3 3"/><path d="M2.5 10v2A1.5 1.5 0 0 0 4 13.5h8a1.5 1.5 0 0 0 1.5-1.5v-2"/>"#,
    ),
    ("plus", r#"<path d="M8 3.5v9M3.5 8h9"/>"#),
    ("minus", r#"<path d="M3.5 8h9"/>"#),
    (
        "fit",
        r#"<path d="M2.5 6V3.5a1 1 0 0 1 1-1H6M10 2.5h2.5a1 1 0 0 1 1 1V6M13.5 10v2.5a1 1 0 0 1-1 1H10M6 13.5H3.5a1 1 0 0 1-1-1V10"/>"#,
    ),
    ("diamond", r#"<path d="M8 2.5 13.5 8 8 13.5 2.5 8z"/>"#),
    ("chevron", r#"<path d="m4.5 6.5 3.5 3.5 3.5-3.5"/>"#),
    ("chevron-up", r#"<path d="m4.5 9.5 3.5-3.5 3.5 3.5"/>"#),
    ("chevron-right", r#"<path d="m6.5 4.5 3.5 3.5-3.5 3.5"/>"#),
    (
        "instance",
        r#"<rect x="2.5" y="2.5" width="11" height="11" rx="3"/><path d="M5.5 6.5h5M5.5 9.5h3"/>"#,
    ),
    ("filter", r#"<path d="M2.5 3.5h11L9.5 8.5v4l-3 1.5V8.5z"/>"#),
    (
        "merge",
        r#"<circle cx="4" cy="3.5" r="1.5"/><circle cx="4" cy="12.5" r="1.5"/><circle cx="12" cy="8" r="1.5"/><path d="M4 5v6M4 5c0 2 2 3 6.5 3"/>"#,
    ),
    (
        "sun",
        r#"<circle cx="8" cy="8" r="3"/><path d="M8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1 1M11.6 11.6l1 1M3.4 12.6l1-1M11.6 4.4l1-1"/>"#,
    ),
    (
        "moon",
        r#"<path d="M13.5 9.5A5.5 5.5 0 0 1 6.5 2.5a5.5 5.5 0 1 0 7 7z"/>"#,
    ),
];

pub struct Assets;

impl Assets {
    fn icon(name: &str, width: f32) -> Option<String> {
        let body = ICONS.iter().find(|(n, _)| *n == name)?.1;
        Some(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="black" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
        ))
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // icons/<name>.svg, or icons/<name>@2.svg with a heavier stroke.
        let Some(name) = path.strip_prefix("icons/").and_then(|p| p.strip_suffix(".svg")) else {
            return Ok(None);
        };
        if name == "dot-diamond" {
            // The input origin mark: a filled diamond.
            let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M5 .4 9.6 5 5 9.6.4 5z" fill="black"/></svg>"#;
            return Ok(Some(Cow::Borrowed(svg.as_bytes())));
        }
        let (name, width) = match name.strip_suffix("@2") {
            Some(n) => (n, 2.0),
            None => (name, 1.5),
        };
        Ok(Self::icon(name, width).map(|s| Cow::Owned(s.into_bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(if path.trim_end_matches('/') == "icons" {
            ICONS
                .iter()
                .map(|(n, _)| SharedString::from(format!("icons/{n}.svg")))
                .collect()
        } else {
            Vec::new()
        })
    }
}

/// Geist and Geist Mono at 400/500/600/700 (build.rs).
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    macro_rules! font {
        ($f:literal) => {
            Cow::Borrowed(include_bytes!(concat!(env!("OUT_DIR"), "/", $f)).as_slice())
        };
    }
    vec![
        font!("Geist-400.ttf"),
        font!("Geist-500.ttf"),
        font!("Geist-600.ttf"),
        font!("Geist-700.ttf"),
        font!("GeistMono-400.ttf"),
        font!("GeistMono-500.ttf"),
        font!("GeistMono-600.ttf"),
        font!("GeistMono-700.ttf"),
    ]
}
