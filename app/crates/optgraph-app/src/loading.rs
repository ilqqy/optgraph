//! Opening a graph.json (file dialog, drag and drop) and the colour scheme
//! (follows the system unless pinned by --dark/--light or the toggle).

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, ExternalPaths, FontWeight, IntoElement, PathPromptOptions, SharedString, Window,
    WindowAppearance, div, prelude::*, px,
};
use optgraph_core::{Model, load};

use crate::sidebar::logo;
use crate::theme::{FS_2XL, FS_MD, Mode, R_MD, R_XL, Theme, theme};
use crate::ui::{icon, shadow_3};
use crate::view::OptgraphView;

/// How the colour scheme is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    System,
    Pinned(Mode),
}

pub fn mode_of(appearance: WindowAppearance) -> Mode {
    match appearance {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Mode::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => Mode::Light,
    }
}

impl OptgraphView {
    /// Loads a graph.json; on failure the current graph stays and the error shows.
    pub fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match load::from_path(&path) {
            Ok(g) => {
                let host = g.meta.host.clone();
                self.source = path.display().to_string();
                self.set_model(Model::new(g), cx);
                window.set_window_title(&match host {
                    Some(h) => format!("optgraph: {h}"),
                    None => "optgraph".to_string(),
                });
            }
            Err(e) => {
                self.load_error = Some(format!("{}: {e}", path.display()));
                cx.notify();
            }
        }
    }

    /// The platform's file dialog (xdg-desktop-portal on Linux).
    pub fn prompt_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open graph.json".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let picked = rx.await;
            this.update_in(cx, |this, window, cx| match picked {
                Ok(Ok(Some(paths))) => {
                    if let Some(p) = paths.into_iter().next() {
                        this.open_path(p, window, cx);
                    }
                }
                Ok(Ok(None)) | Err(_) => {}
                Ok(Err(e)) => {
                    eprintln!("optgraph-app: no file dialog: {e:#}");
                    this.load_error = Some(
                        "No file dialog (xdg-desktop-portal is not reachable): drop a graph.json on the window or pass it on the command line."
                            .into(),
                    );
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn on_drop_paths(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = paths.paths().first() {
            self.open_path(p.clone(), window, cx);
        }
    }

    /// Follow the system's light/dark preference unless pinned.
    pub fn apply_scheme(&mut self, window: &Window, cx: &mut Context<Self>) {
        let mode = match self.scheme {
            Scheme::Pinned(m) => m,
            Scheme::System => mode_of(window.appearance()),
        };
        if theme(cx).mode != mode {
            cx.set_global(Theme::for_mode(mode));
            cx.notify();
        }
    }

    pub fn toggle_scheme(&mut self, window: &Window, cx: &mut Context<Self>) {
        let next = match theme(cx).mode {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
        };
        self.scheme = Scheme::Pinned(next);
        self.apply_scheme(window, cx);
    }

    /// No graph: a drop zone with an open button (and the last error).
    pub fn render_empty(&mut self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(16.))
            .child(
                div()
                    .max_w(px(520.))
                    .px(px(32.))
                    .pt(px(32.))
                    .pb(px(24.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(R_XL))
                    .border(px(1.5))
                    .border_dashed()
                    .border_color(t.line2)
                    .bg(t.s1)
                    .shadow(shadow_3(t))
                    .child(logo(t, 40. / 22.))
                    .child(
                        div()
                            .mt(px(8.))
                            .text_size(px(FS_2XL))
                            .font_weight(FontWeight(650.))
                            .child("optgraph"),
                    )
                    .child(
                        div()
                            .text_color(t.text2)
                            .text_center()
                            .child("Drop a graph.json made by optgraph here, or open one."),
                    )
                    .child(
                        div()
                            .id("open-empty")
                            .mt(px(8.))
                            .h(px(36.))
                            .px(px(18.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .rounded(px(R_MD))
                            .bg(t.accent)
                            .text_color(t.on_accent)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(FS_MD))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, window, cx| this.prompt_open(window, cx)))
                            .child(icon("open", 16., t.on_accent))
                            .child("Open graph.json"),
                    )
                    .when_some(self.load_error.clone(), |d, e| {
                        d.child(
                            div()
                                .mt(px(8.))
                                .text_color(t.err)
                                .text_center()
                                .child(SharedString::from(e)),
                        )
                    })
                    .child(
                        div()
                            .mt(px(8.))
                            .text_color(t.text3)
                            .text_size(px(12.))
                            .text_center()
                            .child("or run: optgraph-app <flake>#nixosConfigurations.<host>"),
                    ),
            )
            .into_any_element()
    }
}
